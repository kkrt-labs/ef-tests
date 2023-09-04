use starknet::core::{types::FieldElement, utils::get_storage_var_address};
use starknet_api::{hash::StarkFelt, state::StorageKey};

use crate::models::error::RunnerError;

/// Returns the Starknet storage key as a starknet api type
pub fn get_starknet_storage_key(
    var_name: &str,
    args: &[FieldElement],
    storage_offset: u64,
) -> Result<StorageKey, RunnerError> {
    let mut storage_key = get_storage_var_address(var_name, args)?;

    storage_key += FieldElement::from(storage_offset);

    Ok(StorageKey(Into::<StarkFelt>::into(storage_key).try_into()?))
}
