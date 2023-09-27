use blockifier::abi::abi_utils::get_storage_var_address as blockifier_get_storage_var_address;
use reth_primitives::Bytes;
use revm_primitives::U256;
use starknet_api::{
    core::{ClassHash, ContractAddress},
    hash::StarkFelt,
    state::StorageKey,
};

pub(crate) fn get_storage_var_address(
    storage_var: &str,
    keys: &[StarkFelt],
) -> Result<StorageKey, eyre::Error> {
    Ok(blockifier_get_storage_var_address(storage_var, keys)?)
}

pub(crate) fn split_bytecode_to_starkfelt(bytecode: &Bytes) -> Vec<StarkFelt> {
    bytecode
        .chunks(16)
        .map(|x| {
            let mut storage_value = [0u8; 16];
            storage_value[..x.len()].copy_from_slice(x);
            StarkFelt::from(u128::from_be_bytes(storage_value))
        })
        .collect()
}

pub(crate) fn split_u256(value: U256) -> [u128; 2] {
    [
        (value & U256::from(u128::MAX)).try_into().unwrap(), // safe unwrap <= U128::MAX.
        (value >> 128).try_into().unwrap(),                  // safe unwrap <= U128::MAX.
    ]
}

pub(crate) fn contract_address_to_starkfelt(contract_address: &ContractAddress) -> StarkFelt {
    *contract_address.0.key()
}

pub(crate) fn class_hash_to_starkfelt(class_hash: &ClassHash) -> StarkFelt {
    class_hash.0
}
