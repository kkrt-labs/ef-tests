// Inspired by https://github.com/paradigmxyz/reth/tree/main/testing/ef-tests

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};
use walkdir::{DirEntry, WalkDir};

use ef_tests::{models::BlockchainTest, Case, Cases, Suite};

use super::{result::assert_tests_pass, BlockchainTestTransaction};

pub struct BlockchainTestSuite {
    pub name: String,
}

impl BlockchainTestSuite {
    pub fn new(name: String) -> Self {
        Self { name }
    }
}

impl Suite for BlockchainTestSuite {
    type Case = BlockchainTestCase;

    fn suite_name(&self) -> String {
        format!("BlockchainTests/{}", self.name)
    }

    /// Load and run all test cases for the suite
    /// Copied from https://github.com/paradigmxyz/reth/blob/main/testing/ef-tests/src/suite.rs
    /// because env!("CARGO_MANIFEST_DIR") causes the path to be in ~/.cargo/checkouts/reth-...
    fn run(&self) {
        let suite_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("ethereum-tests")
            .join(self.suite_name());

        let test_cases = find_all_files_with_extension(&suite_path, ".json")
            .into_iter()
            .map(|test_case_path| {
                let case = Self::Case::load(&test_case_path).expect("test case should load");
                (test_case_path, case)
            })
            .collect();

        let results = Cases { test_cases }.run();

        assert_tests_pass(&self.suite_name(), &suite_path, &results);
    }
}

/// Recursively find all files with a given extension.
fn find_all_files_with_extension(path: &Path, extension: &str) -> Vec<PathBuf> {
    WalkDir::new(path)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().ends_with(extension))
        .map(DirEntry::into_path)
        .collect::<_>()
}

#[derive(Debug)]
pub struct BlockchainTestCase {
    pub tests: BTreeMap<String, BlockchainTest>,
    pub transaction: BlockchainTestTransaction,
}

impl Case for BlockchainTestCase {
    /// Load a test case from a path. This is a path to a directory containing
    /// the BlockChainTest
    fn load(path: &Path) -> Result<Self, ef_tests::Error> {
        let general_state_tests_path = path
            .components()
            .filter(|x| !x.as_os_str().eq_ignore_ascii_case("BlockchainTests"))
            .collect::<PathBuf>();
        let general_state_tests_path = general_state_tests_path.as_path();
        Ok(Self {
            tests: {
                let s = fs::read_to_string(path).map_err(|error| ef_tests::Error::Io {
                    path: path.into(),
                    error: error.to_string(),
                })?;
                serde_json::from_str(&s).map_err(|error| ef_tests::Error::CouldNotDeserialize {
                    path: path.into(),
                    error: error.to_string(),
                })?
            },
            transaction: {
                let s = fs::read_to_string(general_state_tests_path).map_err(|error| {
                    ef_tests::Error::Io {
                        path: general_state_tests_path.into(),
                        error: error.to_string(),
                    }
                })?;
                let test: BTreeMap<String, serde_json::Value> =
                    serde_json::from_str(&s).map_err(|error| {
                        ef_tests::Error::CouldNotDeserialize {
                            path: general_state_tests_path.into(),
                            error: error.to_string(),
                        }
                    })?;
                let case = test
                    .into_values()
                    .collect::<Vec<_>>()
                    .first()
                    .ok_or_else(|| ef_tests::Error::CouldNotDeserialize {
                        path: general_state_tests_path.into(),
                        error: "missing test entry for suite".into(),
                    })?
                    .clone();
                serde_json::from_value(case).map_err(|err| {
                    ef_tests::Error::CouldNotDeserialize {
                        path: general_state_tests_path.into(),
                        error: err.to_string(),
                    }
                })?
            },
        })
    }

    fn run(&self) -> Result<(), ef_tests::Error> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use revm_primitives::B256;

    use super::*;

    #[test]
    fn test_load_case() {
        // Given
        let path = Path::new(
            "test_data/BlockchainTests/GeneralStateTests/VMTests/vmArithmeticTest/add.json",
        );

        // When
        let case = BlockchainTestCase::load(path).unwrap();

        // Then
        assert!(!case.tests.is_empty());
        assert!(case.transaction.transaction.secret_key != B256::zero());
    }
}
