lazy_static::lazy_static! {
    pub static ref UNSUPPORTED_IDENTIFIER_CHAR: regex::Regex = regex::Regex::new(r"[=^.]").unwrap();
}

pub const ROOT: &str = "GeneralStateTests";
pub const FORK: &str = "Shanghai";
