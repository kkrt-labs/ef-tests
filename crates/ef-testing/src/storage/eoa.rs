use kakarot_test_utils::deploy_helpers::KakarotTestEnvironmentContext;
use katana_core::db::cached::AsCachedDb;
use starknet::core::types::FieldElement;
use starknet_api::{
    core::ContractAddress as StarknetContractAddress, hash::StarkFelt,
    state::StorageKey as StarknetStorageKey,
};

use crate::{models::error::RunnerError, utils::starknet::get_starknet_storage_key};

use super::{
    generate_evm_address_storage, generate_is_initialized_storage, starknet_storage_key_value,
};

/// Returns the class hash used for the EOA contract.
pub fn get_eoa_class_hash(
    ctx: &KakarotTestEnvironmentContext,
    starknet: &AsCachedDb,
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
        generate_is_initialized_storage()?,
        generate_kakarot_address_storage(kakarot_address)?,
        generate_evm_address_storage(evm_address)?,
    ];
    Ok(eoa_storage)
}

/// Returns the kakarot address storage tuple.
fn generate_kakarot_address_storage(
    kakarot_address: FieldElement,
) -> Result<(StarknetStorageKey, StarkFelt), RunnerError> {
    starknet_storage_key_value("kakarot_address", &[], kakarot_address)
}
