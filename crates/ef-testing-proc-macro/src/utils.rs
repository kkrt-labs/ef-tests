use std::path::{Path, PathBuf};

use crate::dir_reader::PathWrapper;

/// Converts the path to a string and removes the "BlockchainTests" folder
///
/// # Example
///
/// Input: BlockchainTests/GeneralStateTests/stCallCreateCallCodeTest/Call1024PreCalls.json
/// Output: GeneralStateTests/stCallCreateCallCodeTest/Call1024PreCalls.json
pub(crate) fn blockchain_tests_to_general_state_tests_path(path: PathWrapper) -> PathWrapper {
    Into::<PathBuf>::into(path)
        .components()
        .filter(|x| !x.as_os_str().eq_ignore_ascii_case("BlockchainTests"))
        .collect::<PathBuf>()
        .into()
}

/// Converts the path to a Vector of Strings
pub(crate) fn path_to_vec_string(path: &Path) -> Result<Vec<String>, eyre::Error> {
    path.iter()
        .map(|os_str| {
            Ok(os_str
                .to_str()
                .ok_or_else(|| eyre::eyre!("Error while converting the path to a string"))?
                .to_string())
        })
        .collect()
}

/// Trims the path at the given folder, returning only the path after the folder
pub(crate) fn trim_path_at_folder(path: Vec<String>, folder: &str) -> Vec<String> {
    path.into_iter()
        .skip_while(|x| x != folder)
        .skip(1)
        .collect()
}
