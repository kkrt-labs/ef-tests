use std::collections::HashMap;

use hive_utils::types::{ContractAddress, StorageKey, StorageValue};
use starknet::core::utils::get_storage_var_address;
use starknet_api::{hash::StarkFelt, state::StorageKey as StarknetStorageKey};

fn _madara_to_katana_storage(
    source: Vec<((ContractAddress, StorageKey), StorageValue)>,
    destination: &mut HashMap<StarknetStorageKey, StarkFelt>,
) {
    for ((_, k), v) in source {
        let key = StarknetStorageKey(Into::<StarkFelt>::into(k.0).try_into().unwrap());
        let value = Into::<StarkFelt>::into(v.0);
        destination.insert(key, value);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::str::FromStr;
    use std::sync::Arc;

    use bytes::BytesMut;
    use hive_utils::madara::utils::{
        genesis_fund_starknet_address, genesis_set_bytecode,
        genesis_set_storage_kakarot_contract_account, genesis_set_storage_starknet_contract,
    };
    use kakarot_rpc_core::client::api::{KakarotEthApi, KakarotStarknetApi};
    use kakarot_rpc_core::client::constants::STARKNET_NATIVE_TOKEN;
    use kakarot_rpc_core::client::helpers::split_u256_into_field_elements;
    use kakarot_rpc_core::contracts::account::Account;
    use kakarot_rpc_core::contracts::contract_account::ContractAccount;
    use kakarot_rpc_core::mock::constants::ACCOUNT_ADDRESS;
    use kakarot_rpc_core::test_utils::deploy_helpers::{
        compute_kakarot_contracts_class_hash, KakarotTestEnvironmentContext,
    };
    use kakarot_rpc_core::test_utils::execution_helpers::execute_tx;
    use kakarot_rpc_core::test_utils::fixtures::kakarot_test_env_ctx;
    use katana_core::backend::state::StorageRecord;
    use reth_primitives::{Bytes, SealedBlock, U256};
    use reth_rlp::{Decodable, Encodable};
    use rstest::rstest;
    use starknet::core::types::{BlockId as StarknetBlockId, BlockTag, FieldElement};
    use starknet::providers::Provider;
    use starknet_api::core::{ClassHash, ContractAddress as StarknetContractAddress, Nonce};
    use starknet_api::hash::StarkFelt;
    use starknet_api::state::StorageKey as StarknetStorageKey;

    use crate::*;
    use hive_utils::kakarot::compute_starknet_address;

    fn get_starknet_storage_key(var_name: &str, args: &[FieldElement]) -> StarknetStorageKey {
        StarknetStorageKey(
            Into::<StarkFelt>::into(get_storage_var_address(var_name, args).unwrap())
                .try_into()
                .unwrap(),
        )
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
        // single case of a GenerateStateTest
        let data = r#"
{"add_d0g0v0_Shanghai": {
    "_info": {
      "comment": "Ori Pomerantz qbzzt1@gmail.com",
      "filling-rpc-server": "evm version 1.11.4-unstable-e14043db-20230308",
      "filling-tool-version": "retesteth-0.3.0-shanghai+commit.fd2c0a83.Linux.g++",
      "generatedTestHash": "dc4687b4e526bcd4fe23eac73894cacf8ba5b9a139363de0073eb67db0df36fb",
      "lllcversion": "Version: 0.5.14-develop.2022.7.30+commit.a096d7a9.Linux.g++",
      "solidity": "Version: 0.8.17+commit.8df45f5f.Linux.g++",
      "source": "src/GeneralStateTestsFiller/VMTests/vmArithmeticTest/addFiller.yml",
      "sourceHash": "78afea990a2d534831acc4883b9ff6e81d560091942db7234232d68fdbf1c33e"
    },
    "blocks": [
      {
        "blockHeader": {
          "baseFeePerGas": "0x0a",
          "bloom": "0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
          "coinbase": "0x2adc25665018aa1fe0e6bc666dac8fc2697ff9ba",
          "difficulty": "0x00",
          "extraData": "0x00",
          "gasLimit": "0x05f5e100",
          "gasUsed": "0xb36e",
          "hash": "0xc52b87215ff3cea57b70b9e2104f7be6877a00eb98a1f10e8f4941aaefc90ae6",
          "mixHash": "0x0000000000000000000000000000000000000000000000000000000000020000",
          "nonce": "0x0000000000000000",
          "number": "0x01",
          "parentHash": "0x6d4b3f3898786350e8b7bccdce7f1d4a567c5594699de8cd7884e948d019672c",
          "receiptTrie": "0x7fb0f40c31c7596ff1847f39f294a466a231fa3d722c78408d6dcff53a3bcdb4",
          "stateRoot": "0x6e9dccb57a15e2885ff1193da0db98cbaaac218bf3a0abeb0c3ceff966de2830",
          "timestamp": "0x03e8",
          "transactionsTrie": "0x86be9b5d20254e0393853c82e5534d9d9f8486f1fd2ed4f4c0a169339c79bd1c",
          "uncleHash": "0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347",
          "withdrawalsRoot": "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421"
        },
        "rlp": "0xf902a5f90217a06d4b3f3898786350e8b7bccdce7f1d4a567c5594699de8cd7884e948d019672ca01dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347942adc25665018aa1fe0e6bc666dac8fc2697ff9baa06e9dccb57a15e2885ff1193da0db98cbaaac218bf3a0abeb0c3ceff966de2830a086be9b5d20254e0393853c82e5534d9d9f8486f1fd2ed4f4c0a169339c79bd1ca07fb0f40c31c7596ff1847f39f294a466a231fa3d722c78408d6dcff53a3bcdb4b901000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000080018405f5e10082b36e8203e800a000000000000000000000000000000000000000000000000000000000000200008800000000000000000aa056e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421f887f885800a8404c4b40094cccccccccccccccccccccccccccccccccccccccc01a4693c613900000000000000000000000000000000000000000000000000000000000000001ba0e8ff56322287185f6afd3422a825b47bf5c1a4ccf0dc0389cdc03f7c1c32b7eaa0776b02f9f5773238d3ff36b74a123f409cd6420908d7855bbe4c8ff63e00d698c0c0",
        "transactions": [
          {
            "data": "0x693c61390000000000000000000000000000000000000000000000000000000000000000",
            "gasLimit": "0x04c4b400",
            "gasPrice": "0x0a",
            "nonce": "0x00",
            "r": "0xe8ff56322287185f6afd3422a825b47bf5c1a4ccf0dc0389cdc03f7c1c32b7ea",
            "s": "0x776b02f9f5773238d3ff36b74a123f409cd6420908d7855bbe4c8ff63e00d698",
            "sender": "0xa94f5374fce5edbc8e2a8697c15331677e6ebf0b",
            "to": "0xcccccccccccccccccccccccccccccccccccccccc",
            "v": "0x1b",
            "value": "0x01"
          }
        ],
        "uncleHeaders": [],
        "withdrawals": []
      }
    ],
    "genesisBlockHeader": {
      "baseFeePerGas": "0x0b",
      "bloom": "0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
      "coinbase": "0x2adc25665018aa1fe0e6bc666dac8fc2697ff9ba",
      "difficulty": "0x00",
      "extraData": "0x00",
      "gasLimit": "0x05f5e100",
      "gasUsed": "0x00",
      "hash": "0x6d4b3f3898786350e8b7bccdce7f1d4a567c5594699de8cd7884e948d019672c",
      "mixHash": "0x0000000000000000000000000000000000000000000000000000000000020000",
      "nonce": "0x0000000000000000",
      "number": "0x00",
      "parentHash": "0x0000000000000000000000000000000000000000000000000000000000000000",
      "receiptTrie": "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421",
      "stateRoot": "0xf3d3787e33cb7913a304f188002f59e7b7a1e1fe3a712988c7092a213f8c2e8f",
      "timestamp": "0x00",
      "transactionsTrie": "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421",
      "uncleHash": "0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347",
      "withdrawalsRoot": "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421"
    },
    "genesisRLP": "0xf90219f90213a00000000000000000000000000000000000000000000000000000000000000000a01dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347942adc25665018aa1fe0e6bc666dac8fc2697ff9baa0f3d3787e33cb7913a304f188002f59e7b7a1e1fe3a712988c7092a213f8c2e8fa056e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421a056e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421b901000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000080808405f5e100808000a000000000000000000000000000000000000000000000000000000000000200008800000000000000000ba056e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421c0c0c0",
    "lastblockhash": "0xc52b87215ff3cea57b70b9e2104f7be6877a00eb98a1f10e8f4941aaefc90ae6",
    "network": "Shanghai",
    "postState": {
      "0x0000000000000000000000000000000000000100": {
        "balance": "0x0ba1a9ce0ba1a9ce",
        "code": "0x7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff0160005500",
        "nonce": "0x00",
        "storage": {
          "0x00": "0xfffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe"
        }
      },
      "0x0000000000000000000000000000000000000101": {
        "balance": "0x0ba1a9ce0ba1a9ce",
        "code": "0x60047fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff0160005500",
        "nonce": "0x00",
        "storage": {}
      },
      "0x0000000000000000000000000000000000000102": {
        "balance": "0x0ba1a9ce0ba1a9ce",
        "code": "0x60017fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff0160005500",
        "nonce": "0x00",
        "storage": {}
      },
      "0x0000000000000000000000000000000000000103": {
        "balance": "0x0ba1a9ce0ba1a9ce",
        "code": "0x600060000160005500",
        "nonce": "0x00",
        "storage": {}
      },
      "0x0000000000000000000000000000000000000104": {
        "balance": "0x0ba1a9ce0ba1a9ce",
        "code": "0x7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff60010160005500",
        "nonce": "0x00",
        "storage": {}
      },
      "0xa94f5374fce5edbc8e2a8697c15331677e6ebf0b": {
        "balance": "0x0ba1a9ce0b9aa781",
        "code": "0x",
        "nonce": "0x01",
        "storage": {}
      },
      "0xcccccccccccccccccccccccccccccccccccccccc": {
        "balance": "0x0ba1a9ce0ba1a9cf",
        "code": "0x600060006000600060006004356101000162fffffff100",
        "nonce": "0x00",
        "storage": {}
      }
    },
    "pre": {
      "0x0000000000000000000000000000000000000100": {
        "balance": "0x0ba1a9ce0ba1a9ce",
        "code": "0x7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff0160005500",
        "nonce": "0x00",
        "storage": {}
      },
      "0x0000000000000000000000000000000000000101": {
        "balance": "0x0ba1a9ce0ba1a9ce",
        "code": "0x60047fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff0160005500",
        "nonce": "0x00",
        "storage": {}
      },
      "0x0000000000000000000000000000000000000102": {
        "balance": "0x0ba1a9ce0ba1a9ce",
        "code": "0x60017fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff0160005500",
        "nonce": "0x00",
        "storage": {}
      },
      "0x0000000000000000000000000000000000000103": {
        "balance": "0x0ba1a9ce0ba1a9ce",
        "code": "0x600060000160005500",
        "nonce": "0x00",
        "storage": {}
      },
      "0x0000000000000000000000000000000000000104": {
        "balance": "0x0ba1a9ce0ba1a9ce",
        "code": "0x7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff60010160005500",
        "nonce": "0x00",
        "storage": {}
      },
      "0xa94f5374fce5edbc8e2a8697c15331677e6ebf0b": {
        "balance": "0x0ba1a9ce0ba1a9ce",
        "code": "0x",
        "nonce": "0x00",
        "storage": {}
      },
      "0xcccccccccccccccccccccccccccccccccccccccc": {
        "balance": "0x0ba1a9ce0ba1a9ce",
        "code": "0x600060006000600060006004356101000162fffffff100",
        "nonce": "0x00",
        "storage": {}
      }
    },
    "sealEngine": "NoProof"
  }}
    
    "#;

        // parse it as value, for now
        let v: serde_json::Value = serde_json::from_str(data).expect("Failed to parse JSON");

        // Get the first entry as a (key, value) tuple and clone the values to ensure no reference to `v`
        // remains
        let (_test_name, test_structure) = v
            .as_object()
            .and_then(|obj| obj.iter().next())
            .map(|(k, v)| (k.clone(), v.clone()))
            .unwrap();
        // Given
        let test_environment = Arc::new(kakarot_test_env_ctx);
        let starknet_client = test_environment.client().starknet_provider();

        // Create an atomic reference to the test environment to avoid dropping it
        let env = Arc::clone(&test_environment);

        // prop up seed state
        let binding = test_structure.clone();

        // need to debug why the class hashes aren't lining up
        let class_hashes = compute_kakarot_contracts_class_hash();
        let kakarot_class_hashes: HashMap<String, FieldElement> = class_hashes
            .into_iter()
            .map(|(filename, class_hash)| (filename.to_string(), class_hash))
            .collect();

        // It is not possible to block the async test task, so we need to spawn a blocking task
        tokio::task::spawn_blocking(move || {
            // access the necessary account/contract state the test expects
            let pre = binding.get("pre").unwrap();

            // Get lock on the Starknet sequencer
            let mut starknet = env.sequencer().sequencer.backend.state.blocking_write();

            // let me double check if this is necessary
            let eoa = &env.kakarot().eoa_addresses;
            let eoa_address = StarknetContractAddress(
                Into::<StarkFelt>::into(eoa.starknet_address)
                    .try_into()
                    .unwrap(),
            );
            let eoa_class_hash: FieldElement = (*starknet
                .storage
                .get(&eoa_address)
                .unwrap()
                .storage
                .get(&get_starknet_storage_key("_implementation", &[]))
                .unwrap())
            .into();

            // iterate through pre-state addresses
            for (original_address, account_info) in pre.as_object().unwrap().iter() {
                let mut storage = HashMap::new();

                let address_ = FieldElement::from_str(original_address).unwrap();
                let address_as_sn_address = compute_starknet_address(
                    env.kakarot().kakarot_address,
                    env.kakarot().proxy_class_hash,
                    address_,
                );

                // balance
                let balance =
                    U256::from_str(account_info.get("balance").unwrap().as_str().unwrap())
                        .expect("balance should be convertable to u256");
                // this is wrong, i think
                let balance_storage_tuples_madara =
                    genesis_fund_starknet_address(address_as_sn_address, balance);
                _madara_to_katana_storage(balance_storage_tuples_madara, &mut storage);

                // storage
                if let Some(evm_contract_storage) = account_info.get("storage").unwrap().as_object()
                {
                    let mut evm_contract_storage: Vec<(U256, U256)> = evm_contract_storage
                        .iter()
                        .map(|(k, v)| {
                            (
                                U256::from_str(k.as_str()).unwrap(),
                                U256::from_str(v.as_str().unwrap()).unwrap(),
                            )
                        })
                        .collect();
                    evm_contract_storage.sort_by_key(|(key, _)| *key);
                    evm_contract_storage.iter().for_each(|(key, value)| {
                        // Call genesis_set_storage_kakarot_contract_account util to get the storage tuples
                        let storage_tuples = genesis_set_storage_kakarot_contract_account(
                            address_as_sn_address,
                            *key,
                            *value,
                        );
                        _madara_to_katana_storage(storage_tuples, &mut storage);
                    });
                }

                let code_value = account_info.get("code").unwrap().as_str().unwrap();

                let proxy_implementation_class_hash = match code_value {
                    "0x" => {
                        // this returns the wrong class hash,
                        // *kakarot_class_hashes.get("externally_owned_account").unwrap()
                        // so i am reflecting on deployed context
                        eoa_class_hash
                    }
                    bytecode => {
                        let code_as_bytes = Bytes::from_str(bytecode).unwrap();
                        let kakarot_bytes_storage_madara =
                            genesis_set_bytecode(&code_as_bytes, address_as_sn_address);
                        _madara_to_katana_storage(kakarot_bytes_storage_madara, &mut storage);

                        let key = get_starknet_storage_key("bytecode_len_", &[]);
                        let value =
                            Into::<StarkFelt>::into(StarkFelt::from(code_as_bytes.len() as u64));
                        storage.insert(key, value);

                        env.kakarot().contract_account_class_hash
                    }
                };

                // rudimentary way to get a mapping from eth -> starknet address
                dbg!(
                    original_address,
                    address_as_sn_address,
                    account_info,
                    proxy_implementation_class_hash
                );

                let proxy_implementation_storage_tuples = genesis_set_storage_starknet_contract(
                    address_as_sn_address,
                    "_implementation",
                    &[],
                    proxy_implementation_class_hash,
                    0, // 0 since it's storage value is felt
                );

                _madara_to_katana_storage(vec![proxy_implementation_storage_tuples], &mut storage);

                // now, finally, we update the sequencer state with the eth->starknet address

                let address = StarknetContractAddress(
                    Into::<StarkFelt>::into(address_as_sn_address)
                        .try_into()
                        .unwrap(),
                );
                let account_nonce =
                    FieldElement::from_str(account_info.get("nonce").unwrap().as_str().unwrap())
                        .unwrap();
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
        let temp_value = test_structure.clone();
        let blocks = temp_value.get("blocks").unwrap();

        // we get the rlp of the block
        let block_rlp_bytes = Bytes::from_str(
            blocks
                .get(0)
                .unwrap()
                .as_object()
                .unwrap()
                .get("rlp")
                .unwrap()
                .as_str()
                .unwrap(),
        )
        .unwrap();

        // parse it as a sealed block
        let parsed_block = SealedBlock::decode(&mut block_rlp_bytes.as_ref());
        // encode body as transaction
        let mut encoded_transaction = BytesMut::new();
        parsed_block
            .unwrap()
            .body
            .get(0)
            .unwrap()
            .encode(&mut encoded_transaction);

        // execute transaction in block
        let client = test_environment.client();
        let hash = client
            .send_transaction(encoded_transaction.to_vec().into())
            .await
            .unwrap();

        let transaction_hash: FieldElement = FieldElement::from_bytes_be(&hash).unwrap();
        let receipt = starknet_client
            .get_transaction_receipt::<FieldElement>(transaction_hash.into())
            .await
            .expect("transaction has receipt");
        dbg!(receipt);


        // assert on post state
        // prop up seed state
        let binding = test_structure.clone();
        let env = Arc::clone(&test_environment);
        tokio::task::spawn_blocking(move || {
            let post_state = binding.get("postState").unwrap();

            // Get lock on the Starknet sequencer
            let starknet = env.sequencer().sequencer.backend.state.blocking_read();

            for (original_address, account_info) in post_state.as_object().unwrap().iter() {

                let address_ = FieldElement::from_str(original_address).unwrap();
                let address_as_sn_address =
        compute_starknet_address(env.kakarot().kakarot_address, env.kakarot().proxy_class_hash,
                                 address_);
                let address =
                StarknetContractAddress(Into::<StarkFelt>::into(address_as_sn_address).
                                        try_into().unwrap());
                dbg!(original_address,
        starknet.storage.get(&address));

            };
        }).await.unwrap();
    }
}
