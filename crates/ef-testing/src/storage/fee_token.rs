use kakarot_test_utils::hive_utils::madara::utils::{
    genesis_approve_kakarot, genesis_fund_starknet_address,
};
use katana_core::{constants::FEE_TOKEN_ADDRESS, db::Database};
use reth_primitives::Address;
use revm_primitives::U256;
use starknet::core::types::FieldElement;
use starknet_api::{
    core::{ContractAddress, PatriciaKey},
    hash::StarkFelt,
    state::StorageKey,
};
use tokio::sync::RwLockWriteGuard;

use crate::{models::error::RunnerError, utils::starknet::get_starknet_storage_key};

use super::madara_to_katana_storage;

/// Returns the fee token storage tuple for balance and allowance.
pub(crate) fn initialize_fee_token_storage(
    kakarot_address: FieldElement,
    starknet_address: FieldElement,
    balance: U256,
) -> Result<Vec<(StorageKey, StarkFelt)>, RunnerError> {
    let mut storage = Vec::new();
    storage.append(&mut generate_balance_storage(starknet_address, balance)?);
    storage.append(&mut generate_allowance_storage(
        kakarot_address,
        starknet_address,
    )?);
    Ok(storage)
}

/// Returns the balance storage tuple.
pub(crate) fn generate_balance_storage(
    starknet_address: FieldElement,
    balance: U256,
) -> Result<Vec<(StorageKey, StarkFelt)>, RunnerError> {
    let balance = genesis_fund_starknet_address(starknet_address, balance);
    let balance_storage = madara_to_katana_storage(balance)?;

    Ok(balance_storage)
}

/// Returns the allowance storage tuple.
pub(crate) fn generate_allowance_storage(
    kakarot_address: FieldElement,
    starknet_address: FieldElement,
) -> Result<Vec<(StorageKey, StarkFelt)>, RunnerError> {
    let allowance = genesis_approve_kakarot(starknet_address, kakarot_address, U256::MAX);
    let allowance_storage = madara_to_katana_storage(allowance)?;

    Ok(allowance_storage)
}

/// Reads the balance of an account of the katana storage.
pub(crate) fn read_balance(
    evm_address: &Address,
    starknet_address: FieldElement,
    starknet: &mut RwLockWriteGuard<'_, dyn Database>,
) -> Result<FieldElement, RunnerError> {
    let fee_token_address = ContractAddress(TryInto::<PatriciaKey>::try_into(*FEE_TOKEN_ADDRESS)?);

    let storage_key = get_starknet_storage_key("ERC20_balances", &[starknet_address], 0)?;
    let balance = starknet
        .get_storage_at(fee_token_address, storage_key)
        .map_err(|_| RunnerError::Other(format!("missing balance for {:#20x}", evm_address)))?;
    Ok(balance.into())
}
