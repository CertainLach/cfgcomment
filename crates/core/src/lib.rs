use std::{cell::RefCell, collections::{HashMap, HashSet}, fs::File, io::{BufRead, BufReader, BufWriter, Write}, path::PathBuf, rc::Rc, sync::Arc};

pub struct Data {
    pub features: HashSet<String>,
    pub reset: bool,
}
impl Data {
    fn has_feature(&self, feature: &str) -> bool {
        self.features.contains(feature)
    }
}

enum Predicate {
    Feature(String),
}
impl Predicate {
    fn matches(&self, config: &Data) -> bool {
        match self {
            Self::Feature(f) => config.has_feature(f),
        }
    }
}

enum Group {
    Option(Predicate),
    All(Vec<Self>),
    Any(Vec<Self>),
    Not(Box<Self>),
}

impl Group {
    fn matches(&self, config: &Data) -> bool {
        match self {
            Self::Option(o) => o.matches(config),
            Self::All(v) => v.iter().all(|p| p.matches(config)),
            Self::Any(v) => v.iter().any(|p| p.matches(config)),
            Self::Not(v) => !v.matches(config),
        }
    }
}

enum CfgTag {
    Start(Group),
    End,
}

peg::parser! {
    grammar cfg() for str {
        pub(crate) rule cfg() -> CfgTag
            = "[" _ "cfg" _ "(" _ "end" _ ")" _ "]" {CfgTag::End}
            / "[" _ "cfg" _ "(" _ p:pred() _ ")" _ "]" {CfgTag::Start(p)}

        rule opt() -> Predicate
            = "feature" _ "=" _ "\"" s:$((!['"'] [_])*) "\"" {Predicate::Feature(s.to_owned())}

        rule pred() -> Group
            = "any" _ "(" _ l:pred_list() _ ")" {Group::Any(l)}
            / "all" _ "(" _ l:pred_list() _ ")" {Group::All(l)}
            / "not" _ "(" _ p:pred() _ ")" {Group::Not(Box::new(p))}
            / o:opt() {Group::Option(o)}

        rule list_sep() = _ "," _
        rule pred_list() -> Vec<Group>
            = l:pred()**list_sep() list_sep()? {l}

            rule _ = [' ' | '\t']*
    }
}

fn split_at_ws_end(i: &str) -> (&str, &str) {
    let idx = i
        .bytes()
        .position(|c| !c.is_ascii_whitespace())
        .unwrap_or(i.len());
    i.split_at(idx)
}

#[derive(Default, Clone)]
struct CfgState(Rc<RefCell<Vec<(bool, String)>>>);
impl CfgState {
    fn enabled(&self) -> bool {
        self.0.borrow().iter().all(|(p, _)| *p)
    }
    fn prefix(&self) -> String {
        self.0
            .borrow()
            .iter()
            .last()
            .map(|(_, p)| p.to_owned())
            .unwrap_or_else(|| "".to_owned())
    }
    fn push(&self, state: (bool, String)) {
        self.0.borrow_mut().push(state)
    }
    fn pop(&self) -> Option<()> {
        self.0.borrow_mut().pop().map(|_| ())
    }
}

#[derive(Clone)]
pub struct LangDesc {
    pub cfg_prefix: String,
    pub cfg_prefix_comment_len: usize,
    pub cfg_suffix: String,
    pub comment: String,
}

fn process(
    read: impl Iterator<Item = String>,
    config: Arc<Data>,
    desc: Rc<LangDesc>,
) -> impl Iterator<Item = String> {
    let state = CfgState::default();
    read.map(move |s| {
        let state = state.clone();
        if s.trim_start().starts_with(&desc.cfg_prefix) && s.trim_end().ends_with(&desc.cfg_suffix)
        {
            let (ws, cfg) = split_at_ws_end(&s);
            let parsed = cfg::cfg(&cfg[desc.cfg_prefix_comment_len..]).unwrap();
            match parsed {
                CfgTag::Start(c) => {
                    state.push((c.matches(&config), ws.to_owned()));
                    s
                }
                CfgTag::End => {
                    state.pop().expect("unexpected end");
                    s
                }
            }
        } else {
            if s.trim().is_empty() {
                return s;
            }
            let prefix = state.prefix();
            let trimmed = s.strip_prefix(&prefix).expect("wrong prefix");
            let enabled = !trimmed.starts_with(&desc.comment);
            let should_be = config.reset || state.enabled();

            log::trace!("{} {:?} {:?}", trimmed, enabled, should_be);
            if !enabled && should_be {
                format!("{}{}", prefix, &trimmed[desc.comment.len()..])
            } else if enabled && !should_be {
                format!("{}{}{}", prefix, desc.comment, trimmed)
            } else {
                s
            }
        }
    })
}

pub fn walkdir_parallel(paths: Vec<PathBuf>, config: Data, lang_config: HashMap<String, LangDesc>) {
    let mut walk = ignore::WalkBuilder::new(paths[0].clone());
    for dir in paths.iter().skip(1) {
        walk.add(dir);
    }
    walk.add_custom_ignore_filename(".cfgignore");

    let config = Arc::new(config);
    let lang_config = Arc::new(lang_config);

    walk.build_parallel().run(move || {
        let config = config.clone();
        let lang_config = lang_config.clone();
        Box::new(move |path| {
            let path = path.unwrap();
            // Skip dirs/symlinks
            if !path.file_type().map(|f| f.is_file()).unwrap_or(false) {
                return ignore::WalkState::Continue;
            }
            let extension = match path.path().extension() {
                Some(v) => v,
                None => return ignore::WalkState::Continue,
            };
            let extension = extension.to_string_lossy().to_string();
            let desc = match lang_config.get(&extension) {
                Some(v) => v,
                None => return ignore::WalkState::Continue,
            };
            let desc = Rc::new(desc.clone());

            let file = BufReader::new(File::open(path.path()).unwrap());
            let mut out = BufWriter::new(
                tempfile::NamedTempFile::new_in(path.path().parent().unwrap()).unwrap(),
            );

            for line in process(
                file.lines().map(|l| l.unwrap()),
                config.clone(),
                desc,
            ) {
                writeln!(out, "{}", line).unwrap();
            }

            out.into_inner().unwrap().persist(path.path()).unwrap();

            ignore::WalkState::Continue
        })
    });
}
