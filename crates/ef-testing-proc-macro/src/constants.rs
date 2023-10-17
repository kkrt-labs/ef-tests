use std::path::{Path, PathBuf};

lazy_static::lazy_static! {
    pub(crate) static ref TEST_FILTER_PATH: PathBuf =  Path::new("blockchain-tests-skip.yml").to_path_buf();
    pub(crate) static ref REGEX: regex::Regex = regex::Regex::new(r"[=^.]").unwrap();
}

pub(crate) const ROOT: &str = "GeneralStateTests";
pub(crate) const FORK: &str = "Shanghai";
