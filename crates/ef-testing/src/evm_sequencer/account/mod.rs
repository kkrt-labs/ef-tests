#[cfg(feature = "v0")]
pub mod v0;
#[cfg(feature = "v1")]
pub mod v1;

use starknet::core::utils::cairo_short_string_to_felt;
use starknet_api::{core::Nonce, state::StorageKey};
use starknet_crypto::{poseidon_permute_comp, Felt};

#[macro_export]
macro_rules! starknet_storage {
    ($storage_var: expr, $felt: expr) => {
        (
            get_storage_var_address($storage_var, &[]),
            Felt::from($felt),
        )
    };
    ($storage_var: expr, [$($key: expr),*], $felt: expr) => {
        {
            let args = vec![$($key),*];
            (
                get_storage_var_address($storage_var, &args),
                Felt::from($felt),
            )
        }
    };
}

/// Structure representing a Kakarot account.
/// Contains a nonce, Starknet storage, account
/// type, evm address and starknet address.
#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct KakarotAccount {
    pub(crate) evm_address: Felt,
    pub(crate) nonce: Nonce,
    pub(crate) storage: Vec<(StorageKey, Felt)>,
}

impl KakarotAccount {
    pub const fn evm_address(&self) -> &Felt {
        &self.evm_address
    }

    pub const fn nonce(&self) -> &Nonce {
        &self.nonce
    }

    pub fn storage(&self) -> &[(StorageKey, Felt)] {
        self.storage.as_slice()
    }
}

#[derive(Debug, Default, Clone)]
pub enum AccountType {
    #[default]
    Uninitialized = 0,
    EOA = 1,
    Contract = 2,
}

#[cfg(not(any(feature = "v0", feature = "v1")))]
pub mod kkrt_account {
    use super::KakarotAccount;
    use reth_primitives::{Address, Bytes, U256};
    use starknet::core::types::Felt;
    use starknet_api::{core::Nonce, StarknetApiError};

    impl KakarotAccount {
        pub fn new(
            _evm_address: &Address,
            _code: &Bytes,
            _nonce: U256,
            _balance: U256,
            _evm_storage: &[(U256, U256)],
        ) -> Result<Self, StarknetApiError> {
            Ok(Self {
                evm_address: Felt::default(),
                nonce: Nonce::default(),
                storage: vec![],
            })
        }
    }
}

/// Splits a byte array into 31-byte chunks and converts each chunk to a Felt.
pub fn pack_byte_array_to_starkfelt_array(bytes: &[u8]) -> impl Iterator<Item = Felt> + '_ {
    bytes.chunks(31).map(Felt::from_bytes_be_slice)
}

/// Computes the inner pointer of a byte array in storage.
///
/// The pointer is determined by the hash of:
/// - The base address of the byte array.
/// - The storage segment.
/// - The short string `ByteArray`.
///
/// # Arguments
/// * `base_address` - The base address of the byte array.
/// * `storage_segment` - The index of the storage segment to compute the pointer for. Each segment should store at most 256 * 31 bytes
///
/// # Returns
/// The inner pointer of the byte array.
pub fn inner_byte_array_pointer(base_address: Felt, storage_segment: Felt) -> Felt {
    let suffix = cairo_short_string_to_felt("ByteArray").unwrap();
    let mut state = [base_address, storage_segment, suffix];
    poseidon_permute_comp(&mut state);
    state[0]
}

#[cfg(test)]
mod tests {
    use crate::evm_sequencer::constants::storage_variables::ACCOUNT_BYTECODE;

    use super::*;
    use blockifier::abi::abi_utils::get_storage_var_address;
    use reth_primitives::Bytes;

    #[test]
    fn test_pack_byte_array_to_starkfelt_array() {
        // Given
        let bytes = Bytes::from([0x01, 0x02, 0x03, 0x04, 0x05]);

        // When
        let result: Vec<_> = pack_byte_array_to_starkfelt_array(&bytes).collect();

        // Then
        assert_eq!(result, vec![Felt::from(0x0102030405u64)]);
    }

    #[test]
    fn test_inner_byte_array_pointer() {
        // Given
        let base_address: Felt = get_storage_var_address(ACCOUNT_BYTECODE, &[]).into();
        let chunk = Felt::ZERO;

        // When
        let result = inner_byte_array_pointer(base_address, chunk);

        // Then
        assert_eq!(
            result,
            Felt::from_hex("0x030dc4fd6786155d4743a0f56ea73bea9521eba2552a2ca5080b830ad047907a")
                .unwrap()
        );
    }
}
