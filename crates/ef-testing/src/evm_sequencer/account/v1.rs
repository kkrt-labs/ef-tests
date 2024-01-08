use cairo_vm::felt::Felt252;
use reth_primitives::{Address, Bytes};
use revm_primitives::U256;
use sequencer::state::StateResult;
use starknet_crypto::{poseidon_hash_many, FieldElement};
use starknet_in_rust::{
    core::errors::state_errors::StateError,
    utils::{felt_to_field_element, field_element_to_felt, get_storage_var_address},
};

use super::{AccountType, KakarotAccount};
use crate::evm_sequencer::{
    constants::CHAIN_ID,
    utils::{address_to_felt252, compute_starknet_address, split_u256},
};
use crate::{
    evm_sequencer::{constants::KAKAROT_ADDRESS, evm_state::v1::poseidon_storage_base_address},
    starknet_storage,
};

impl KakarotAccount {
    pub fn new(
        evm_address: &Address,
        code: &Bytes,
        nonce: U256,
        evm_storage: &[(U256, U256)],
    ) -> StateResult<Self> {
        let starknet_address = compute_starknet_address(evm_address);

        let nonce = Felt252::from(
            TryInto::<u128>::try_into(nonce)
                .map_err(|_| StateError::CustomError("nonce bigger than u128::MAX".to_string()))?,
        );

        let evm_address: Felt252 = address_to_felt252(evm_address);

        let mut storage = vec![
            starknet_storage!("kakarot_core_address", KAKAROT_ADDRESS.0.clone()),
            starknet_storage!("evm_address", evm_address.clone()),
        ];

        // Initialize the implementation and nonce based on account type.
        // The account is an EOA if it has no bytecode and no storage (or all storage is zero).
        let has_code_or_storage = !code.is_empty() || evm_storage.iter().any(|x| x.1 != U256::ZERO);
        let account_type = if !has_code_or_storage {
            storage.push((
                get_storage_var_address("chain_id", &[])?,
                Felt252::from(*CHAIN_ID),
            ));
            AccountType::EOA
        } else {
            storage.push(starknet_storage!("contract_account_nonce", nonce.clone()));
            AccountType::Contract
        };

        // Initialize the bytecode storage vars.
        let bytecode_base_address = get_storage_var_address("contract_account_bytecode", &[])?;
        let pending_word_address = &bytecode_base_address - 2u32;
        let pending_word_len_address = &bytecode_base_address - 1u32;
        let pending_word_index = code.len() / 31 * 31;
        let pending_word = &code[pending_word_index..];
        // Assumes that the bytecode is stored in 31 byte chunks using the List type from Alexandria.
        // Given the current implementation of the List type in Alexandria, we store the pending word
        // at base address - 2, the pending word len at base address - 1, and the bytecode len (not including
        // the pending word length) at the base address.
        storage.append(&mut vec![
            (pending_word_address, Felt252::from_bytes_be(pending_word)),
            (
                pending_word_len_address,
                Felt252::from(pending_word.len() as u64),
            ),
            (
                bytecode_base_address.clone(),
                Felt252::from((pending_word_index / 31) as u64),
            ),
        ]);

        // The storage address for the bytecode is computed as poseidon_hash(contract_account_bytecode, index) + offset.
        // In our case, the index is incremented every 256 chunks of 31 bytes.
        let mut bytecode_storage = split_bytecode_to_starkfelt(&code[..pending_word_index])
            .enumerate()
            .filter_map(|(index, b)| {
                let offset = index % 256;
                let index = index / 256;
                let key = poseidon_hash_many(&[
                    felt_to_field_element(&bytecode_base_address).ok()?,
                    FieldElement::from(index),
                ]);
                let key = field_element_to_felt(&key);
                Some((key + offset, b))
            })
            .collect::<Vec<_>>();
        storage.append(&mut bytecode_storage);

        // Initialize the storage vars.
        let mut evm_storage_storage: Vec<(Felt252, Felt252)> = evm_storage
            .iter()
            .flat_map(|(k, v)| {
                let keys = split_u256(*k).map(Into::into);
                let values = split_u256(*v).map(Into::<Felt252>::into);
                let low_key = poseidon_storage_base_address("contract_account_storage_keys", &keys);
                let high_key = &low_key + 1u64;
                vec![(low_key, values[0].clone()), (high_key, values[1].clone())]
            })
            .collect::<Vec<_>>();
        storage.append(&mut evm_storage_storage);

        Ok(Self {
            account_type,
            storage,
            starknet_address,
            evm_address,
            nonce,
        })
    }
}

fn split_bytecode_to_starkfelt(bytecode: &[u8]) -> impl Iterator<Item = Felt252> + '_ {
    bytecode.chunks(31).map(Felt252::from_bytes_be)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_bytecode_to_starkfelt() {
        // Given
        let bytes = Bytes::from([0x01, 0x02, 0x03, 0x04, 0x05]);

        // When
        let result: Vec<_> = split_bytecode_to_starkfelt(&bytes).collect();

        // Then
        assert_eq!(result, vec![Felt252::from(0x0102030405u64)]);
    }
}
