use std::{fs, path::Path};

use serde::Deserialize;

pub(crate) fn load_file(path: &Path) -> Result<String, ef_tests::Error> {
    fs::read_to_string(path).map_err(|error| ef_tests::Error::Io {
        path: path.into(),
        error: error.to_string(),
    })
}

pub(crate) fn deserialize_into<T: for<'a> Deserialize<'a>>(
    val: String,
    path: &Path,
) -> Result<T, ef_tests::Error> {
    serde_json::from_str(&val).map_err(|error| ef_tests::Error::CouldNotDeserialize {
        path: path.into(),
        error: error.to_string(),
    })
}
