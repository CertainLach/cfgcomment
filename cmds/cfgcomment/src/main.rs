use cfgcomment_core::{walkdir_parallel, Data, LangDesc};
use std::{collections::HashSet, path::PathBuf};
use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(name = "cfgcomment", author)]
enum Opts {
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

fn main() {
    env_logger::init();
    let opts = Opts::from_args();

    let lang_config = std::array::IntoIter::new([
        (
            "rs".to_owned(),
            LangDesc {
                cfg_prefix: "//[".to_owned(),
                cfg_prefix_comment_len: 2,
                cfg_suffix: "]".to_owned(),
                comment: "//# ".to_owned(),
            },
        ),
        (
            "js".to_owned(),
            LangDesc {
                cfg_prefix: "//[".to_owned(),
                cfg_prefix_comment_len: 2,
                cfg_suffix: "]".to_owned(),
                comment: "//# ".to_owned(),
            },
        ),
        (
            "ts".to_owned(),
            LangDesc {
                cfg_prefix: "//[".to_owned(),
                cfg_prefix_comment_len: 2,
                cfg_suffix: "]".to_owned(),
                comment: "//# ".to_owned(),
            },
        ),
        (
            "toml".to_owned(),
            LangDesc {
                cfg_prefix: "#[".to_owned(),
                cfg_prefix_comment_len: 1,
                cfg_suffix: "]".to_owned(),
                comment: "#- ".to_owned(),
            },
        ),
    ])
    .collect();

    match opts {
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
}
