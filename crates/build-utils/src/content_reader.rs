use std::collections::BTreeMap;

use serde_json::Value;

use crate::{path::PathWrapper, utils::blockchain_tests_to_general_state_tests_path};

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
    pub fn secret_key(path: PathWrapper) -> Result<Option<Value>, eyre::Error> {
        let path = blockchain_tests_to_general_state_tests_path(path);
        let content = path.read_file_to_string()?;

        let cases: BTreeMap<String, Value> = serde_json::from_str(&content)?;
        let case = cases.into_values().next();

        Ok(case
            .as_ref()
            .and_then(|value| value.get("transaction"))
            .and_then(|value| value.get("secretKey"))
            .cloned())
    }

    pub fn pre_state(test_case: &Value) -> Result<Value, eyre::Error> {
        Ok(serde_json::from_value(
            test_case
                .get("pre")
                .ok_or_else(|| eyre::eyre!("key 'preState' is empty"))?
                .clone(),
        )?)
    }

    pub fn post_state(test_case: &Value) -> Result<Value, eyre::Error> {
        Ok(test_case
            .get("postState")
            .ok_or_else(|| eyre::eyre!("key 'postState' is empty"))?
            .clone())
    }

    pub fn block(test_case: &Value) -> Result<Value, eyre::Error> {
        // Attempt to get the "blocks" value
        let blocks = test_case
            .get("blocks")
            .ok_or_else(|| eyre::eyre!("key 'blocks' not found"))?;

        // Ensure it's an array
        let blocks_array = blocks
            .as_array()
            .ok_or_else(|| eyre::eyre!("'blocks' is not an array"))?;

        // Get the first block
        let first_block = blocks_array
            .first()
            .ok_or_else(|| eyre::eyre!("'blocks' array is empty"))?;

        // Return a clone of the block
        Ok(first_block.clone())
    }
}
