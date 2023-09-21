// Inspired by https://github.com/paradigmxyz/reth/tree/main/testing/ef-tests

use super::{error::RunnerError, result::CaseResult, BlockchainTestTransaction};
use crate::{
    get_signed_rlp_encoded_transaction,
    storage::{
        eoa::get_eoa_class_hash, fee_token::read_balance, models::ClassHashes, write_test_state,
    },
    traits::Case,
    utils::{
        assert::{assert_contract_post_state, assert_empty_post_state},
        io::{deserialize_into, load_file},
    },
};
use async_trait::async_trait;
use ef_tests::models::BlockchainTest;
use ef_tests::models::{ForkSpec, RootOrState, State};
use kakarot_rpc_core::{client::api::KakarotEthApi, models::felt::Felt252Wrapper};
use kakarot_test_utils::deploy_helpers::{DeployedKakarot, KakarotTestEnvironmentContext};
use kakarot_test_utils::hive_utils::kakarot::compute_starknet_address;

use regex::Regex;
use starknet::core::types::FieldElement;
use starknet_api::{core::ContractAddress as StarknetContractAddress, hash::StarkFelt};
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

async fn handle_pre_state(
    kakarot: &DeployedKakarot,
    env: &KakarotTestEnvironmentContext,
    pre_state: &State,
) -> Result<(), RunnerError> {
    let kakarot_address = kakarot.kakarot_address;

    let mut starknet = env.sequencer().sequencer.backend.state.write().await;
    let starknet_db = starknet
        .maybe_as_cached_db()
        .ok_or_else(|| RunnerError::SequencerError("failed to get Katana database".to_string()))?;

    let eoa_class_hash = get_eoa_class_hash(env, &starknet_db)?;
    let class_hashes = ClassHashes::new(
        kakarot.proxy_class_hash,
        eoa_class_hash,
        kakarot.contract_account_class_hash,
    );
    write_test_state(pre_state, kakarot_address, class_hashes, &mut starknet)?;
    Ok(())
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
        let name = path.file_name().unwrap().to_str().unwrap();

        matches!(
            name,
            "calldatacopy.json" // ef-tests #20
                | "sha3.json" // ef-tests #19
                | "exp.json" // kakarot #700
                | "expPower2.json" // kakarot #700
                | "expPower256.json" // kakarot #700
                | "expPower256Of256.json" // kakarot #700
                | "twoOps.json" // kakarot #700
                | "addmod.json" // kakarot #695
                | "mulmod.json" // kakarot #695
                | "divByZero.json" // kakarot #695
                | "jumpi.json" // kakarot #693
                | "jump.json" // ef-tests #38
                | "jumpToPush.json" // ef-tests #61
                | "signextend.json" // kakarot #677
                | "mload.json" // ef-tests #31
                | "gas.json" // ef-tests #36
                | "loopExp.json" // ef-tests #39
                | "loopMul.json" // ef-tests #39
                | "performanceTester.json" // ef-tests #39
                | "suicide.json" // ef-tests #57
                | "blockInfo.json" // ef-tests #67
                | "envInfo.json" // ef-tests #63
                | "memCopySelf.json" // ef-tests #52
                | "bufferSrcOffset.json" // ef-tests #51
                | "buffer.json" // ef-tests #50
                | "oog.json" // ef-tests #49
                | "sloadGasCost.json" // ef-tests #78
                | "TransactionCreateStopInInitcode.json" // ef-tests #108
                | "CallRecursiveContract.json" // ef-tests #109
                | "CallContractToCreateContractWhichWouldCreateContractInInitCode.json" // ef-tests #110
                | "CallContractToCreateContractOOGBonusGas.json" // ef-tests #111
                | "OutOfGasPrefundedContractCreation.json" // ef-test #112
                | "CallContractToCreateContractWhichWouldCreateContractIfCalled.json" // ef-test #114
                | "CallTheContractToCreateEmptyContract.json" // ef-test #115
                | "OutOfGasContractCreation.json" // ef-test #116
                | "CallContractToCreateContractAndCallItOOG.json" // ef-test #117
                | "TestContractSuicide.json" // ef-test #132
                | "TestCryptographicFunctions.json" // ef-test #133
                | "RecursiveCreateContracts.json" // ef-test #134
                | "ByZero.json" //ef-test #135
                | "ContractInheritance.json" // ef-test #136
                | "CallLowLevelCreatesSolidity.json" // ef-test #137
                | "CreateContractFromMethod.json" // ef-test #138
                | "TestStoreGasPrices.json" // ef-test #139
                | "TestContractInteraction.json" // ef-test #140
                | "RecursiveCreateContractsCreate4Contracts.json" // ef-test #141
        )
    }

    fn test(&self, test_name: &str) -> Result<&BlockchainTest, RunnerError> {
        let test = self.tests.get(test_name).ok_or_else(|| {
            RunnerError::Other(format!("case {} doesn't exist in test file", test_name))
        })?;
        Ok(test)
    }

    async fn handle_pre_state(
        &self,
        env: &KakarotTestEnvironmentContext,
        test_case_name: &str,
    ) -> Result<(), RunnerError> {
        let test = self.test(test_case_name)?;

        let kakarot = env.kakarot();
        handle_pre_state(kakarot, env, &test.pre).await?;

        Ok(())
    }

    async fn handle_transaction(
        &self,
        env: &KakarotTestEnvironmentContext,
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

        let client = env.client();
        // Send the transaction without checking for errors, accounting
        // for the fact that some transactions might fail.
        let _ = client.send_transaction(tx_encoded).await;

        Ok(())
    }

    async fn handle_post_state(
        &self,
        env: &KakarotTestEnvironmentContext,
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

        let kakarot = env.kakarot();
        let kakarot_address = kakarot.kakarot_address;

        // Get lock on the Starknet sequencer
        let mut starknet = env.sequencer().sequencer.backend.state.write().await;
        let starknet_db = starknet.maybe_as_cached_db().ok_or_else(|| {
            RunnerError::SequencerError("failed to get Katana database".to_string())
        })?;

        for (evm_address, expected_state) in post_state.iter() {
            let addr: FieldElement = Felt252Wrapper::from(*evm_address).into();
            let starknet_address =
                compute_starknet_address(kakarot_address, kakarot.proxy_class_hash, addr);
            let starknet_contract_address =
                StarknetContractAddress(Into::<StarkFelt>::into(starknet_address).try_into()?);

            let actual_state = starknet_db.storage.get(&starknet_contract_address);
            match actual_state {
                None => {
                    // if no state, check post state is empty
                    let actual_balance = read_balance(evm_address, starknet_address, &mut starknet)
                        .map_err(|err| {
                            RunnerError::Assertion(format!("{} {}", test_case_name, err))
                        })?;
                    assert_empty_post_state(test_case_name, expected_state, actual_balance)?;
                    continue;
                }
                Some(state) => {
                    assert_contract_post_state(test_case_name, evm_address, expected_state, state)?;
                }
            };
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

                tracing::info!("Running test {}", test_name);

                let with_dumped_state = true;
                let env = KakarotTestEnvironmentContext::new(with_dumped_state).await;
                // handle pretest
                self.handle_pre_state(&env, test_name).await?;

                // necessary to have our updated state actually applied to transaction
                // think of it as 'burping' the sequencer
                env.sequencer().sequencer.backend.mine_empty_block().await;

                // handle transaction
                self.handle_transaction(&env, test_name).await?;

                // handle post state
                self.handle_post_state(&env, test_name).await?;
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
        let filter = filter::EnvFilter::new("ef_testing=info");
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
            "ethereum-tests/BlockchainTests/GeneralStateTests/VMTests/vmArithmeticTest/calldatacopy.json",
        );

        // When
        let should_skip = BlockchainTestCase::should_skip(path);

        // Then
        assert!(should_skip);
    }
}
