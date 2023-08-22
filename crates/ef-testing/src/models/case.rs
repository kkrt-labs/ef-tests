// Inspired by https://github.com/paradigmxyz/reth/tree/main/testing/ef-tests

use super::{result::CaseResult, BlockchainTest, BlockchainTestTransaction};
use crate::{
    constants::FORK_NAME,
    get_signed_rlp_encoded_transaction,
    storage::{eoa::get_eoa_class_hash, read_balance, write_test_state, ClassHashes},
    traits::Case,
    utils::{
        io::{deserialize_into, load_file},
        starknet::get_starknet_storage_key,
    },
};
use async_trait::async_trait;
use ef_tests::models::RootOrState;
use hive_utils::kakarot::compute_starknet_address;
use kakarot_rpc_core::{
    client::{
        api::{KakarotEthApi, KakarotStarknetApi},
        helpers::split_u256_into_field_elements,
    },
    models::felt::Felt252Wrapper,
    test_utils::deploy_helpers::KakarotTestEnvironmentContext,
};
use starknet::{core::types::FieldElement, providers::Provider};
use starknet_api::{
    core::{ContractAddress as StarknetContractAddress, Nonce},
    hash::StarkFelt,
};
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
        let test = self.tests.clone();

        // Prepare the pre test state
        let env_binding = env.clone();
        let _ = tokio::task::spawn_blocking(move || {
            let kakarot = env_binding.kakarot();
            let kakarot_address = kakarot.kakarot_address;

            // Get lock on the Starknet sequencer
            let mut starknet = env_binding
                .sequencer()
                .sequencer
                .backend
                .state
                .blocking_write();

            let eoa_class_hash = get_eoa_class_hash(env_binding.clone(), &starknet)
                .expect("failed to get eoa class hash");
            let class_hashes = ClassHashes::new(
                kakarot.proxy_class_hash,
                eoa_class_hash,
                kakarot.contract_account_class_hash,
            );
            write_test_state(&test, kakarot_address, class_hashes, &mut starknet)
        })
        .await
        .map_err(|err| ef_tests::Error::Assertion(err.to_string()))?;

        // necessary to have our updated state actually applied to transaction
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

        // Get the encoded transaction
        let test = self.tests.clone();
        let block = test
            .blocks
            .first()
            .ok_or(ef_tests::Error::Assertion("test has no blocks".to_string()))?
            .clone();
        let tx_encoded =
            get_signed_rlp_encoded_transaction(block.rlp, self.transaction.transaction.secret_key)?;
        let client = env.client();
        let hash = client
            .send_transaction(tx_encoded.to_vec().into())
            .await
            .map_err(|err| ef_tests::Error::Assertion(err.to_string()))?;

        // Get the receipt to verify the transaction was executed
        let starknet_provider = env.client().starknet_provider();
        let transaction_hash: FieldElement = FieldElement::from_bytes_be(&hash).unwrap();
        let _ = starknet_provider
            .get_transaction_receipt::<FieldElement>(transaction_hash)
            .await
            .map_err(|err| ef_tests::Error::Assertion(err.to_string()))?;

        // assert on post state
        let env_binding = env.clone();
        let post_state = match test.post_state.as_ref().ok_or_else(|| {
            ef_tests::Error::Assertion(format!("failed test {}: missing post state", self.name))
        })? {
            RootOrState::Root(_) => panic!("RootOrState::Root(_) not supported"),
            RootOrState::State(state) => state,
        };

        let kakarot = env_binding.kakarot();
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
            // is there a more efficient route to do this... lol
            let Nonce(actual_nonce) = actual_state.nonce;
            let account_nonce: FieldElement = Felt252Wrapper::try_from(expected_state.nonce.0)
                .unwrap()
                .into();

            let _expected_account_balance: FieldElement =
                Felt252Wrapper::try_from(expected_state.balance.0)
                    .unwrap()
                    .into();

            let _actual_account_balance = read_balance(starknet_address, &starknet)
                .map_err(|err| ef_tests::Error::Assertion(err.to_string()))?;

            // we don't presume gas equivalence
            // assert_eq!(actual_account_balance, StarkFelt::from(expected_account_balance));

            if actual_nonce != StarkFelt::from(account_nonce) {
                return Err(ef_tests::Error::Assertion(format!(
                    "failed test {}: expected nonce {}, got {}",
                    self.name,
                    account_nonce.to_string(),
                    actual_nonce.to_string()
                )));
            }

            for (key, value) in expected_state.storage.iter() {
                let keys = split_u256_into_field_elements(key.0);

                let expected_state_values = split_u256_into_field_elements(value.0);
                for (offset, value) in expected_state_values.into_iter().enumerate() {
                    let stark_key = get_starknet_storage_key("storage_", &keys, offset as u64);

                    let actual_state_value =
                        *actual_state.storage.get(&stark_key).ok_or_else(|| {
                            ef_tests::Error::Assertion(format!(
                                "failed test {}: missing storage for key {:?}",
                                self.name, stark_key
                            ))
                        })?;

                    if actual_state_value != StarkFelt::from(value) {
                        return Err(ef_tests::Error::Assertion(format!(
                            "failed test {}: expected storage value {}, got {}",
                            self.name,
                            value.to_string(),
                            actual_state_value.to_string()
                        )));
                    }
                }
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
