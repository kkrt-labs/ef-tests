// Inspired by https://github.com/paradigmxyz/reth/tree/main/testing/ef-tests

use super::{error::RunnerError, result::CaseResult, BlockchainTestTransaction};
use crate::{
    evm_sequencer::{
        constants::CHAIN_ID, evm_state::EvmState, utils::to_broadcasted_starknet_transaction,
        KakarotSequencer,
    },
    get_signed_rlp_encoded_transaction,
    traits::Case,
    utils::{deserialize_into, load_file},
};
use async_trait::async_trait;
use ef_tests::models::BlockchainTest;
use ef_tests::models::{ForkSpec, RootOrState};

use regex::Regex;
use sequencer::{
    execution::Execution, state::State as SequencerState, transaction::StarknetTransaction,
};
use serde::Deserialize;
use starknet::core::types::{BroadcastedTransaction, FieldElement};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub struct BlockchainTestCase {
    pub name: String,
    pub tests: BTreeMap<String, BlockchainTest>,
    pub transaction: BlockchainTestTransaction,
    skip: bool,
}

#[derive(Deserialize)]
pub struct BlockchainTestsSkip {
    pub filename: BTreeMap<String, Vec<String>>,
    pub regex: BTreeMap<String, Vec<String>>,
}

lazy_static::lazy_static! {
    pub static ref SKIP: BlockchainTestsSkip = {
        let skip_file = Path::new("../../blockchain-tests-skip.yml");
        let skip_str = load_file(skip_file).unwrap();

        serde_yaml::from_str(&skip_str).unwrap()
    };
}

// Division of logic:
//// 'handle' methods attempt to abstract the data coming from BlockChainTestCase
//// from more general logic that can be used across tests
impl BlockchainTestCase {
    /// Returns whether a given test should be skipped
    /// # Panics
    ///
    /// Will panic if the file name cannot be stringified.
    #[must_use]
    pub fn should_skip(path: &Path) -> bool {
        let dir = path
            .parent()
            .unwrap()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap();
        let name = path.file_name().unwrap().to_str().unwrap();

        let mut should_skip = false;
        if SKIP.filename.contains_key(dir) {
            should_skip = SKIP
                .filename
                .get(dir)
                .unwrap()
                .iter()
                .any(|filename| filename == name);
        }

        if !should_skip && SKIP.regex.contains_key(dir) {
            should_skip = SKIP
                .regex
                .get(dir)
                .unwrap()
                .iter()
                .any(|regex| Regex::new(regex.as_str()).unwrap().is_match(name));
        }

        should_skip
    }

    fn test(&self, test_name: &str) -> Result<&BlockchainTest, RunnerError> {
        let test = self.tests.get(test_name).ok_or_else(|| {
            RunnerError::Other(format!("case {} doesn't exist in test file", test_name))
        })?;
        Ok(test)
    }

    async fn handle_pre_state(
        &self,
        sequencer: &mut KakarotSequencer,
        test_case_name: &str,
    ) -> Result<(), RunnerError> {
        let test = self.test(test_case_name)?;

        for (address, account) in test.pre.iter() {
            sequencer.setup_account(
                address,
                &account.code,
                account.nonce.0,
                account.storage.iter().map(|(k, v)| (k.0, v.0)).collect(),
            )?;
            sequencer.fund(address, account.balance.0)?;
        }

        Ok(())
    }

    async fn handle_transaction(
        &self,
        sequencer: &mut KakarotSequencer,
        test_case_name: &str,
    ) -> Result<(), RunnerError> {
        let test = self.test(test_case_name)?;

        // we extract the transaction from the block
        let block = test
            .blocks
            .first()
            .ok_or_else(|| RunnerError::Other("test has no blocks".to_string()))?
            .clone();
        // we adjust the rlp to correspond with our currently hardcoded CHAIN_ID
        let tx_encoded = get_signed_rlp_encoded_transaction(
            &block.rlp,
            self.transaction.transaction.secret_key,
        )?;

        let starknet_transaction = StarknetTransaction::new(BroadcastedTransaction::Invoke(
            to_broadcasted_starknet_transaction(&tx_encoded)?,
        ));
        sequencer.execute(
            starknet_transaction
                .try_into_execution_transaction(FieldElement::from(*CHAIN_ID))
                .unwrap(),
        )?;

        Ok(())
    }

    async fn handle_post_state(
        &self,
        sequencer: &mut KakarotSequencer,
        test_case_name: &str,
    ) -> Result<(), RunnerError> {
        let test = self.test(test_case_name)?;

        let post_state = match test.post_state.as_ref().ok_or_else(|| {
            RunnerError::Other(format!("missing post state for {}", test_case_name))
        })? {
            RootOrState::Root(_) => {
                panic!("RootOrState::Root(_) not supported, for {}", test_case_name)
            }
            RootOrState::State(state) => state,
        };

        // TODO we should assert on contract code in order to be sure that created contracts are created with the correct code
        // TODO we should assert that the balance of all accounts but the sender is correct
        for (address, expected_state) in post_state.iter() {
            for (k, v) in expected_state.storage.iter() {
                let actual = sequencer.get_storage_at(address, k.0)?;
                if actual != v.0 {
                    return Err(RunnerError::Other(format!(
                        "storage mismatch for {:#20x} at {:#32x}: expected {:#32x}, got {:#32x}",
                        address, k.0, v.0, actual
                    )));
                }
            }
            let actual = sequencer.get_nonce(address)?;
            if actual != expected_state.nonce.0 {
                return Err(RunnerError::Other(format!(
                    "nonce mismatch for {:#20x}: expected {:#32x}, got {:#32x}",
                    address, expected_state.nonce.0, actual
                )));
            }
        }

        Ok(())
    }
}

#[async_trait]
impl Case for BlockchainTestCase {
    /// A description of the test.
    fn description(&self) -> String {
        self.name.clone()
    }

    /// Load a test case from a path. This is a path to a directory containing
    /// the BlockChainTest
    fn load(path: &Path) -> Result<Self, RunnerError> {
        let general_state_tests_path = path
            .components()
            .filter(|x| !x.as_os_str().eq_ignore_ascii_case("BlockchainTests"))
            .collect::<PathBuf>();
        let test_name = general_state_tests_path
            .file_stem()
            .ok_or(RunnerError::Io {
                path: path.into(),
                error: "expected file".into(),
            })?
            .to_str()
            .ok_or_else(|| RunnerError::Io {
                path: path.into(),
                error: format!("expected valid utf8 path, got {:?}", path),
            })?;

        let general_state_tests_path = general_state_tests_path.as_path();
        Ok(Self {
            tests: {
                let file = load_file(path)?;
                deserialize_into(&file, path)?
            },
            transaction: {
                let file = load_file(general_state_tests_path)?;
                let test: BTreeMap<String, serde_json::Value> =
                    deserialize_into(&file, general_state_tests_path)?;

                let case = test
                    .into_values()
                    .collect::<Vec<_>>()
                    .first()
                    .ok_or_else(|| {
                        RunnerError::Other(format!("Missing transaction for {}", test_name))
                    })?
                    .clone();

                deserialize_into(&case.to_string(), general_state_tests_path)?
            },
            name: test_name.to_string(),
            skip: Self::should_skip(path),
        })
    }

    async fn run(&self) -> Result<(), RunnerError> {
        if self.skip {
            return Err(RunnerError::Skipped);
        }

        let test_regexp: Option<String> = std::env::var("TARGET").ok();
        let test_regexp = match test_regexp {
            Some(x) => Some(Regex::new(x.as_str())?),
            None => None,
        };

        for (test_name, case) in &self.tests {
            if matches!(case.network, ForkSpec::Shanghai) {
                if let Some(ref test_regexp) = test_regexp {
                    if !test_regexp.is_match(test_name) {
                        continue;
                    }
                }

                let sequencer = KakarotSequencer::new(SequencerState::default());
                let mut sequencer = sequencer.initialize()?;

                tracing::info!("Running test {}", test_name);

                self.handle_pre_state(&mut sequencer, test_name).await?;

                // handle transaction
                self.handle_transaction(&mut sequencer, test_name).await?;

                // handle post state
                self.handle_post_state(&mut sequencer, test_name).await?;
            }
        }
        Ok(())
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
        for (path, case) in &self.test_cases {
            results.push(CaseResult::new(path, case, case.run().await));
        }
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ctor::ctor;
    use revm_primitives::B256;
    use tracing_subscriber::{filter, FmtSubscriber};

    #[ctor]
    fn setup() {
        // Change this to "error" to see less output.
        let filter = filter::EnvFilter::new("ef_testing=info,executor=warn,sequencer=warn");
        let subscriber = FmtSubscriber::builder().with_env_filter(filter).finish();
        tracing::subscriber::set_global_default(subscriber)
            .expect("setting tracing default failed");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_load_case() {
        // Given
        let path = Path::new(
            "ethereum-tests/BlockchainTests/GeneralStateTests/VMTests/vmArithmeticTest/add.json",
        );

        // When
        let case = BlockchainTestCase::load(path).unwrap();

        // Then
        assert!(!case.tests.is_empty());
        assert!(case.transaction.transaction.secret_key != B256::zero());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_run_add() {
        // Given
        let path = Path::new(
            "ethereum-tests/BlockchainTests/GeneralStateTests/VMTests/vmArithmeticTest/add.json",
        );

        // When
        let case = BlockchainTestCase::load(path).unwrap();

        // Then
        assert!(!case.tests.is_empty());
        assert!(case.transaction.transaction.secret_key != B256::zero());

        case.run().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_run_mul() {
        // Given
        let path = Path::new(
            "ethereum-tests/BlockchainTests/GeneralStateTests/VMTests/vmArithmeticTest/mul.json",
        );

        // When
        let case = BlockchainTestCase::load(path).unwrap();

        // Then
        assert!(!case.tests.is_empty());
        assert!(case.transaction.transaction.secret_key != B256::zero());

        case.run().await.unwrap();
    }

    #[test]
    fn test_should_skip() {
        // Given
        let path = Path::new(
            "ethereum-tests/BlockchainTests/GeneralStateTests/VMTests/vmArithmeticTest/mulmod.json",
        );

        // When
        let should_skip = BlockchainTestCase::should_skip(path);

        // Then
        assert!(should_skip);
    }
}
