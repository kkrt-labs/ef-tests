use std::{fs, path::Path};

use serde::Deserialize;

use crate::models::error::RunnerError;

pub(crate) fn load_file(path: &Path) -> Result<String, RunnerError> {
    fs::read_to_string(path).map_err(|error| RunnerError::Io {
        path: path.into(),
        error: error.to_string(),
    })
}

pub(crate) fn deserialize_into<T: for<'a> Deserialize<'a>>(
    val: &str,
    path: &Path,
) -> Result<T, RunnerError> {
    serde_json::from_str(&val).map_err(|error| RunnerError::Io {
        path: path.into(),
        error: error.to_string(),
    })
}
