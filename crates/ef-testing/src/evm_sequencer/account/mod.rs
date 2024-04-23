#[cfg(feature = "v0")]
pub mod v0;
#[cfg(feature = "v1")]
pub mod v1;

use starknet::core::utils::cairo_short_string_to_felt;
use starknet_api::{core::Nonce, hash::StarkFelt, state::StorageKey};
use starknet_crypto::{poseidon_permute_comp, FieldElement};

#[macro_export]
macro_rules! starknet_storage {
    ($storage_var: expr, $felt: expr) => {
        (
            get_storage_var_address($storage_var, &[]),
            StarkFelt::from($felt),
        )
    };
    ($storage_var: expr, [$($key: expr),*], $felt: expr) => {
        {
            let args = vec![$($key),*];
            (
                get_storage_var_address($storage_var, &args),
                StarkFelt::from($felt),
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
    pub(crate) evm_address: StarkFelt,
    pub(crate) nonce: Nonce,
    pub(crate) storage: Vec<(StorageKey, StarkFelt)>,
    pub(crate) account_type: AccountType,
}

impl KakarotAccount {
    pub const fn evm_address(&self) -> &StarkFelt {
        &self.evm_address
    }

    pub const fn nonce(&self) -> &Nonce {
        &self.nonce
    }

    pub fn storage(&self) -> &[(StorageKey, StarkFelt)] {
        self.storage.as_slice()
    }

    pub const fn account_type(&self) -> &AccountType {
        &self.account_type
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
    use super::{AccountType, KakarotAccount};
    use reth_primitives::{Address, Bytes, U256};
    use starknet_api::{core::Nonce, hash::StarkFelt, StarknetApiError};

    impl KakarotAccount {
        pub fn new(
            _evm_address: &Address,
            _code: &Bytes,
            _nonce: U256,
            _evm_storage: &[(U256, U256)],
            _is_eoa: bool,
        ) -> Result<Self, StarknetApiError> {
            Ok(Self {
                evm_address: StarkFelt::default(),
                nonce: Nonce::default(),
                storage: vec![],
                account_type: AccountType::EOA,
            })
        }
    }
}

/// Splits a byte array into 31-byte chunks and converts each chunk to a StarkFelt.
pub fn split_bytecode_to_starkfelt(bytecode: &[u8]) -> impl Iterator<Item = StarkFelt> + '_ {
    bytecode.chunks(31).filter_map(|bytes| {
        let f = FieldElement::from_byte_slice_be(bytes);
        f.map(StarkFelt::from).ok()
    })
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
pub fn inner_byte_array_pointer(
    base_address: FieldElement,
    storage_segment: FieldElement,
) -> FieldElement {
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
    fn test_split_bytecode_to_starkfelt() {
        // Given
        let bytes = Bytes::from([0x01, 0x02, 0x03, 0x04, 0x05]);

        // When
        let result: Vec<_> = split_bytecode_to_starkfelt(&bytes).collect();

        // Then
        assert_eq!(result, vec![StarkFelt::from(0x0102030405u64)]);
    }

    #[test]
    fn test_inner_byte_array_pointer() {
        // Given
        let base_address: StarkFelt = get_storage_var_address(ACCOUNT_BYTECODE, &[]).into();
        let chunk = FieldElement::ZERO;

        // When
        let result = inner_byte_array_pointer(base_address.into(), chunk);

        // Then
        assert_eq!(
            result,
            FieldElement::from_hex_be(
                "0x030dc4fd6786155d4743a0f56ea73bea9521eba2552a2ca5080b830ad047907a"
            )
            .unwrap()
        );
    }
}
