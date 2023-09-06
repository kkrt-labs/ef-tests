use starknet::core::types::FieldElement;
use starknet_api::{hash::StarkFelt, state::StorageKey};

use crate::models::error::RunnerError;

use super::starknet_storage_key_value;

pub(crate) fn generate_evm_to_starknet_address(
    evm_address: FieldElement,
    starknet_address: FieldElement,
) -> Result<(StorageKey, StarkFelt), RunnerError> {
    starknet_storage_key_value("evm_to_starknet_address", &[evm_address], starknet_address)
}
