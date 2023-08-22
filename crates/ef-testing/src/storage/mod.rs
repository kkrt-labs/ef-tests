pub mod contract;
pub mod eoa;

use std::collections::HashMap;

use hive_utils::{
    madara::utils::{genesis_approve_kakarot, genesis_fund_starknet_address},
    types::{ContractAddress, StorageKey, StorageValue},
};
use katana_core::{backend::state::MemDb, constants::FEE_TOKEN_ADDRESS};
use revm_primitives::U256;
use starknet::core::types::FieldElement;
use starknet_api::{
    core::{ContractAddress as StarknetContractAddress, PatriciaKey},
    hash::StarkFelt,
    state::StorageKey as StarknetStorageKey,
};
use tokio::sync::{RwLockReadGuard, RwLockWriteGuard};

use crate::utils::starknet::get_starknet_storage_key;

/// Converts a madara storage tuple to a katana storage tuple.
pub fn madara_to_katana_storage(
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

/// Writes a katana storage tuple to a hashmap.
pub fn write_katana_storage(
    data: Vec<(StarknetStorageKey, StarkFelt)>,
    destination: &mut HashMap<StarknetStorageKey, StarkFelt>,
) {
    for (key, value) in data {
        destination.insert(key, value);
    }
}

/// Writes a madara storage tuple to a hashmap.
pub fn write_madara_to_katana_storage(
    source: Vec<((ContractAddress, StorageKey), StorageValue)>,
    destination: &mut HashMap<StarknetStorageKey, StarkFelt>,
) {
    let reformatted_data = madara_to_katana_storage(source);
    write_katana_storage(reformatted_data, destination);
}

/// Writes the fee token balance and allowance to the katana storage.
pub fn write_fee_token(
    kakarot_address: FieldElement,
    starknet_address: FieldElement,
    balance: U256,
    starknet: &mut RwLockWriteGuard<'_, MemDb>,
) -> Result<(), eyre::Error> {
    write_balance(starknet_address, balance, starknet)?;
    write_allowance(kakarot_address, starknet_address, starknet)?;
    Ok(())
}

/// Writes the balance of an account to the katana storage.
pub fn write_balance(
    starknet_address: FieldElement,
    balance: U256,
    starknet: &mut RwLockWriteGuard<'_, MemDb>,
) -> Result<(), eyre::Error> {
    let balance_storage_tuples_madara = genesis_fund_starknet_address(starknet_address, balance);
    let fee_token_address =
        StarknetContractAddress(TryInto::<PatriciaKey>::try_into(*FEE_TOKEN_ADDRESS)?);
    let balance_storage = madara_to_katana_storage(balance_storage_tuples_madara);

    // funding balance
    for (storage_key, balance) in balance_storage {
        starknet
            .storage
            .entry(fee_token_address)
            .or_default()
            .storage
            .insert(storage_key, balance);
    }
    Ok(())
}

/// Writes the allowance of an account to the katana storage.
pub fn write_allowance(
    kakarot_address: FieldElement,
    starknet_address: FieldElement,
    starknet: &mut RwLockWriteGuard<'_, MemDb>,
) -> Result<(), eyre::Error> {
    let allowance = genesis_approve_kakarot(starknet_address, kakarot_address, U256::MAX);
    let balance_storage = madara_to_katana_storage(allowance);

    let fee_token_address =
        StarknetContractAddress(TryInto::<PatriciaKey>::try_into(*FEE_TOKEN_ADDRESS)?);

    // funding balance
    for (storage_key, balance) in balance_storage {
        starknet
            .storage
            .entry(fee_token_address)
            .or_default()
            .storage
            .insert(storage_key, balance);
    }
    Ok(())
}

/// Reads the balance of an account of the katana storage.
pub fn read_balance(
    starknet_address: FieldElement,
    starknet: &RwLockReadGuard<'_, MemDb>,
) -> Result<StarkFelt, eyre::Error> {
    let fee_token_address =
        StarknetContractAddress(TryInto::<PatriciaKey>::try_into(*FEE_TOKEN_ADDRESS)?);

    let storage_key = get_starknet_storage_key("ERC20_balances", &[starknet_address], 0);
    Ok(*starknet
        .storage
        .get(&fee_token_address)
        .unwrap()
        .storage
        .get(&storage_key)
        .unwrap())
}
