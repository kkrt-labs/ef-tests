use blockifier::abi::{abi_utils::get_storage_var_address, sierra_types::next_storage_key};
use reth_primitives::{Address, Bytes, U256};
use starknet_api::{core::Nonce, hash::StarkFelt, state::StorageKey, StarknetApiError};

use super::{split_bytecode_to_starkfelt, AccountType, KakarotAccount};
use crate::evm_sequencer::{types::felt::FeltSequencer, utils::split_u256};
use crate::starknet_storage;

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

        let evm_address = TryInto::<FeltSequencer>::try_into(*evm_address)
            .unwrap() // infallible
            .into();

        let mut storage = vec![
            starknet_storage!("evm_address", evm_address),
            starknet_storage!("is_initialized_", 1u8),
            starknet_storage!("bytecode_len_", code.len() as u32),
        ];

        // Initialize the implementation and nonce based on account type.
        // The account is an EOA if it has no bytecode and no storage (or all storage is zero).
        let has_code_or_storage = !code.is_empty() || evm_storage.iter().any(|x| x.1 != U256::ZERO);
        let account_type = if !has_code_or_storage {
            AccountType::EOA
        } else {
            storage.append(&mut vec![starknet_storage!("nonce", nonce)]);
            AccountType::Contract
        };

        // Initialize the bytecode storage var.
        let mut bytecode_storage = split_bytecode_to_starkfelt(code)
            .enumerate()
            .map(|(i, bytes)| (StorageKey::from(i as u32), bytes))
            .collect();
        storage.append(&mut bytecode_storage);

        // Initialize the storage vars.
        let mut evm_storage_storage: Vec<(StorageKey, StarkFelt)> = evm_storage
            .iter()
            .flat_map(|(k, v)| {
                let keys = split_u256(*k).map(Into::into);
                let values = split_u256(*v).map(Into::<StarkFelt>::into);
                let low_key = get_storage_var_address("storage_", &keys);
                let high_key = next_storage_key(&low_key).unwrap(); // can fail only if low is the max key
                vec![(low_key, values[0]), (high_key, values[1])]
            })
            .collect();
        storage.append(&mut evm_storage_storage);

        Ok(Self {
            account_type,
            storage,
            evm_address,
            nonce: Nonce(nonce),
        })
    }
}
