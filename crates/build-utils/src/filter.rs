use regex::Regex;
use serde::Deserialize;
use std::{collections::BTreeMap, fs};

use crate::path::PathWrapper;

type FilterMap = BTreeMap<String, Vec<String>>;

/// Filter to be applied on the tests files
#[derive(Deserialize, Default)]
pub struct Filter {
    /// Mapping containing the directories and the files that should be skipped
    filename: FilterMap,
    /// Mapping containing the directories and the regex patterns that should be skipped
    regex: FilterMap,
    /// Vector containing the specific tests that should be skipped
    #[serde(rename = "testname")]
    test_name: Vec<String>,
}

impl Filter {
    pub fn load_file(path: &str) -> Result<Self, eyre::Error> {
        let filter = fs::read_to_string(path)?;
        Ok(serde_yaml::from_str(&filter)?)
    }

    /// Returns the difference between the two filters
    pub fn diff(self, rhs: Self) -> Self {
        let filename = map_diff(self.filename, rhs.filename);
        let regex = map_diff(self.regex, rhs.regex);
        let mut test_name = Vec::new();

        for test in &self.test_name {
            if !rhs.test_name.contains(test) {
                test_name.push(test.clone());
            }
        }
        for test in rhs.test_name {
            if !self.test_name.contains(&test) {
                test_name.push(test);
            }
        }

        Self {
            filename,
            regex,
            test_name,
        }
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
        self.test_name.contains(&test_name.to_string())
    }
}

fn map_diff(lhs: FilterMap, rhs: FilterMap) -> FilterMap {
    let mut top = BTreeMap::new();
    for (key, value) in lhs.iter() {
        let mut diff = Vec::new();
        if !rhs.contains_key(key) {
            top.insert(key.clone(), value.clone());
            continue;
        }
        for v in value {
            if !rhs.get(key).unwrap().contains(v) {
                diff.push(v.to_string());
            }
        }
        top.insert(key.clone(), diff);
    }

    for (key, value) in rhs.iter() {
        let mut diff = Vec::new();
        if !lhs.contains_key(key) {
            top.insert(key.clone(), diff.clone());
            continue;
        }
        for v in value {
            if !lhs.get(key).unwrap().contains(v) {
                diff.push(v.to_string());
            }
        }
        top.insert(key.clone(), diff);
    }

    top
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_filter_file() {
        let filter = Filter::load_file("../../blockchain-tests-skip.yml").unwrap();
        let path = PathWrapper::from(Path::new(
            "../../ef-testing/ethereum-tests/BlockchainTests/GeneralStateTests/stCallCreateCallCodeTest/Call1024PreCalls.json",
        ).to_path_buf());
        assert!(filter.is_skipped(&path));
    }

    #[test]
    fn test_filter_regex() {
        let filter = Filter::load_file("../../blockchain-tests-skip.yml").unwrap();
        let path = PathWrapper::from(Path::new(
            "../../ef-testing/ethereum-tests/BlockchainTests/GeneralStateTests/stBadOpcode/opc4DDiffPlaces.json",
        ).to_path_buf());
        assert!(filter.is_skipped(&path));
    }

    #[test]
    fn test_filter_test() {
        let filter = Filter::load_file("../../blockchain-tests-skip.yml").unwrap();
        assert!(filter.is_test_skipped("CreateMessageSuccess_d0g0v0_Shanghai"));
    }

    #[test]
    fn test_map_diff() {
        // Given
        let lhs: FilterMap = vec![
            ("a".to_string(), vec!["a".to_string()]),
            ("b".to_string(), vec!["b".to_string(), "b".to_string()]),
            (
                "c".to_string(),
                vec!["c".to_string(), "c".to_string(), "c".to_string()],
            ),
            (
                "e".to_string(),
                vec!["e".to_string(), "f".to_string(), "g".to_string()],
            ),
        ]
        .into_iter()
        .collect();
        let rhs: FilterMap = vec![
            ("a".to_string(), vec!["a".to_string()]),
            ("b".to_string(), vec!["b".to_string(), "d".to_string()]),
            (
                "c".to_string(),
                vec!["c".to_string(), "c".to_string(), "c".to_string()],
            ),
            (
                "d".to_string(),
                vec!["e".to_string(), "f".to_string(), "g".to_string()],
            ),
        ]
        .into_iter()
        .collect();

        // When
        let diff = map_diff(lhs, rhs);

        // Then
        let expected: FilterMap = vec![
            ("a".to_string(), vec![]),
            ("b".to_string(), vec!["d".to_string()]),
            ("c".to_string(), vec![]),
            (
                "d".to_string(),
                vec!["e".to_string(), "f".to_string(), "g".to_string()],
            ),
            (
                "e".to_string(),
                vec!["e".to_string(), "f".to_string(), "g".to_string()],
            ),
        ]
        .into_iter()
        .collect();

        assert_eq!(diff, expected)
    }
}
