use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use crate::{converter::EfTests, path::PathWrapper};

type Folder = String;
type FilterMap = BTreeMap<Folder, Vec<String>>;

/// Filter to be applied on the tests files
#[derive(Deserialize, Default, Serialize)]
pub struct Filter {
    // List of directories that should be skipped. e.g.: Pyspecs.
    directories: Vec<String>,
    /// Mapping containing the directories and the files that should be skipped
    filename: FilterMap,
    /// Mapping containing the directories and the regex patterns that should be skipped
    regex: FilterMap,
    /// Mapping containing the directories and the specific tests that should be skipped
    #[serde(rename = "testname")]
    test_name: FilterMap,
}

impl Filter {
    pub fn load_file(path: &str) -> Result<Self, eyre::Error> {
        let filter = fs::read_to_string(path)?;
        Ok(serde_yaml::from_str(&filter)?)
    }

    /// Checks if the given path is inside the filter object
    pub fn is_skipped(&self, path: &PathWrapper, case_name: Option<String>) -> bool {
        let pathb: PathBuf = (*path).clone().into();
        let path_str = pathb.to_string_lossy();

        let relative_path = path_str
            .split_once("GeneralStateTests/")
            .map(|(_, path)| path)
            .unwrap_or(&path_str);

        let relative_path = Path::new(relative_path)
            .components()
            .next()
            .and_then(|c| c.as_os_str().to_str())
            .map(ToString::to_string)
            .unwrap_or_default();

        let dir_name = path.parent().file_stem_to_string();
        let file_name = path.file_stem_to_string();

        if self
            .directories
            .iter()
            .any(|dir| relative_path.contains(dir))
        {
            return true;
        }

        let mut should_skip = self
            .filename
            .get(&dir_name)
            .map(|filtered_files| filtered_files.iter().any(|filename| filename == &file_name))
            .unwrap_or_default();

        should_skip |= self
            .regex
            .get(&dir_name)
            .map(|regexes| {
                regexes.iter().any(|regex| {
                    Regex::new(regex.as_str())
                        .expect("Error with regex pattern")
                        .is_match(&file_name)
                })
            })
            .unwrap_or_default();

        if let Some(case_name) = case_name {
            let test_identifier = &EfTests::format_into_identifier(&case_name);
            should_skip |= self
                .test_name
                .get(&dir_name)
                .map(|tests| tests.iter().any(|test| test == test_identifier))
                .unwrap_or_default();
        }

        should_skip
    }

    /// Returns the difference in keys (folders) between the two filters
    pub fn diff(&self, rhs: &Self) -> Vec<Folder> {
        // Allocate at most the sum of the lengths of the keys.
        let capacity = std::cmp::max(self.filename.len(), rhs.filename.len())
            + std::cmp::max(self.regex.len(), rhs.regex.len())
            + std::cmp::max(self.test_name.len(), rhs.test_name.len());
        let mut diff = Vec::with_capacity(capacity);

        diff.append(&mut diff_map(&self.filename, &rhs.filename));
        diff.append(&mut diff_map(&self.regex, &rhs.regex));
        diff.append(&mut diff_map(&self.test_name, &rhs.test_name));
        diff
    }
}

fn diff_map(lhs: &FilterMap, rhs: &FilterMap) -> Vec<Folder> {
    let mut top = Vec::with_capacity(std::cmp::max(lhs.len(), rhs.len()));
    let diff = |top: &mut Vec<String>, lhs: &FilterMap, rhs: &FilterMap| {
        for (key, _) in lhs.iter() {
            if !rhs.contains_key(key) && !top.contains(key) {
                top.push(key.clone());
                continue;
            }
            let same = lhs
                .get(key)
                .unwrap()
                .iter()
                .zip(rhs.get(key).unwrap().iter())
                .all(|(lhs, rhs)| lhs == rhs);
            if !same && !top.contains(key) {
                top.push(key.clone());
            }
        }
    };

    diff(&mut top, lhs, rhs);
    diff(&mut top, rhs, lhs);

    top
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_filter_regex() {
        let filter = Filter::load_file("../../blockchain-tests-skip.yml").unwrap();
        let path = PathWrapper::from(Path::new(
            "../../ef-testing/ethereum-tests/BlockchainTests/GeneralStateTests/stBadOpcode/opc4DDiffPlaces.json",
        ).to_path_buf());
        assert!(filter.is_skipped(&path, None));
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
        let mut diff = diff_map(&lhs, &rhs);
        diff.sort();

        // Then
        let expected: Vec<Folder> = vec!["b".to_string(), "d".to_string(), "e".to_string()]
            .into_iter()
            .collect();

        assert_eq!(diff, expected)
    }
}
