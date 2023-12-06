use blockifier::abi::{abi_utils::get_storage_var_address, sierra_types::next_storage_key};
use reth_primitives::{Address, Bytes};
use revm_primitives::U256;
use starknet_api::{
    core::{ContractAddress, Nonce},
    hash::StarkFelt,
    state::StorageKey,
    StarknetApiError,
};

use super::{AccountType, KakarotAccount};
use crate::evm_sequencer::{
    constants::{
        kkrt_constants_v0::{CONTRACT_ACCOUNT_CLASS_HASH, EOA_CLASS_HASH},
        KAKAROT_ADDRESS,
    },
    types::felt::FeltSequencer,
    utils::{compute_starknet_address, split_u256},
};
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

        let starknet_address = compute_starknet_address(evm_address);
        let starknet_address = ContractAddress::try_from(starknet_address)?;

        let evm_address = TryInto::<FeltSequencer>::try_into(*evm_address)
            .unwrap() // infallible
            .into();

        let mut storage = vec![
            starknet_storage!("evm_address", evm_address),
            starknet_storage!("is_initialized_", 1u8),
            starknet_storage!("Ownable_owner", *KAKAROT_ADDRESS.0.key()),
            starknet_storage!("bytecode_len_", code.len() as u32),
            starknet_storage!("kakarot_address", *KAKAROT_ADDRESS.0.key()),
        ];

        // Initialize the implementation and nonce based on account type.
        // The account is an EOA if it has no bytecode and no storage (or all storage is zero).
        let has_code_or_storage = !code.is_empty() || evm_storage.iter().any(|x| x.1 != U256::ZERO);
        let account_type = if !has_code_or_storage {
            storage.push(starknet_storage!("_implementation", EOA_CLASS_HASH.0));
            AccountType::EOA
        } else {
            storage.append(&mut vec![
                starknet_storage!("nonce", nonce),
                starknet_storage!("_implementation", CONTRACT_ACCOUNT_CLASS_HASH.0),
            ]);
            AccountType::Contract
        };

        // Initialize the bytecode storage var.
        let mut bytecode_storage = split_bytecode_to_starkfelt(code)
            .enumerate()
            .map(|(i, bytes)| starknet_storage!("bytecode_", [StarkFelt::from(i as u32)], bytes))
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
            starknet_address,
            evm_address,
            nonce: Nonce(nonce),
        })
    }
}

/// Splits a byte array into 16-byte chunks and converts each chunk to a StarkFelt.
pub fn split_bytecode_to_starkfelt<'a>(
    bytecode: &'a Bytes,
) -> impl Iterator<Item = StarkFelt> + 'a {
    bytecode.chunks(16).map(|x| {
        let mut storage_value = [0u8; 16];
        storage_value[..x.len()].copy_from_slice(x);
        StarkFelt::from(u128::from_be_bytes(storage_value))
    })
}
