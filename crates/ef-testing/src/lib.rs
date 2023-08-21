pub mod models;

use std::collections::HashMap;

use hive_utils::types::{ContractAddress, StorageKey, StorageValue};
use reth_primitives::{sign_message, Signature, Transaction};
use revm_primitives::B256;
use starknet_api::{hash::StarkFelt, state::StorageKey as StarknetStorageKey};

fn madara_to_katana_storage(
    source: Vec<((ContractAddress, StorageKey), StorageValue)>,
) -> Vec<(StarknetStorageKey, StarkFelt)> {
    source
        .into_iter()
        .map(|((_, k), v)| {
            let key = StarknetStorageKey(Into::<StarkFelt>::into(k.0).try_into().unwrap());
            let value = Into::<StarkFelt>::into(v.0);
            (key, value)
        })
        .collect()
}

fn write_katana_storage(
    data: Vec<(StarknetStorageKey, StarkFelt)>,
    destination: &mut HashMap<StarknetStorageKey, StarkFelt>,
) {
    for (key, value) in data {
        destination.insert(key, value);
    }
}

fn write_madara_to_katana_storage(
    source: Vec<((ContractAddress, StorageKey), StorageValue)>,
    destination: &mut HashMap<StarknetStorageKey, StarkFelt>,
) {
    let reformatted_data = madara_to_katana_storage(source);
    write_katana_storage(reformatted_data, destination);
}

fn sign_tx_with_chain_id(
    tx: &mut Transaction,
    private_key: &B256,
    chain_id: u64,
) -> Result<Signature, eyre::Error> {
    tx.set_chain_id(chain_id);
    let signature = sign_message(*private_key, tx.signature_hash())?;
    Ok(signature)
}

#[cfg(test)]
mod tests {

    use crate::models::GeneralStateTest;

    use super::*;
    use std::collections::HashMap;
    use std::sync::Arc;

    use bytes::BytesMut;
    use ctor::ctor;
    use ef_tests::models::{BlockchainTest, RootOrState};
    use hive_utils::madara::utils::{
        genesis_fund_starknet_address, genesis_set_bytecode,
        genesis_set_storage_kakarot_contract_account, genesis_set_storage_starknet_contract,
    };
    use kakarot_rpc_core::client::api::{KakarotEthApi, KakarotStarknetApi};
    use kakarot_rpc_core::client::constants::{CHAIN_ID, STARKNET_NATIVE_TOKEN};
    use kakarot_rpc_core::models::felt::Felt252Wrapper;
    use kakarot_rpc_core::test_utils::deploy_helpers::KakarotTestEnvironmentContext;
    use kakarot_rpc_core::test_utils::execution_helpers::execute_tx;
    use kakarot_rpc_core::test_utils::fixtures::kakarot_test_env_ctx;
    use katana_core::backend::state::StorageRecord;
    use reth_primitives::SealedBlock;
    use reth_rlp::Decodable;
    use rstest::rstest;
    use starknet::core::types::FieldElement;
    use starknet::core::utils::get_storage_var_address;
    use starknet::providers::Provider;
    use starknet_api::core::{ClassHash, ContractAddress as StarknetContractAddress, Nonce};
    use starknet_api::hash::StarkFelt;
    use starknet_api::state::StorageKey as StarknetStorageKey;
    use tracing_subscriber::FmtSubscriber;

    use hive_utils::kakarot::compute_starknet_address;

    fn get_starknet_storage_key(var_name: &str, args: &[FieldElement]) -> StarknetStorageKey {
        StarknetStorageKey(
            Into::<StarkFelt>::into(get_storage_var_address(var_name, args).unwrap())
                .try_into()
                .unwrap(),
        )
    }

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
    async fn test_happy_path(kakarot_test_env_ctx: KakarotTestEnvironmentContext) {
        // propped up to give myself logging view into the 'happy path' of how a kakarot transaction is ran
        // Given
        // When
        execute_tx(&kakarot_test_env_ctx, "Counter", "inc", vec![]).await;
    }

    #[rstest]
    #[tokio::test(flavor = "multi_thread")]
    async fn test_generalstatetransition_mvp(kakarot_test_env_ctx: KakarotTestEnvironmentContext) {
        // single case of a GenerateStateTest (block chain state + normal)
        let data_blockchain_test = include_str!("./test_data/blockchain_test.json");
        let data_general_state_test = include_str!("./test_data/general_state_test.json");
        let test_case = String::from("add");
        let fork = String::from("_d0g0v0_Shanghai");

        // parse it as value, for now
        let bt: HashMap<String, serde_json::Value> =
            serde_json::from_str(data_blockchain_test).expect("Failed to parse JSON");
        let bt: Arc<BlockchainTest> = Arc::new(
            serde_json::from_value(bt.get(&(test_case.clone() + &fork)).unwrap().to_owned())
                .unwrap(),
        );

        let gt: HashMap<String, serde_json::Value> =
            serde_json::from_str(data_general_state_test).expect("Failed to parse JSON");
        let gt: GeneralStateTest =
            serde_json::from_value(gt.get(&test_case).unwrap().to_owned()).unwrap();

        // Given
        let test_environment = Arc::new(kakarot_test_env_ctx);
        let starknet_client = test_environment.client().starknet_provider();

        // Create an atomic reference to the test environment to avoid dropping it
        let env = Arc::clone(&test_environment);

        // prop up seed state
        let binding = bt.clone();

        // It is not possible to block the async test task, so we need to spawn a blocking task
        tokio::task::spawn_blocking(move || {
            // Get lock on the Starknet sequencer
            let mut starknet = env.sequencer().sequencer.backend.state.blocking_write();

            let eoa = &env.kakarot().eoa_addresses;
            let eoa_address = StarknetContractAddress(
                Into::<StarkFelt>::into(eoa.starknet_address)
                    .try_into()
                    .unwrap(),
            );

            // deriving the eao class hash this way so things are always based off the katana dump file
            let eoa_class_hash: FieldElement = (*starknet
                .storage
                .get(&eoa_address)
                .unwrap()
                .storage
                .get(&get_starknet_storage_key("_implementation", &[]))
                .unwrap())
            .into();

            // iterate through pre-state addresses
            for (address, account_info) in binding.pre.iter() {
                let mut storage = HashMap::new();
                let kakarot_address = env.kakarot().kakarot_address;
                let address = Felt252Wrapper::from(address.to_owned()).into();
                let address_as_sn_address = compute_starknet_address(
                    kakarot_address,
                    env.kakarot().proxy_class_hash,
                    address,
                );

                // funding balance
                let balance = account_info.balance.0;

                let balance_storage_tuples_madara =
                    genesis_fund_starknet_address(address_as_sn_address, balance);
                let native_token_address = StarknetContractAddress(
                    Into::<StarkFelt>::into(
                        FieldElement::from_hex_be(STARKNET_NATIVE_TOKEN).unwrap(),
                    )
                    .try_into()
                    .unwrap(),
                );
                let katana_storage_tuples = madara_to_katana_storage(balance_storage_tuples_madara);

                for (key, value) in katana_storage_tuples {
                    starknet
                        .storage
                        .entry(native_token_address)
                        .or_default()
                        .storage
                        .insert(key, value);
                }

                // storage
                account_info.storage.iter().for_each(|(key, value)| {
                    // Call genesis_set_storage_kakarot_contract_account util to get the storage tuples
                    let storage_tuples = genesis_set_storage_kakarot_contract_account(
                        address_as_sn_address,
                        key.0,
                        value.0,
                    );
                    write_madara_to_katana_storage(storage_tuples, &mut storage);
                });

                // TODO clean this
                let proxy_implementation_class_hash = match account_info.code.is_empty() {
                    true => {
                        let kakarot_address_key = get_starknet_storage_key("kakarot_address", &[]);
                        let kakarot_address_value =
                            Into::<StarkFelt>::into(StarkFelt::from(kakarot_address));
                        storage.insert(kakarot_address_key, kakarot_address_value);

                        let evm_address_key = get_starknet_storage_key("evm_address", &[]);
                        let evm_address_value = Into::<StarkFelt>::into(StarkFelt::from(address));
                        storage.insert(evm_address_key, evm_address_value);
                        eoa_class_hash
                    }
                    false => {
                        let kakarot_bytes_storage_madara =
                            genesis_set_bytecode(&account_info.code, address_as_sn_address);
                        write_madara_to_katana_storage(kakarot_bytes_storage_madara, &mut storage);

                        let bytecode_len_key = get_starknet_storage_key("bytecode_len_", &[]);
                        let bytecode_len_value = Into::<StarkFelt>::into(StarkFelt::from(
                            account_info.code.len() as u64,
                        ));
                        storage.insert(bytecode_len_key, bytecode_len_value);

                        env.kakarot().contract_account_class_hash
                    }
                };

                // rudimentary way to get a mapping from eth -> starknet address
                dbg!(
                    address,
                    address_as_sn_address,
                    account_info,
                    proxy_implementation_class_hash
                );

                // write implementation state of proxy
                let proxy_implementation_storage_tuples = genesis_set_storage_starknet_contract(
                    address_as_sn_address,
                    "_implementation",
                    &[],
                    proxy_implementation_class_hash,
                    0, // 0 since it's storage value is felt
                );

                write_madara_to_katana_storage(
                    vec![proxy_implementation_storage_tuples],
                    &mut storage,
                );

                // now, finally, we update the sequencer state with the eth->starknet address

                let address = StarknetContractAddress(
                    Into::<StarkFelt>::into(address_as_sn_address)
                        .try_into()
                        .unwrap(),
                );
                let account_nonce: FieldElement = Felt252Wrapper::try_from(account_info.nonce.0)
                    .unwrap()
                    .into();
                let storage_record = StorageRecord {
                    nonce: Nonce(StarkFelt::from(account_nonce)),
                    class_hash: ClassHash(env.kakarot().proxy_class_hash.into()),
                    storage: storage.clone(),
                };
                starknet.storage.insert(address, storage_record);
            }
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

        // parse it as a sealed block
        let mut parsed_block = SealedBlock::decode(&mut block_rlp_bytes.as_ref()).unwrap();
        // encode body as transaction
        let mut encoded_transaction = BytesMut::new();
        let tx_signed = parsed_block.body.get_mut(0).unwrap();
        let copy_tx = tx_signed.clone();
        let pk = gt.transaction.secret_key;
        dbg!(copy_tx.recover_signer());
        let tx = &mut tx_signed.transaction;
        tx.set_chain_id(CHAIN_ID);
        let signature = sign_tx_with_chain_id(tx, &pk, CHAIN_ID).unwrap();
        tx_signed.encode_with_signature(&signature, &mut encoded_transaction, true);

        let evm_address = tx_signed.recover_signer().unwrap();
        dbg!(evm_address);

        // execute transaction in block
        let client = test_environment.client();
        let hash = client
            .send_transaction(encoded_transaction.to_vec().into())
            .await
            .unwrap();

        let transaction_hash: FieldElement = FieldElement::from_bytes_be(&hash).unwrap();
        let receipt = starknet_client
            .get_transaction_receipt::<FieldElement>(transaction_hash)
            .await
            .expect("transaction has receipt");
        dbg!(receipt);

        // assert on post state
        // prop up seed state
        let binding = bt.clone();
        let env = Arc::clone(&test_environment);
        tokio::task::spawn_blocking(move || {
            let post_state = match binding.post_state.as_ref().unwrap() {
                RootOrState::Root(_) => panic!("RootOrState::Root(_) not supported"),
                RootOrState::State(state) => state,
            };

            // Get lock on the Starknet sequencer
            let starknet = env.sequencer().sequencer.backend.state.blocking_read();

            for (address, _) in post_state.iter() {
                let address: FieldElement = Felt252Wrapper::from(*address).into();
                let address_as_sn_address = compute_starknet_address(
                    env.kakarot().kakarot_address,
                    env.kakarot().proxy_class_hash,
                    address,
                );
                let address = StarknetContractAddress(
                    Into::<StarkFelt>::into(address_as_sn_address)
                        .try_into()
                        .unwrap(),
                );
                dbg!(address, starknet.storage.get(&address));
            }
        })
        .await
        .unwrap();
    }
}
