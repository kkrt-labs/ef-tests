use std::{collections::HashMap, sync::Arc};

use kakarot_rpc_core::test_utils::deploy_helpers::KakarotTestEnvironmentContext;
use katana_core::backend::state::MemDb;
use starknet::core::types::FieldElement;
use starknet_api::{
    core::ContractAddress as StarknetContractAddress, hash::StarkFelt,
    state::StorageKey as StarknetStorageKey,
};
use tokio::sync::RwLockWriteGuard;

use crate::utils::get_starknet_storage_key;

/// Returns the class hash used for the EOA contract.
pub fn get_eoa_class_hash(
    ctx: Arc<KakarotTestEnvironmentContext>,
    starknet: &RwLockWriteGuard<'_, MemDb>,
) -> Result<FieldElement, eyre::Error> {
    let eoa = &ctx.kakarot().eoa_addresses;

    let eoa_address =
        StarknetContractAddress(Into::<StarkFelt>::into(eoa.starknet_address).try_into()?);

    // deriving the eao class hash this way so things are always based off the katana dump file
    let eoa_class_hash: FieldElement = (*starknet
        .storage
        .get(&eoa_address)
        .ok_or_else(|| eyre::eyre!("Failed to get storage for eoa at {:?}", eoa_address))?
        .storage
        .get(&get_starknet_storage_key("_implementation", &[]))
        .ok_or_else(|| {
            eyre::eyre!(
                "Failed to get value at key _implementation for eoa at {:?}",
                eoa_address
            )
        })?)
    .into();

    Ok(eoa_class_hash)
}

/// Initializes the EOA contract.
/// Writes the kakarot address and the evm address to storage.
pub fn initialize_eoa(
    kakarot_address: FieldElement,
    evm_address: FieldElement,
    destination: &mut HashMap<StarknetStorageKey, StarkFelt>,
) {
    write_kakarot_address(kakarot_address, destination);
    write_evm_address(evm_address, destination);
}

/// Writes the kakarot address to the EOA contract.
pub fn write_kakarot_address(
    kakarot_address: FieldElement,
    destination: &mut HashMap<StarknetStorageKey, StarkFelt>,
) {
    let kakarot_address_key = get_starknet_storage_key("kakarot_address", &[]);
    let kakarot_address_value = Into::<StarkFelt>::into(StarkFelt::from(kakarot_address));
    destination.insert(kakarot_address_key, kakarot_address_value);
}

/// Writes the evm address to the EOA contract.
pub fn write_evm_address(
    evm_address: FieldElement,
    destination: &mut HashMap<StarknetStorageKey, StarkFelt>,
) {
    let evm_address_key = get_starknet_storage_key("evm_address", &[]);
    let evm_address_value = Into::<StarkFelt>::into(StarkFelt::from(evm_address));
    destination.insert(evm_address_key, evm_address_value);
}
