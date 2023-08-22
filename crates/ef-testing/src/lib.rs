pub mod constants;
pub mod models;
pub mod storage;
pub mod traits;
pub mod utils;

use bytes::BytesMut;
use kakarot_rpc_core::client::constants::CHAIN_ID;
use reth_primitives::{sign_message, Bytes, SealedBlock, Signature, Transaction};
use reth_rlp::Decodable;
use revm_primitives::B256;

/// Sign a transaction given a private key and a chain id.
pub fn sign_tx_with_chain_id(
    tx: &mut Transaction,
    private_key: &B256,
    chain_id: u64,
) -> Result<Signature, eyre::Error> {
    tx.set_chain_id(chain_id);
    let signature = sign_message(*private_key, tx.signature_hash())?;
    Ok(signature)
}

pub fn get_signed_rlp_encoded_transaction(
    block: Bytes,
    pk: B256,
) -> Result<Bytes, ef_tests::Error> {
    // parse it as a sealed block
    let mut block =
        SealedBlock::decode(&mut block.as_ref()).map_err(ef_tests::Error::RlpDecodeError)?;

    // encode body as transaction
    let mut out = BytesMut::new();
    let tx_signed = block.body.get_mut(0).unwrap();

    let tx = &mut tx_signed.transaction;
    tx.set_chain_id(CHAIN_ID);
    let signature = sign_tx_with_chain_id(tx, &pk, CHAIN_ID).unwrap();
    tx_signed.encode_with_signature(&signature, &mut out, true);

    Ok(out.to_vec().into())
}

#[cfg(test)]
mod tests {

    use crate::models::{BlockchainTest, BlockchainTestTransaction};
    use crate::storage::eoa::get_eoa_class_hash;
    use crate::storage::{read_balance, write_test_state, ClassHashes};

    use super::*;
    use std::collections::HashMap;
    use std::sync::Arc;

    use ctor::ctor;
    use ef_tests::models::RootOrState;
    use kakarot_rpc_core::client::api::{KakarotEthApi, KakarotStarknetApi};
    use kakarot_rpc_core::client::helpers::split_u256_into_field_elements;
    use kakarot_rpc_core::models::felt::Felt252Wrapper;
    use kakarot_rpc_core::test_utils::deploy_helpers::KakarotTestEnvironmentContext;
    use kakarot_rpc_core::test_utils::fixtures::kakarot_test_env_ctx;
    use rstest::rstest;
    use starknet::core::types::FieldElement;
    use starknet::providers::Provider;
    use starknet_api::core::{ContractAddress as StarknetContractAddress, Nonce};
    use starknet_api::hash::StarkFelt;
    use tracing_subscriber::FmtSubscriber;

    use crate::utils::starknet::get_starknet_storage_key;
    use hive_utils::kakarot::compute_starknet_address;

    #[ctor]
    fn setup() {
        let subscriber = FmtSubscriber::builder()
            .with_max_level(tracing::Level::INFO)
            .finish();
        tracing::subscriber::set_global_default(subscriber)
            .expect("setting tracing default failed");
    }

    #[rstest]
    #[tokio::test(flavor = "multi_thread")]
    async fn test_generalstatetransition_mvp(kakarot_test_env_ctx: KakarotTestEnvironmentContext) {
        // single case of a GenerateStateTest (block chain state + normal)
        let data_blockchain_test = include_str!(
            "../test_data/BlockchainTests/GeneralStateTests/VmTests/vmArithmeticTest/add.json"
        );
        let data_general_state_test =
            include_str!("../test_data/GeneralStateTests/VmTests/vmArithmeticTest/add.json");
        let test_case = String::from("add");
        let fork = String::from("_d0g0v0_Shanghai");

        // parse it as value, for now
        let bt: HashMap<String, serde_json::Value> =
            serde_json::from_str(data_blockchain_test).expect("Failed to parse JSON");
        let bt: BlockchainTest =
            serde_json::from_value(bt.get(&(test_case.clone() + &fork)).unwrap().to_owned())
                .unwrap();

        let gt: HashMap<String, serde_json::Value> =
            serde_json::from_str(data_general_state_test).expect("Failed to parse JSON");
        let gt: BlockchainTestTransaction =
            serde_json::from_value(gt.get(&test_case).unwrap().to_owned()).unwrap();

        // Given
        let test_environment = Arc::new(kakarot_test_env_ctx);
        let starknet_client = test_environment.client().starknet_provider();
        let kakarot_address = test_environment.kakarot().kakarot_address;
        let proxy_class_hash = test_environment.kakarot().proxy_class_hash;
        let contract_account_class_hash = test_environment.kakarot().contract_account_class_hash;

        // Create an atomic reference to the test environment to avoid dropping it
        let env = Arc::clone(&test_environment);

        let bt_binding = bt.clone();

        // It is not possible to block the async test task, so we need to spawn a blocking task
        tokio::task::spawn_blocking(move || {
            // Get lock on the Starknet sequencer
            let mut starknet = env.sequencer().sequencer.backend.state.blocking_write();

            // deriving the eao class hash this way so things are always based off the katana dump file
            let eoa_class_hash: FieldElement = get_eoa_class_hash(env.clone(), &starknet).unwrap();

            // iterate through pre-state addresses
            write_test_state(
                &bt_binding,
                kakarot_address,
                ClassHashes::new(
                    proxy_class_hash,
                    eoa_class_hash,
                    contract_account_class_hash,
                ),
                &mut starknet,
            )
            .expect("failed to write prestate for test");
        })
        .await
        .unwrap();

        // necessary to have our updated state actually applied to transaction
        test_environment
            .sequencer()
            .sequencer
            .backend
            .generate_latest_block()
            .await;
        test_environment
            .sequencer()
            .sequencer
            .backend
            .generate_pending_block()
            .await;

        // each test is essentually one block that has one transaction
        let temp_value = bt.clone();
        let block_rlp_bytes = temp_value.blocks.get(0).unwrap().rlp.clone();
        let tx_encoded =
            get_signed_rlp_encoded_transaction(block_rlp_bytes, gt.transaction.secret_key).unwrap();

        // execute transaction in block
        let client = test_environment.client();
        let hash = client
            .send_transaction(tx_encoded.to_vec().into())
            .await
            .unwrap();

        let transaction_hash: FieldElement = FieldElement::from_bytes_be(&hash).unwrap();
        let _ = starknet_client
            .get_transaction_receipt::<FieldElement>(transaction_hash)
            .await
            .expect("transaction has receipt");

        let binding = bt.clone();
        let env = Arc::clone(&test_environment);
        // assert on post state
        tokio::task::spawn_blocking(move || {
            let post_state = match binding.post_state.as_ref().unwrap() {
                RootOrState::Root(_) => panic!("RootOrState::Root(_) not supported"),
                RootOrState::State(state) => state,
            };

            // Get lock on the Starknet sequencer
            let starknet = env.sequencer().sequencer.backend.state.blocking_read();

            for (address, expected_state) in post_state.iter() {
                let address_: FieldElement = Felt252Wrapper::from(*address).into();
                let starknet_address =
                    compute_starknet_address(kakarot_address, proxy_class_hash, address_);
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

                let _actual_account_balance = read_balance(starknet_address, &starknet).unwrap();

                // we don't presume gas equivalence
                // assert_eq!(actual_account_balance, StarkFelt::from(expected_account_balance));

                assert_eq!(actual_nonce, StarkFelt::from(account_nonce));

                expected_state.storage.iter().for_each(|(key, value)| {
                    let keys = split_u256_into_field_elements(key.0);

                    let expected_state_values = split_u256_into_field_elements(value.0);
                    expected_state_values
                        .iter()
                        .enumerate()
                        .for_each(|(offset, value)| {
                            let stark_key =
                                get_starknet_storage_key("storage_", &keys, offset as u64);

                            let actual_state_value = *actual_state.storage.get(&stark_key).unwrap();
                            assert_eq!(actual_state_value, StarkFelt::from(*value));
                        });
                })
            }
        })
        .await
        .unwrap();
    }
}
