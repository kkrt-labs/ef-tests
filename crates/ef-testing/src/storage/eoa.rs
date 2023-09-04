use std::collections::HashMap;

use kakarot_rpc_core::test_utils::deploy_helpers::KakarotTestEnvironmentContext;
use katana_core::backend::state::MemDb;
use starknet::core::types::FieldElement;
use starknet_api::{
    core::ContractAddress as StarknetContractAddress, hash::StarkFelt,
    state::StorageKey as StarknetStorageKey,
};
use tokio::sync::RwLockWriteGuard;

use crate::{models::error::RunnerError, utils::starknet::get_starknet_storage_key};

use super::write_is_initialized;

/// Returns the class hash used for the EOA contract.
pub fn get_eoa_class_hash(
    ctx: &KakarotTestEnvironmentContext,
    starknet: &RwLockWriteGuard<'_, MemDb>,
) -> Result<FieldElement, RunnerError> {
    let eoa = &ctx.kakarot().eoa_addresses;

    let eoa_address =
        StarknetContractAddress(Into::<StarkFelt>::into(eoa.starknet_address).try_into()?);

    // deriving the eao class hash this way so things are always based off the katana dump file
    let eoa_class_hash: FieldElement = (*starknet
        .storage
        .get(&eoa_address)
        .ok_or_else(|| {
            RunnerError::Other(format!(
                "Failed to get storage for eoa at {:?}",
                eoa_address
            ))
        })?
        .storage
        .get(&get_starknet_storage_key("_implementation", &[], 0)?)
        .ok_or_else(|| {
            RunnerError::Other(format!(
                "Failed to get value at key _implementation for eoa at {:?}",
                eoa_address
            ))
        })?)
    .into();

    Ok(eoa_class_hash)
}

/// Initializes the EOA contract.
/// Writes the kakarot address and the evm address to a hashmap.
pub fn initialize_eoa(
    kakarot_address: FieldElement,
    evm_address: FieldElement,
    destination: &mut HashMap<StarknetStorageKey, StarkFelt>,
) -> Result<(), RunnerError> {
    write_is_initialized(destination)?;
    write_kakarot_address(kakarot_address, destination)?;
    write_evm_address(evm_address, destination)?;
    Ok(())
}

/// Writes the kakarot address to a hashmap.
fn write_kakarot_address(
    kakarot_address: FieldElement,
    destination: &mut HashMap<StarknetStorageKey, StarkFelt>,
) -> Result<(), RunnerError> {
    let kakarot_address_key = get_starknet_storage_key("kakarot_address", &[], 0)?;
    let kakarot_address_value = Into::<StarkFelt>::into(StarkFelt::from(kakarot_address));
    destination.insert(kakarot_address_key, kakarot_address_value);
    Ok(())
}

/// Writes the evm address to a hashmap.
fn write_evm_address(
    evm_address: FieldElement,
    destination: &mut HashMap<StarknetStorageKey, StarkFelt>,
) -> Result<(), RunnerError> {
    let evm_address_key = get_starknet_storage_key("evm_address", &[], 0)?;
    let evm_address_value = Into::<StarkFelt>::into(StarkFelt::from(evm_address));
    destination.insert(evm_address_key, evm_address_value);
    Ok(())
}
