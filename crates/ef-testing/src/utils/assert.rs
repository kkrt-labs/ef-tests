use crate::utils::starknet::get_starknet_storage_key;
use ef_tests::models::Account;
use std::collections::HashMap;

use katana_core::backend::state::StorageRecord;
use starknet::core::types::FieldElement;
use starknet_api::{core::Nonce, hash::StarkFelt, state::StorageKey};

use kakarot_rpc_core::{
    client::helpers::split_u256_into_field_elements, models::felt::Felt252Wrapper,
};

use reth_primitives::JsonU256;
use std::collections::BTreeMap;

pub fn assert_contract_post_state(
    test_name: &String,
    expected_state: &Account,
    actual_state: &StorageRecord,
) -> Result<(), ef_tests::Error> {
    let Nonce(actual_nonce) = actual_state.nonce;
    let account_nonce: FieldElement = Felt252Wrapper::try_from(expected_state.nonce.0)
        .unwrap()
        .into();

    // we don't presume gas equivalence
    // TODO: find way to assert on balance
    // assert_eq!(actual_account_balance, StarkFelt::from(expected_account_balance));

    if actual_nonce != StarkFelt::from(account_nonce) {
        return Err(ef_tests::Error::Assertion(format!(
            "failed test {}: expected nonce {}, got {}",
            test_name,
            account_nonce.to_string(),
            actual_nonce.to_string()
        )));
    }

    assert_contract_post_storage(test_name, &expected_state.storage, &actual_state.storage)?;

    Ok(())
}

pub fn assert_contract_post_storage(
    test_name: &String,
    expected_storage: &BTreeMap<JsonU256, JsonU256>,
    actual_state_storage: &HashMap<StorageKey, StarkFelt>,
) -> Result<(), ef_tests::Error> {
    for (key, value) in expected_storage.iter() {
        let keys = split_u256_into_field_elements(key.0);

        let expected_state_values = split_u256_into_field_elements(value.0);
        for (offset, value) in expected_state_values.into_iter().enumerate() {
            let stark_key = get_starknet_storage_key("storage_", &keys, offset as u64);

            let actual_state_value = *actual_state_storage.get(&stark_key).ok_or_else(|| {
                ef_tests::Error::Assertion(format!(
                    "failed test {}: missing storage for key {:?}",
                    test_name, stark_key
                ))
            })?;

            if actual_state_value != StarkFelt::from(value) {
                return Err(ef_tests::Error::Assertion(format!(
                    "failed test {}: expected storage value {}, got {}",
                    test_name,
                    value.to_string(),
                    actual_state_value.to_string()
                )));
            }
        }
    }

    Ok(())
}
