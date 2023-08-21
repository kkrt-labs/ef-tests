use starknet::core::{types::FieldElement, utils::get_storage_var_address};
use starknet_api::{hash::StarkFelt, state::StorageKey};

pub fn get_starknet_storage_key(var_name: &str, args: &[FieldElement]) -> StorageKey {
    StorageKey(
        Into::<StarkFelt>::into(get_storage_var_address(var_name, args).unwrap())
            .try_into()
            .unwrap(),
    )
}
