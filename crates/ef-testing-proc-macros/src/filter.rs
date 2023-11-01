use regex::Regex;
use serde::Deserialize;
use std::collections::BTreeMap;

use crate::path::PathWrapper;

/// Filter to be applied on the tests files
#[derive(Deserialize)]
pub struct Filter {
    /// Mapping containing the directories and the files that should be skipped
    filename: BTreeMap<String, Vec<String>>,
    /// Mapping containing the directories and the regex patterns that should be skipped
    regex: BTreeMap<String, Vec<String>>,
    /// Vector containing the specific tests that should be skipped
    testname: Vec<String>,
}

impl Filter {
    pub fn new(filter: &str) -> Self {
        serde_yaml::from_str(filter).expect("Error while deserializing into Filter struct")
    }

    /// Checks if the given path should be skipped
    pub fn is_skipped(&self, path: &PathWrapper) -> bool {
        let dir_name = path.parent().file_stem_to_string();
        let file_name = path.file_stem_to_string();

        let mut should_skip = if self.filename.contains_key(&dir_name) {
            self.filename
                .get(&dir_name)
                .unwrap() // safe unwrap
                .iter()
                .any(|filename| filename == &file_name)
        } else {
            false
        };

        if !should_skip && self.regex.contains_key(&dir_name) {
            should_skip = self.regex.get(&dir_name).unwrap().iter().any(|regex| {
                Regex::new(regex.as_str())
                    .expect("Error with regex pattern")
                    .is_match(&file_name)
            });
        }
        should_skip
    }

    /// Checks if the given test should be skipped
    pub fn is_test_skipped(&self, test_name: &str) -> bool {
        self.testname.contains(&test_name.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_filter_file() {
        let filter = Filter::new(include_str!("../../../blockchain-tests-skip.yml"));
        let path = PathWrapper::from(Path::new(
            "../../ef-testing/ethereum-tests/BlockchainTests/GeneralStateTests/stCallCreateCallCodeTest/Call1024PreCalls.json",
        ).to_path_buf());
        assert!(filter.is_skipped(&path));
    }

    #[test]
    fn test_filter_regex() {
        let filter = Filter::new(include_str!("../../../blockchain-tests-skip.yml"));
        let path = PathWrapper::from(Path::new(
            "../../ef-testing/ethereum-tests/BlockchainTests/GeneralStateTests/stBadOpcode/opc4DDiffPlaces.json",
        ).to_path_buf());
        assert!(filter.is_skipped(&path));
    }

    #[test]
    fn test_filter_test() {
        let filter = Filter::new(include_str!("../../../blockchain-tests-skip.yml"));
        assert!(filter.is_test_skipped("randomStatetest303_d0g0v0_Shanghai"));
    }
}
