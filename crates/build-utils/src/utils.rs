use std::path::{Path, PathBuf};

use crate::path::PathWrapper;

/// Converts the path to a string and removes the `BlockchainTests` folder
///
/// # Example
///
/// Input: BlockchainTests/GeneralStateTests/stCallCreateCallCodeTest/Call1024PreCalls.json
/// Output: GeneralStateTests/stCallCreateCallCodeTest/Call1024PreCalls.json
pub fn blockchain_tests_to_general_state_tests_path(path: PathWrapper) -> PathWrapper {
    path.components()
        .filter(|x| !x.as_os_str().eq_ignore_ascii_case("BlockchainTests"))
        .collect::<PathBuf>()
        .into()
}

/// Converts the path to a Vector of Strings
pub fn path_to_vec_string(path: &Path) -> Result<Vec<String>, eyre::Error> {
    path.iter()
        .map(|os_str| {
            Ok(os_str
                .to_str()
                .ok_or_else(|| eyre::eyre!("Error while converting the path to a string"))?
                .to_string())
        })
        .collect()
}

/// Returns the path relative to the given folder
pub fn path_relative_to(path: Vec<String>, folder: &str) -> Vec<String> {
    path.into_iter()
        .skip_while(|x| x != folder)
        .skip(1)
        .collect()
}
