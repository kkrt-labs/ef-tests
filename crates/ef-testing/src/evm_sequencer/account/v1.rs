use blockifier::abi::abi_utils::get_storage_var_address;
use reth_primitives::{Address, Bytes};
use revm_primitives::U256;
use starknet_api::{
    core::{ContractAddress, Nonce, PatriciaKey},
    hash::StarkFelt,
    state::StorageKey,
    StarknetApiError,
};
use starknet_crypto::{poseidon_hash_many, FieldElement};

use super::{AccountType, KakarotAccount};
use crate::evm_sequencer::{
    constants::KAKAROT_ADDRESS,
    evm_state::v1::{compute_storage_base_address, offset_storage_base_address},
};
use crate::evm_sequencer::{
    types::felt::FeltSequencer,
    utils::{compute_starknet_address, split_u256},
};

impl KakarotAccount {
    pub fn new(
        evm_address: &Address,
        code: &Bytes,
        nonce: U256,
        evm_storage: Vec<(U256, U256)>,
    ) -> Result<Self, StarknetApiError> {
        let nonce = StarkFelt::from(TryInto::<u128>::try_into(nonce).map_err(|err| {
            StarknetApiError::OutOfRange {
                string: err.to_string(),
            }
        })?);

        let starknet_address = compute_starknet_address(evm_address);
        let starknet_address = ContractAddress::try_from(starknet_address)?;

        let evm_address = TryInto::<FeltSequencer>::try_into(*evm_address)
            .unwrap()
            .into(); // infallible

        let mut storage = vec![
            (
                get_storage_var_address("kakarot_core_address", &[]),
                *KAKAROT_ADDRESS.0.key(),
            ),
            (get_storage_var_address("evm_address", &[]), evm_address),
        ];

        // Initialize the implementation and nonce based on account type.
        // The account is an EOA if it has no bytecode and no storage (or all storage is zero).
        let has_code_or_storage = !code.is_empty() || evm_storage.iter().any(|x| x.1 != U256::ZERO);
        let account_type = if !has_code_or_storage {
            AccountType::EOA
        } else {
            storage.push((
                get_storage_var_address("contract_account_nonce", &[]),
                nonce,
            ));
            AccountType::Contract
        };

        // Initialize the bytecode storage vars.
        let bytecode_base_address = get_storage_var_address("contract_account_bytecode", &[]);
        let pending_word_address = offset_storage_base_address(bytecode_base_address, -2);
        let pending_word_len_address = offset_storage_base_address(bytecode_base_address, -1);
        let pending_word_index = code.len() / 31 * 31;
        let pending_word = &code[pending_word_index..];
        // Assumes that the bytecode is stored in 31 byte chunks using the List type from Alexandria.
        // Given the current implementation of the List type in Alexandria, we store the pending word
        // at base address - 2, the pending word len at base address - 1, and the bytecode len (not including
        // the pending word length) at the base address.
        storage.append(&mut vec![
            (
                pending_word_address,
                StarkFelt::from(FieldElement::from_byte_slice_be(pending_word).unwrap()), // infallible
            ),
            (
                pending_word_len_address,
                StarkFelt::from(pending_word.len() as u64),
            ),
            (
                bytecode_base_address,
                StarkFelt::from((pending_word_index / 31) as u64),
            ),
        ]);

        // The storage address for the bytecode is computed as poseidon_hash(contract_account_bytecode, index) + offset.
        // In our case, the index is incremented every 256 chunks of 31 bytes.
        let mut bytecode_storage = split_bytecode_to_starkfelt(&code[..pending_word_index].into())
            .into_iter()
            .enumerate()
            .map(|(index, b)| {
                let offset = index % 256;
                let index = index / 256;
                let key = poseidon_hash_many(&[
                    (*bytecode_base_address.0.key()).into(),
                    FieldElement::from(index),
                ]);
                (
                    offset_storage_base_address(
                        StorageKey(PatriciaKey::try_from(StarkFelt::from(key)).unwrap()),
                        offset as i64,
                    ),
                    b,
                )
            })
            .collect::<Vec<_>>();
        storage.append(&mut bytecode_storage);

        // Initialize the storage vars.
        let mut evm_storage_storage: Vec<(StorageKey, StarkFelt)> = evm_storage
            .iter()
            .flat_map(|(k, v)| {
                let keys = split_u256(*k).map(Into::into);
                let values = split_u256(*v).map(Into::<StarkFelt>::into);
                let low_key = compute_storage_base_address("contract_account_storage_keys", &keys);
                let high_key = offset_storage_base_address(low_key, 1);
                vec![(low_key, values[0]), (high_key, values[1])]
            })
            .collect();
        storage.append(&mut evm_storage_storage);

        Ok(Self {
            account_type,
            storage,
            starknet_address,
            evm_address,
            nonce: Nonce(nonce),
        })
    }
}

fn split_bytecode_to_starkfelt(bytecode: &Bytes) -> Vec<StarkFelt> {
    bytecode
        .chunks(31)
        .map(|bytes| StarkFelt::from(FieldElement::from_byte_slice_be(bytes).unwrap())) // infallible
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_bytecode_to_starkfelt() {
        // Given
        let bytes = Bytes::from([0x01, 0x02, 0x03, 0x04, 0x05]);

        // When
        let result = split_bytecode_to_starkfelt(&bytes);

        // Then
        assert_eq!(result, vec![StarkFelt::from(0x0102030405u64)]);
    }
}
