// Inspired by https://github.com/paradigmxyz/reth/tree/main/testing/ef-tests

use super::{result::CaseResult, BlockchainTestTransaction};
use crate::{
    constants::FORK_NAME,
    traits::Case,
    utils::io::{deserialize_into, load_file},
};
use async_trait::async_trait;
use ef_tests::models::BlockchainTest;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub struct BlockchainTestCase {
    pub tests: BlockchainTest,
    pub transaction: BlockchainTestTransaction,
}

#[async_trait]
impl Case for BlockchainTestCase {
    /// Load a test case from a path. This is a path to a directory containing
    /// the BlockChainTest
    fn load(path: &Path) -> Result<Self, ef_tests::Error> {
        let general_state_tests_path = path
            .components()
            .filter(|x| !x.as_os_str().eq_ignore_ascii_case("BlockchainTests"))
            .collect::<PathBuf>();
        let test_name = general_state_tests_path
            .file_stem()
            .ok_or_else(|| ef_tests::Error::Io {
                path: path.into(),
                error: "expected file".into(),
            })?
            .to_str()
            .expect("filename contains no unicode charaters");

        let general_state_tests_path = general_state_tests_path.as_path();
        Ok(Self {
            tests: {
                let s = load_file(path)?;
                let mut cases: BTreeMap<String, BlockchainTest> = deserialize_into(s, path)?;
                let test_name = format!("{}{}", test_name, FORK_NAME);

                cases
                    .remove(&test_name)
                    .ok_or_else(|| ef_tests::Error::CouldNotDeserialize {
                        path: path.into(),
                        error: format!("could not find test {}", test_name),
                    })?
            },
            transaction: {
                let s = load_file(general_state_tests_path)?;
                let test: BTreeMap<String, serde_json::Value> =
                    deserialize_into(s, general_state_tests_path)?;

                let case = test
                    .into_values()
                    .collect::<Vec<_>>()
                    .first()
                    .ok_or_else(|| ef_tests::Error::CouldNotDeserialize {
                        path: general_state_tests_path.into(),
                        error: "missing test entry for suite".into(),
                    })?
                    .clone();

                deserialize_into(case.to_string(), general_state_tests_path)?
            },
        })
    }

    async fn init(self) -> Result<Self, ef_tests::Error> {
        todo!()
    }

    async fn run(&self) -> Result<(), ef_tests::Error> {
        todo!()
    }
}

/// A container for multiple test cases.
#[derive(Debug)]
pub struct Cases<T> {
    /// The contained test cases and the path to each test.
    pub test_cases: Vec<(PathBuf, T)>,
}

impl<T: Case> Cases<T> {
    /// Run the contained test cases.
    pub async fn run(&self) -> Vec<CaseResult> {
        let mut results: Vec<CaseResult> = Vec::new();
        for (path, case) in self.test_cases.iter() {
            results.push(CaseResult::new(path, case, case.run().await))
        }
        results
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
        assert!(!case.tests.pre.is_empty());
        assert!(case.transaction.transaction.secret_key != B256::zero());
    }
}
