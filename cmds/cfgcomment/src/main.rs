use anyhow::{bail, Context};
use cfgcomment_core::{process, walkdir_parallel, Data, LangDesc};
use git_filter_server::{GitFilterServer, ProcessingType, Processor};
use std::{
    collections::{HashMap, HashSet},
    fs::OpenOptions,
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::Command,
    rc::Rc,
    sync::Arc,
};
use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(name = "cfgcomment", author)]
enum Opts {
    /// Configure git filter for resetting comments on stage
    Init,
    /// Internal command used by git attributes
    Git,
    /// Apply cfg comments
    Apply {
        /// Paths to process, if dir passed - then it is recursive walked
        #[structopt(required = true)]
        paths: Vec<PathBuf>,
        /// Features to use with cfg(feature = "name")
        #[structopt(long)]
        features: Vec<String>,
    },
    /// Reset cfg comments, uncommenting everything
    Reset { paths: Vec<PathBuf> },
}

struct UncommentingProcessor {
    config: Arc<Data>,
    lang_config: HashMap<String, LangDesc>,
}
impl Processor for UncommentingProcessor {
    fn process<R: std::io::Read, W: Write>(
        &mut self,
        pathname: &str,
        _process_type: ProcessingType,
        input: &mut R,
        output: &mut W,
    ) -> anyhow::Result<()> {
        let path = PathBuf::from(pathname);
        let extension = match path.as_path().extension() {
            Some(v) => v,
            None => {
                std::io::copy(input, output)?;
                return Ok(());
            }
        };
        let extension = extension.to_string_lossy().to_string();
        let desc = match self.lang_config.get(&extension) {
            Some(v) => v,
            None => {
                std::io::copy(input, output)?;
                return Ok(());
            }
        };

        let lines = BufReader::new(input).lines().map(|l| l.unwrap());
        for line in process(lines, self.config.clone(), Rc::new(desc.clone())) {
            writeln!(output, "{}", line)?;
        }

        Ok(())
    }

    fn supports_processing(&self, process_type: ProcessingType) -> bool {
        matches!(process_type, ProcessingType::Clean)
    }
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::fmt()
        .with_writer(std::io::stderr)
        .init();
    let opts = Opts::from_args();

    let lang_config = LangDesc::default_list();

    match opts {
        Opts::Init => {
            if std::fs::metadata(".git")
                .map(|f| f.is_dir())
                .unwrap_or(false)
            {
                bail!("cfgcomment init should be called in root of git repo");
            }
            Command::new("git")
                .args(["config", "--bool", "filter.cfgcomment.required", "true"])
                .output()
                .context("while setting require")?;
            Command::new("git")
                .args(["config", "filter.cfgcomment.process", "cfgcomment git"])
                .output()
                .context("while setting process")?;
            let attributes = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .open(".gitattributes")
                .context("while creating .gitattributes")?;
            let attributes_reader = BufReader::new(attributes);
            let lines: HashSet<String> = attributes_reader.lines().flatten().collect();

            let mut attributes = OpenOptions::new().append(true).open(".gitattributes")?;
            let needed_lines: Vec<String> = lang_config
                .keys()
                .map(|k| format!("*.{} filter=cfgcomment", k))
                .collect();

            for line in needed_lines {
                if lines.contains(&line) {
                    continue;
                }
                writeln!(attributes, "{}", line)?;
            }
        }
        Opts::Git => {
            GitFilterServer::new(UncommentingProcessor {
                config: Arc::new(Data {
                    features: HashSet::new(),
                    reset: true,
                }),
                lang_config,
            }).communicate_stdio()?;
        }
        Opts::Apply { paths, features } => {
            let config = Data {
                features: features.into_iter().collect(),
                reset: false,
            };
            walkdir_parallel(paths, config, lang_config)
        }
        Opts::Reset { paths } => {
            let config = Data {
                features: HashSet::new(),
                reset: true,
            };
            walkdir_parallel(paths, config, lang_config)
        }
    }
    Ok(())
}
