use std::collections::BTreeMap;

use serde_json::Value;

use crate::{dir_reader::PathWrapper, utils::blockchain_tests_to_general_state_tests_path};

pub struct ContentReader;

impl ContentReader {
    pub fn secret_key(path: PathWrapper) -> Result<Value, eyre::Error> {
        let path = blockchain_tests_to_general_state_tests_path(path);
        let content = path.read_file_to_string()?;

        let cases: BTreeMap<String, Value> = serde_json::from_str(&content)?;
        let case = cases
            .into_values()
            .collect::<Vec<_>>()
            .first()
            .ok_or_else(|| eyre::eyre!("No case found"))?
            .clone();

        Ok(case
            .get("transaction")
            .ok_or_else(|| eyre::eyre!("No transaction found"))?
            .get("secretKey")
            .ok_or_else(|| eyre::eyre!("No secret key found"))?
            .clone())
    }

    pub fn pre_state(test_case: &Value) -> Result<Value, eyre::Error> {
        Ok(serde_json::from_value(
            test_case
                .get("pre")
                .ok_or_else(|| eyre::eyre!("No pre state found"))?
                .clone(),
        )?)
    }

    pub fn post_state(test_case: &Value) -> Result<Value, eyre::Error> {
        Ok(test_case
            .get("postState")
            .ok_or_else(|| eyre::eyre!("No post state found"))?
            .clone())
    }

    pub fn block(test_case: &Value) -> Result<Value, eyre::Error> {
        Ok(test_case
            .get("blocks")
            .and_then(|blocks| blocks.as_array().map(|blocks| blocks.first()))
            .flatten()
            .ok_or_else(|| eyre::eyre!("No blocks found"))?
            .clone())
    }
}
