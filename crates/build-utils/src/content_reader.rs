use crate::{
    constants::ADDRESSES_KEYS, path::PathWrapper,
    utils::blockchain_tests_to_general_state_tests_path,
};
use alloy_rlp::Decodable;
use eyre::eyre;
use reth_primitives::{address, hex, revm_primitives::FixedBytes, Address, Block};
use serde_json::Value;
use std::collections::BTreeMap;

/// The `ContentReader` is used to read the content of the ef-test tests files.
/// The tests files are located in the `BlockchainTests` folder and contain
/// the target block, the pre state, the transaction and the post state.
///
/// The tests are doubled and located in the `GeneralStateTests` folder, but
/// in a lighter version. The secret key for the transaction is only located
/// in the `GeneralStateTests` folder.
pub struct ContentReader;

impl ContentReader {
    /// Reads the secret key from the given path for the `GeneralStateTests`.
    ///
    /// All tests are taken from the `BlockchainTests` folder,
    /// but the secret key is taken from the `GeneralStateTests` folder.
    ///
    /// # Example
    ///
    /// Test location: BlockchainTests/GeneralStateTests/stRandom/randomStatetest0.json
    /// Secret key location: GeneralStateTests/stRandom/randomStatetest0.json
    pub fn secret_key(
        path: PathWrapper,
        case_without_secret: &Value,
    ) -> Result<String, eyre::Error> {
        let path = blockchain_tests_to_general_state_tests_path(path);

        let case = match path.read_file_to_string() {
            Ok(content) => {
                let cases: BTreeMap<String, Value> = serde_json::from_str(&content)?;
                cases.into_values().next()
            }
            Err(_) => Some(case_without_secret.clone()),
        };

        match get_secret_key_from_case(case.as_ref()) {
            Some(key) => Ok(format!("\"{}\"", key)),
            None => get_secret_key_from_block(case_without_secret),
        }
    }

    pub fn pre_state(test_case: &Value) -> Result<Value, eyre::Error> {
        Ok(serde_json::from_value(
            test_case
                .get("pre")
                .ok_or_else(|| eyre!("key 'preState' is empty"))?
                .clone(),
        )?)
    }

    pub fn post_state(test_case: &Value) -> Result<Value, eyre::Error> {
        Ok(test_case
            .get("postState")
            .ok_or_else(|| eyre!("key 'postState' is empty"))?
            .clone())
    }

    pub fn block(test_case: &Value) -> Result<Value, eyre::Error> {
        // Attempt to get the "blocks" value
        let blocks = test_case
            .get("blocks")
            .ok_or_else(|| eyre!("key 'blocks' not found"))?;

        // Ensure it's an array
        let blocks_array = blocks
            .as_array()
            .ok_or_else(|| eyre!("'blocks' is not an array"))?;

        // Get the first block
        let first_block = blocks_array
            .first()
            .ok_or_else(|| eyre!("'blocks' array is empty"))?;

        // Return a clone of the block
        Ok(first_block.clone())
    }

    pub fn transaction(test_case: &Value, block: &Value) -> Result<Value, eyre::Error> {
        let block_data = match block.get("rlp_decoded") {
            Some(block) => Ok(block),
            None => Err(eyre!("key 'rlp_decoded' not found")),
        }?;

        // Check if the block contains a field named "transactions"
        Ok(if let Some(transaction) = block_data.get("transactions") {
            // If the "transactions" field exists, try to convert its value to an array
            let transaction_array = transaction
                .as_array()
                .ok_or_else(|| eyre!("'transactions' is not an array"))?;

            // Get the first transaction from the array
            transaction_array
                .first()
                .ok_or_else(|| eyre!("'transactions' array is empty"))?
                .clone() // Clone the transaction value
        } else {
            // If the block does not contain a "transactions" field,
            // retrieve the transaction directly from the test case
            test_case
                .get("transaction")
                .ok_or_else(|| eyre!("key 'transaction' not found"))?
                .clone() // Clone the transaction value
        })
    }
}

fn get_secret_key_from_case(case: Option<&Value>) -> Option<&str> {
    case.and_then(|value| value.get("transaction"))
        .and_then(|value| value.get("secretKey"))
        .and_then(|value| value.as_str())
}

fn get_secret_key_from_block(case_without_secret: &Value) -> Result<String, eyre::Error> {
    let block = ContentReader::block(case_without_secret)?;

    let transaction = ContentReader::transaction(case_without_secret, &block);

    let sender_address = match transaction {
        Ok(transaction) => {
            let sender = transaction
                .get("sender")
                .and_then(|value| value.as_str())
                .ok_or_else(|| eyre!("Key 'sender' not found"))?;
            let sender_address: Address = sender.parse::<FixedBytes<20>>()?.into();
            sender_address
        }
        Err(_) => {
            // If the block is invalid, it's not possible to get the secret key from transactions.
            // We need to try rlp-decoding the block; and if the encoding is invalid, optimistically assume sender
            // is             address!("a94f5374fce5edbc8e2a8697c15331677e6ebf0b"),
            //TODO: there should be a deeper refactor to simply skip transactionless tests.
            let block_rlp = match block.get("rlp").unwrap() {
                Value::String(s) => hex::decode(s),
                _ => panic!("test"),
            }
            .unwrap();

            let sender_address = match Block::decode(&mut block_rlp.as_slice()) {
                Ok(block) => block
                    .senders()
                    .unwrap()
                    .first()
                    .cloned()
                    .ok_or_else(|| eyre!("No sender address found"))?,
                Err(_) => address!("a94f5374fce5edbc8e2a8697c15331677e6ebf0b"),
            };

            sender_address
        }
    };

    ADDRESSES_KEYS
        .get(&sender_address)
        .map(|addr| format!("\"{}\"", addr))
        .ok_or_else(|| eyre!("No secret key found for {}", sender_address))
}
