use crate::{
    constants::ADDRESSES_KEYS, path::PathWrapper,
    utils::blockchain_tests_to_general_state_tests_path,
};
use eyre::eyre;
use reth_primitives::{revm_primitives::FixedBytes, Address};
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
        let maybe_content_with_secret = path.read_file_to_string();
        let case = match maybe_content_with_secret {
            Ok(content) => {
                let cases: BTreeMap<String, Value> = serde_json::from_str(&content)?;
                cases.into_values().next()
            }
            Err(_) => Some(case_without_secret.clone()),
        };

        let key = match case
            .as_ref()
            .and_then(|value| value.get("transaction"))
            .and_then(|value| value.get("secretKey"))
        {
            Some(key) => key.to_string(),
            None => {
                let block = Self::block(case_without_secret)?;
                let transaction = Self::transaction(case_without_secret, &block)?;
                let sender = transaction
                    .get("sender")
                    .and_then(|value| value.as_str())
                    .ok_or_else(|| eyre!("Key 'sender' not found"))?;

                let sender_address: Address = sender.parse::<FixedBytes<20>>()?.into();
                ADDRESSES_KEYS
                    .get(&sender_address)
                    .map(|addr| format!("\"{}\"", addr))
                    .unwrap_or_else(|| panic!("No secret key found for {sender_address}"))
            }
        };
        Ok(key)
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
        // Check if the block contains a field named "transactions"
        Ok(if let Some(transaction) = block.get("transactions") {
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
