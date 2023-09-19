use ef_tests::models::Account;
use kakarot_rpc_core::{
    client::helpers::split_u256_into_field_elements, models::felt::Felt252Wrapper,
};
use katana_core::db::cached::StorageRecord;
use reth_primitives::Address;
use reth_primitives::JsonU256;
use revm_primitives::U256;
use starknet::core::types::FieldElement;
use starknet_api::{core::Nonce, hash::StarkFelt, state::StorageKey};
use std::collections::BTreeMap;
use std::collections::HashMap;

use crate::models::error::RunnerError;
use crate::utils::starknet::get_starknet_storage_key;

pub fn assert_contract_post_state(
    test_name: &str,
    evm_address: &Address,
    expected_state: &Account,
    actual_state: &StorageRecord,
) -> Result<(), RunnerError> {
    let Nonce(actual_nonce) = actual_state.nonce;
    let account_nonce: FieldElement = Felt252Wrapper::try_from(expected_state.nonce.0)?.into();

    // we don't presume gas equivalence
    // TODO: find way to assert on balance
    // assert_eq!(actual_account_balance, StarkFelt::from(expected_account_balance));

    let account_nonce = StarkFelt::from(account_nonce);
    if actual_nonce != account_nonce {
        return Err(RunnerError::Other(format!(
            "{} expected nonce {} for {:#20x}, got {}",
            test_name, account_nonce, evm_address, actual_nonce
        )));
    }

    assert_contract_post_storage(
        test_name,
        evm_address,
        &expected_state.storage,
        &actual_state.storage,
    )?;

    Ok(())
}

pub fn assert_contract_post_storage(
    test_name: &str,
    evm_address: &Address,
    expected_storage: &BTreeMap<JsonU256, JsonU256>,
    actual_state_storage: &HashMap<StorageKey, StarkFelt>,
) -> Result<(), RunnerError> {
    for (key, value) in expected_storage.iter() {
        let keys = split_u256_into_field_elements(key.0);
        let expected_state_values = split_u256_into_field_elements(value.0);

        for (offset, value) in expected_state_values.into_iter().enumerate() {
            let stark_key = get_starknet_storage_key("storage_", &keys, offset as u64)?;

            let actual_state_value = actual_state_storage
                .get(&stark_key)
                .copied()
                .unwrap_or_default();

            let value = StarkFelt::from(value);
            if actual_state_value != value {
                return Err(RunnerError::Other(format!(
                    "{} expected storage value {} for {:#20x}, got {}",
                    test_name, value, evm_address, actual_state_value
                )));
            }
        }
    }

    Ok(())
}

pub fn assert_empty_post_state(
    test_name: &str,
    state: &Account,
    actual_balance: FieldElement,
) -> Result<(), RunnerError> {
    let is_code_empty = state.code.is_empty();
    let is_storage_empty = state.storage.is_empty();
    let is_nonce_zero = state.nonce.0 == U256::ZERO;

    if !is_code_empty || !is_storage_empty || !is_nonce_zero {
        return Err(RunnerError::Assertion(format!(
            "{test_name} expected empty post state, got {state:#?}"
        )));
    }

    let expected_balance = state.balance.0;
    let actual_balance = U256::from_be_bytes(actual_balance.to_bytes_be());

    if expected_balance != actual_balance {
        return Err(RunnerError::Assertion(format!(
            "{test_name} expected balance {expected_balance:#32x}, got {actual_balance:#32x}"
        )));
    }

    Ok(())
}
