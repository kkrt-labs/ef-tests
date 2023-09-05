use kakarot_rpc_core::test_utils::deploy_helpers::KakarotTestEnvironmentContext;
use katana_core::backend::state::MemDb;
use starknet::core::types::FieldElement;
use starknet_api::{
    core::ContractAddress as StarknetContractAddress, hash::StarkFelt,
    state::StorageKey as StarknetStorageKey,
};
use tokio::sync::RwLockWriteGuard;

use crate::{models::error::RunnerError, utils::starknet::get_starknet_storage_key};

use super::{get_evm_address, get_is_initialized, get_starknet_storage};

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
/// Returns the storage tuples for eoa contract.
pub fn initialize_eoa(
    kakarot_address: FieldElement,
    evm_address: FieldElement,
) -> Result<Vec<(StarknetStorageKey, StarkFelt)>, RunnerError> {
    let eoa_storage = vec![
        get_is_initialized()?,
        get_kakarot_address(kakarot_address)?,
        get_evm_address(evm_address)?,
    ];
    Ok(eoa_storage)
}

/// Returns the kakarot address storage tuple.
fn get_kakarot_address(
    kakarot_address: FieldElement,
) -> Result<(StarknetStorageKey, StarkFelt), RunnerError> {
    get_starknet_storage("kakarot_address", &[], kakarot_address)
}
