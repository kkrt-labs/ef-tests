lazy_static::lazy_static! {
    pub(crate) static ref UNSUPPORTED_IDENTIFIER_CHAR: regex::Regex = regex::Regex::new(r"[=^.]").unwrap();
}

pub(crate) const SKIPPED_TESTS: &str = include_str!("../../../blockchain-tests-skip.yml");
pub(crate) const ROOT: &str = "GeneralStateTests";
pub(crate) const FORK: &str = "Shanghai";
