use std::{collections::HashSet, path::PathBuf};

use cfgcomment_core::{walkdir_parallel, Data, LangDesc};

pub fn preprocess() {
    let features: HashSet<String> = std::env::vars()
        .filter_map(|(n, _)| n.strip_prefix("CARGO_FEATURE_").map(|s| s.to_owned()))
        .map(|s| s.to_ascii_lowercase().replace("_", "-"))
        .collect();
    let paths = vec![PathBuf::from("src")];

    walkdir_parallel(
        paths,
        Data {
            features,
            reset: false,
        },
        LangDesc::default_list(),
    )
}
