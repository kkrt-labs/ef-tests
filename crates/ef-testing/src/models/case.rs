// Inspired by https://github.com/paradigmxyz/reth/tree/main/testing/ef-tests

use super::{result::CaseResult, BlockchainTest, BlockchainTestTransaction};
use crate::{
    constants::FORK_NAME,
    get_signed_rlp_encoded_transaction,
    storage::{eoa::get_eoa_class_hash, write_test_state, ClassHashes},
    traits::Case,
    utils::{
        assert::assert_contract_post_state,
        io::{deserialize_into, load_file},
    },
};
use async_trait::async_trait;
use ef_tests::models::{RootOrState, State};
use hive_utils::kakarot::compute_starknet_address;
use kakarot_rpc_core::{
    client::api::{KakarotEthApi, KakarotStarknetApi},
    models::felt::Felt252Wrapper,
    test_utils::deploy_helpers::{DeployedKakarot, KakarotTestEnvironmentContext},
};
use starknet::{core::types::FieldElement, providers::Provider};
use starknet_api::{core::ContractAddress as StarknetContractAddress, hash::StarkFelt};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Debug)]
pub struct BlockchainTestCase {
    pub name: String,
    pub tests: BlockchainTest,
    pub transaction: BlockchainTestTransaction,
}

async fn handle_pre_state(
    kakarot: &DeployedKakarot,
    env: &Arc<KakarotTestEnvironmentContext>,
    pre_state: &State,
) -> Result<(), ef_tests::Error> {
    let kakarot_address = kakarot.kakarot_address;

    let mut starknet = env.sequencer().sequencer.backend.state.write().await;

    let eoa_class_hash =
        get_eoa_class_hash(env.clone(), &starknet).expect("failed to get eoa class hash");
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
    async fn handle_pre_state(
        &self,
        env: &Arc<KakarotTestEnvironmentContext>,
    ) -> Result<(), ef_tests::Error> {
        let test = &self.tests;

        let kakarot = env.kakarot();
        handle_pre_state(kakarot, env, &test.pre).await?;

        Ok(())
    }

    async fn handle_transaction(
        &self,
        env: &Arc<KakarotTestEnvironmentContext>,
    ) -> Result<(), ef_tests::Error> {
        let test = &self.tests;

        // we extract the transaction from the block
        let block = test
            .blocks
            .first()
            .ok_or(ef_tests::Error::Assertion("test has no blocks".to_string()))?
            .clone();
        // we adjust the rlp to correspond with our currently hardcoded CHAIN_ID
        let tx_encoded =
            get_signed_rlp_encoded_transaction(block.rlp, self.transaction.transaction.secret_key)?;

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
        env: &Arc<KakarotTestEnvironmentContext>,
    ) -> Result<(), ef_tests::Error> {
        let test = &self.tests;
        let post_state = match test.post_state.as_ref().ok_or_else(|| {
            ef_tests::Error::Assertion(format!("failed test {}: missing post state", self.name))
        })? {
            RootOrState::Root(_) => panic!("RootOrState::Root(_) not supported"),
            RootOrState::State(state) => state,
        };

        let kakarot = env.kakarot();
        let kakarot_address = kakarot.kakarot_address;

        // Get lock on the Starknet sequencer
        let starknet = env.sequencer().sequencer.backend.state.read().await;

        for (address, expected_state) in post_state.iter() {
            let addr: FieldElement = Felt252Wrapper::from(*address).into();
            let starknet_address =
                compute_starknet_address(kakarot_address, kakarot.proxy_class_hash, addr);
            let address = StarknetContractAddress(
                Into::<StarkFelt>::into(starknet_address)
                    .try_into()
                    .unwrap(),
            );

            let actual_state = starknet.storage.get(&address).unwrap();

            assert_contract_post_state(&self.name, expected_state, actual_state)?;
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
            name: test_name.to_string(),
        })
    }

    async fn run(&self) -> Result<(), ef_tests::Error> {
        let env = Arc::new(KakarotTestEnvironmentContext::from_dump_state().await);
        // handle pretest
        self.handle_pre_state(&env).await?;

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
        self.handle_transaction(&env).await?;

        // handle post state
        self.handle_post_state(&env).await?;

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
    use revm_primitives::B256;

    use super::*;

    #[tokio::test]
    async fn test_load_case() {
        // Given
        let path = Path::new(
            "test_data/BlockchainTests/GeneralStateTests/VMTests/vmArithmeticTest/add.json",
        );

        // When
        let case = BlockchainTestCase::load(path).unwrap();

        // Then
        assert!(!case.tests.pre.is_empty());
        assert!(case.transaction.transaction.secret_key != B256::zero());

        case.run().await.unwrap();
    }
}
