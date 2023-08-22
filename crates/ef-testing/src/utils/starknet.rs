use starknet::core::{types::FieldElement, utils::get_storage_var_address};
use starknet_api::{hash::StarkFelt, state::StorageKey};

/// Returns the Starknet storage key as a starknet api type
pub fn get_starknet_storage_key(
    var_name: &str,
    args: &[FieldElement],
    storage_offset: u64,
) -> StorageKey {
    let mut storage_key = get_storage_var_address(var_name, args).unwrap();

    storage_key += FieldElement::from(storage_offset);

    StorageKey(Into::<StarkFelt>::into(storage_key).try_into().unwrap())
}
