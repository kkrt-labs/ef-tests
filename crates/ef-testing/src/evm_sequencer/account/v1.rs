use blockifier::abi::abi_utils::get_storage_var_address;
use reth_primitives::{Address, Bytes, U256};
use starknet_api::{
    core::{ContractAddress, Nonce, PatriciaKey},
    hash::StarkFelt,
    state::StorageKey,
    StarknetApiError,
};
use starknet_crypto::{poseidon_hash_many, FieldElement};

use super::split_bytecode_to_starkfelt;
use super::{AccountType, KakarotAccount};
use crate::evm_sequencer::{
    constants::CHAIN_ID,
    types::felt::FeltSequencer,
    utils::{compute_starknet_address, split_u256},
};
use crate::{
    evm_sequencer::{
        constants::KAKAROT_ADDRESS,
        evm_state::v1::{compute_storage_base_address, offset_storage_key},
    },
    starknet_storage,
};

impl KakarotAccount {
    pub fn new(
        evm_address: &Address,
        code: &Bytes,
        nonce: U256,
        evm_storage: &[(U256, U256)],
    ) -> Result<Self, StarknetApiError> {
        let nonce = StarkFelt::from(TryInto::<u128>::try_into(nonce).map_err(|err| {
            StarknetApiError::OutOfRange {
                string: err.to_string(),
            }
        })?);

        let starknet_address = compute_starknet_address(evm_address);
        let starknet_address = ContractAddress::try_from(starknet_address)?;

        let evm_address = TryInto::<FeltSequencer>::try_into(*evm_address)
            .unwrap() // infallible
            .into();

        let mut storage = vec![
            starknet_storage!("kakarot_core_address", *KAKAROT_ADDRESS.0.key()),
            starknet_storage!("evm_address", evm_address),
        ];

        // Initialize the implementation and nonce based on account type.
        // The account is an EOA if it has no bytecode and no storage (or all storage is zero).
        let has_code_or_storage = !code.is_empty() || evm_storage.iter().any(|x| x.1 != U256::ZERO);
        let account_type = if !has_code_or_storage {
            storage.push((
                get_storage_var_address("chain_id", &[]),
                StarkFelt::from(*CHAIN_ID),
            ));
            AccountType::EOA
        } else {
            storage.push(starknet_storage!("contract_account_nonce", nonce));
            AccountType::Contract
        };

        // Initialize the bytecode storage vars.
        let bytecode_base_address = get_storage_var_address("contract_account_bytecode", &[]);
        let pending_word_address = offset_storage_key(bytecode_base_address, -2);
        let pending_word_len_address = offset_storage_key(bytecode_base_address, -1);
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
        let mut bytecode_storage = split_bytecode_to_starkfelt(&code[..pending_word_index])
            .enumerate()
            .map(|(index, b)| {
                let offset = index % 256;
                let index = index / 256;
                let key = poseidon_hash_many(&[
                    (*bytecode_base_address.0.key()).into(),
                    FieldElement::from(index),
                ]);
                (
                    offset_storage_key(
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
                let high_key = offset_storage_key(low_key, 1);
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
