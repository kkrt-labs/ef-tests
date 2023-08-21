use std::collections::HashMap;

use hive_utils::madara::utils::genesis_set_bytecode;
use reth_primitives::Bytes;
use starknet::core::types::FieldElement;
use starknet_api::{hash::StarkFelt, state::StorageKey as StarknetStorageKey};

use crate::utils::get_starknet_storage_key;

use super::write_madara_to_katana_storage;

/// Initializes the contract account.
/// Writes the bytecode and the owner to a hashmap.
pub fn initialize_contract_account(
    kakarot_address: FieldElement,
    starknet_address: FieldElement,
    bytecode: &Bytes,
    destination: &mut HashMap<StarknetStorageKey, StarkFelt>,
) {
    write_bytecode(starknet_address, bytecode, destination);
    write_owner(kakarot_address, destination);
}

/// Writes the bytecode to a hashmap.
pub fn write_bytecode(
    starknet_address: FieldElement,
    bytecode: &Bytes,
    destination: &mut HashMap<StarknetStorageKey, StarkFelt>,
) {
    let bytecode_len = bytecode.len();
    let bytecode = genesis_set_bytecode(bytecode, starknet_address);
    write_madara_to_katana_storage(bytecode, destination);

    let bytecode_len_key = get_starknet_storage_key("bytecode_len_", &[]);
    let bytecode_len_value = Into::<StarkFelt>::into(StarkFelt::from(bytecode_len as u64));
    destination.insert(bytecode_len_key, bytecode_len_value);
}

/// Writes the owner to the contract account.
pub fn write_owner(
    kakarot_address: FieldElement,
    destination: &mut HashMap<StarknetStorageKey, StarkFelt>,
) {
    let owner = get_starknet_storage_key("Ownable_owner", &[]);
    destination.insert(owner, Into::<StarkFelt>::into(kakarot_address));
}
