// Inspired by https://github.com/paradigmxyz/reth/tree/main/testing/ef-tests

use super::{result::CaseResult, BlockchainTestTransaction};
use crate::{
    get_signed_rlp_encoded_transaction,
    storage::{eoa::get_eoa_class_hash, write_test_state, ClassHashes},
    traits::Case,
    utils::{
        assert::assert_contract_post_state,
        io::{deserialize_into, load_file},
    },
};
use async_trait::async_trait;
use ef_tests::models::BlockchainTest;
use ef_tests::models::{ForkSpec, RootOrState, State};
use hive_utils::kakarot::compute_starknet_address;
use kakarot_rpc_core::{
    client::api::{KakarotEthApi, KakarotStarknetApi},
    models::felt::Felt252Wrapper,
    test_utils::deploy_helpers::{DeployedKakarot, KakarotTestEnvironmentContext},
};
use starknet::{core::types::FieldElement, providers::Provider};
use starknet_api::{core::ContractAddress as StarknetContractAddress, hash::StarkFelt};
use std::{
    collections::{BTreeMap, HashSet},
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub struct BlockchainTestCase {
    pub name: String,
    pub tests: BTreeMap<String, BlockchainTest>,
    pub transaction: BlockchainTestTransaction,
}

async fn handle_pre_state(
    kakarot: &DeployedKakarot,
    env: &KakarotTestEnvironmentContext,
    pre_state: &State,
) -> Result<(), ef_tests::Error> {
    let kakarot_address = kakarot.kakarot_address;

    let mut starknet = env.sequencer().sequencer.backend.state.write().await;

    let eoa_class_hash = get_eoa_class_hash(env, &starknet).expect("failed to get eoa class hash");
    let class_hashes = ClassHashes::new(
        kakarot.proxy_class_hash,
        eoa_class_hash,
        kakarot.contract_account_class_hash,
    );
    write_test_state(pre_state, kakarot_address, class_hashes, &mut starknet)?;
    Ok(())
}

// division of logic:
//// 'handle' methods attempt to abstract the data coming from BlockChainTestCase
//// from more general logic that can be used across tests
impl BlockchainTestCase {
    fn test(&self, test_name: &str) -> Result<&BlockchainTest, ef_tests::Error> {
        let test = self.tests.get(test_name).ok_or_else(|| {
            ef_tests::Error::Assertion(format!("case {} doesn't exist in test file", test_name))
        })?;
        Ok(test)
    }

    async fn handle_pre_state(
        &self,
        env: &KakarotTestEnvironmentContext,
        test_case_name: &str,
    ) -> Result<(), ef_tests::Error> {
        let test = self.test(test_case_name)?;

        let kakarot = env.kakarot();
        handle_pre_state(kakarot, env, &test.pre).await?;

        Ok(())
    }

    async fn handle_transaction(
        &self,
        env: &KakarotTestEnvironmentContext,
        test_case_name: &str,
    ) -> Result<(), ef_tests::Error> {
        let test = self.test(test_case_name)?;

        // we extract the transaction from the block
        let block = test
            .blocks
            .first()
            .ok_or(ef_tests::Error::Assertion("test has no blocks".to_string()))?
            .clone();
        // we adjust the rlp to correspond with our currently hardcoded CHAIN_ID
        let tx_encoded = get_signed_rlp_encoded_transaction(
            &block.rlp,
            self.transaction.transaction.secret_key,
        )?;

        let client = env.client();
        let hash = client
            .send_transaction(tx_encoded.to_vec().into())
            .await
            .map_err(|err| ef_tests::Error::Assertion(err.to_string()))?;

        // we make sure that the transaction has a receipt and fail fast if it doesn't
        let starknet_provider = env.client().starknet_provider();
        let transaction_hash: FieldElement = FieldElement::from_bytes_be(&hash).unwrap();
        starknet_provider
            .get_transaction_receipt::<FieldElement>(transaction_hash)
            .await
            .map_err(|err| ef_tests::Error::Assertion(err.to_string()))?;

        Ok(())
    }

    async fn handle_post_state(
        &self,
        env: &KakarotTestEnvironmentContext,
        test_case_name: &str,
    ) -> Result<(), ef_tests::Error> {
        let test = self.test(test_case_name)?;

        let post_state = match test.post_state.as_ref().ok_or_else(|| {
            ef_tests::Error::Assertion(format!(
                "failed test {}: missing post state",
                test_case_name
            ))
        })? {
            RootOrState::Root(_) => panic!("RootOrState::Root(_) not supported"),
            RootOrState::State(state) => state,
        };

        let kakarot = env.kakarot();
        let kakarot_address = kakarot.kakarot_address;

        // Get lock on the Starknet sequencer
        let starknet = env.sequencer().sequencer.backend.state.read().await;

        for (evm_address, expected_state) in post_state.iter() {
            let addr: FieldElement = Felt252Wrapper::from(*evm_address).into();
            let starknet_address =
                compute_starknet_address(kakarot_address, kakarot.proxy_class_hash, addr);
            let address = StarknetContractAddress(
                Into::<StarkFelt>::into(starknet_address)
                    .try_into()
                    .unwrap(),
            );

            let actual_state = starknet.storage.get(&address).ok_or_else(|| {
                ef_tests::Error::Assertion(format!(
                    "failed test {}: missing evm address {:?} in post state storage",
                    test_case_name, evm_address
                ))
            })?;

            assert_contract_post_state(test_case_name, expected_state, actual_state)?;
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
            .ok_or_else(|| ef_tests::Error::Io {
                path: path.into(),
                error: format!("expected valid utf8 path, got {:?}", path),
            })?;

        let general_state_tests_path = general_state_tests_path.as_path();
        Ok(Self {
            tests: {
                let s = load_file(path)?;
                deserialize_into(s, path)?
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
            name: test_name.to_string(),
        })
    }

    async fn run(&self) -> Result<(), ef_tests::Error> {
        let specific_tests_to_run: Option<HashSet<String>> = std::env::var("RUN_SPECIFIC_TESTS")
            .ok()
            .map(|test_names| test_names.split(',').map(String::from).collect());

        for (test_name, case) in self.tests.iter() {
            if matches!(case.network, ForkSpec::Shanghai) {
                if let Some(ref specific_tests) = specific_tests_to_run {
                    if !specific_tests.contains(test_name) {
                        continue;
                    }
                }
                let env = KakarotTestEnvironmentContext::from_dump_state().await;
                // handle pretest
                self.handle_pre_state(&env, test_name).await?;

                // necessary to have our updated state actually applied to transaction
                // think of it as 'burping' the sequencer
                env.sequencer()
                    .sequencer
                    .backend
                    .generate_latest_block()
                    .await;
                env.sequencer()
                    .sequencer
                    .backend
                    .generate_pending_block()
                    .await;

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
        for (path, case) in self.test_cases.iter() {
            results.push(CaseResult::new(path, case, case.run().await))
        }
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ctor::ctor;
    use revm_primitives::B256;
    use tracing_subscriber::FmtSubscriber;

    #[ctor]
    fn setup() {
        // Change this to ERROR to see less output.
        let subscriber = FmtSubscriber::builder()
            .with_max_level(tracing::Level::INFO)
            .finish();
        tracing::subscriber::set_global_default(subscriber)
            .expect("setting tracing default failed");
    }

    #[tokio::test]
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

    #[tokio::test]
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

    #[tokio::test]
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
}
