use hive_utils::madara::utils::genesis_set_bytecode;
use reth_primitives::Bytes;
use starknet::core::types::FieldElement;
use starknet_api::{hash::StarkFelt, state::StorageKey as StarknetStorageKey};

use crate::models::error::RunnerError;

use super::{get_evm_address, get_is_initialized, get_starknet_storage, madara_to_katana_storage};

/// Initializes the contract account.
/// Writes the bytecode and the owner to a hashmap.
pub fn initialize_contract_account(
    kakarot_address: FieldElement,
    evm_address: FieldElement,
    bytecode: &Bytes,
) -> Result<Vec<(StarknetStorageKey, StarkFelt)>, RunnerError> {
    let mut contract_storage = vec![
        get_evm_address(evm_address)?,
        get_is_initialized()?,
        get_owner(kakarot_address)?,
    ];
    contract_storage.append(&mut get_bytecode(bytecode)?);
    Ok(contract_storage)
}

/// Returns the bytecode storage tuples.
fn get_bytecode(bytecode: &Bytes) -> Result<Vec<(StarknetStorageKey, StarkFelt)>, RunnerError> {
    let bytecode_len = bytecode.len();

    let bytecode = genesis_set_bytecode(bytecode, FieldElement::ZERO);
    let mut bytecode_storage = madara_to_katana_storage(bytecode)?;

    bytecode_storage.push(get_starknet_storage(
        "bytecode_len_",
        &[],
        FieldElement::from(bytecode_len),
    )?);

    Ok(bytecode_storage)
}

/// Returns the owner storage tuple.
fn get_owner(
    kakarot_address: FieldElement,
) -> Result<(StarknetStorageKey, StarkFelt), RunnerError> {
    get_starknet_storage("Ownable_owner", &[], kakarot_address)
}
