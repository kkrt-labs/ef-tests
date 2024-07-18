use blockifier::abi::abi_utils::get_storage_var_address;
use reth_primitives::{Address, Bytes, U256};
use starknet::core::types::Felt;
use starknet_api::{core::Nonce, state::StorageKey, StarknetApiError};

use super::KakarotAccount;
use super::{inner_byte_array_pointer, pack_byte_array_to_starkfelt_array};
use crate::evm_sequencer::constants::storage_variables::{
    ACCOUNT_EVM_ADDRESS, ACCOUNT_IS_INITIALIZED, ACCOUNT_NONCE, ACCOUNT_STORAGE,
};
use crate::evm_sequencer::{
    constants::storage_variables::ACCOUNT_BYTECODE, types::felt::FeltSequencer, utils::split_u256,
};
use crate::{
    evm_sequencer::evm_state::v1::{compute_storage_base_address, offset_storage_key},
    starknet_storage,
};

/// The layout of a `ByteArray` in storage is as follows:
/// * Only the length in bytes is stored in the original address where the byte array is logically
///   stored.
/// * The actual data is stored in chunks of 256 `bytes31`s in another place in storage
///   determined by the hash of:
///   - The address storing the length of the array.
///   - The chunk index.
///   - The short string `ByteArray`.
fn prepare_bytearray_storage(code: &Bytes) -> Vec<(StorageKey, Felt)> {
    let bytecode_base_address = get_storage_var_address(ACCOUNT_BYTECODE, &[]);
    let mut bytearray = vec![(bytecode_base_address, Felt::from(code.len()))];

    let bytecode_storage: Vec<_> = pack_byte_array_to_starkfelt_array(code)
        .enumerate()
        .map(|(index, b)| {
            let offset = index % 256;
            let index = index / 256;
            let key = inner_byte_array_pointer(*bytecode_base_address.0.key(), index.into());
            (
                offset_storage_key(key.try_into().unwrap(), offset as i64),
                b,
            )
        })
        .collect();
    bytearray.extend(bytecode_storage);

    bytearray
}

impl KakarotAccount {
    pub fn new(
        evm_address: &Address,
        code: &Bytes,
        nonce: U256,
        evm_storage: &[(U256, U256)],
    ) -> Result<Self, StarknetApiError> {
        let nonce = Felt::from(TryInto::<u128>::try_into(nonce).map_err(|err| {
            StarknetApiError::OutOfRange {
                string: err.to_string(),
            }
        })?);

        let evm_address = TryInto::<FeltSequencer>::try_into(*evm_address)
            .unwrap() // infallible
            .into();

        let mut storage = vec![
            starknet_storage!(ACCOUNT_EVM_ADDRESS, evm_address),
            starknet_storage!(ACCOUNT_IS_INITIALIZED, 1u8),
        ];

        // Write the nonce of the account is written to storage after each tx.
        storage.append(&mut vec![starknet_storage!(ACCOUNT_NONCE, nonce)]);

        // Initialize the bytecode storage vars.
        // Assumes that the bytecode is stored as a ByteArray type, following the Store<ByteArray> implementation of
        // the cairo core library
        let mut bytecode_storage = prepare_bytearray_storage(code);
        storage.append(&mut bytecode_storage);

        // Initialize the storage vars.
        let mut evm_storage_storage: Vec<(StorageKey, Felt)> = evm_storage
            .iter()
            .flat_map(|(k, v)| {
                let keys = split_u256(*k).map(Into::into);
                let values = split_u256(*v).map(Into::<Felt>::into);
                let low_key = compute_storage_base_address(ACCOUNT_STORAGE, &keys);
                let high_key = offset_storage_key(low_key, 1);
                vec![(low_key, values[0]), (high_key, values[1])]
            })
            .collect();
        storage.append(&mut evm_storage_storage);

        Ok(Self {
            storage,
            evm_address,
            nonce: Nonce(nonce),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use starknet::core::types::Felt;

    #[test]
    fn test_prepare_bytearray_storage() {
        // Given
        let code = Bytes::from(vec![0x01, 0x02, 0x03, 0x04, 0x05]);
        let bytecode_base_address = get_storage_var_address(ACCOUNT_BYTECODE, &[]);

        // When
        let result = prepare_bytearray_storage(&code);

        // Then
        let expected_result = vec![
            (bytecode_base_address, Felt::from(code.len())),
            (
                offset_storage_key(
                    inner_byte_array_pointer(*bytecode_base_address.0.key(), Felt::ZERO)
                        .try_into()
                        .unwrap(),
                    0,
                ),
                Felt::from(0x0102030405u64),
            ),
        ];

        assert_eq!(result, expected_result);
    }
}
