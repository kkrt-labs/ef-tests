use std::path::Path;

lazy_static::lazy_static! {
    pub(crate) static ref TEST_FILTER_PATH: &'static Path =  Path::new("blockchain-tests-skip.yml");
}

pub(crate) const REGEX_STR: &str = r"[=^.]";
pub(crate) const ROOT: &str = "GeneralStateTests";
pub(crate) const FORK: &str = "Shanghai";
