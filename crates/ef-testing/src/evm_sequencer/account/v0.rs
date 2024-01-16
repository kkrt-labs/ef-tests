use cairo_vm::Felt252;
use reth_primitives::{Address, Bytes};
use revm_primitives::U256;
use sequencer::state::StateResult;
use starknet_in_rust::{core::errors::state_errors::StateError, utils::get_storage_var_address};

use super::{AccountType, KakarotAccount};
use crate::evm_sequencer::{
    constants::{
        kkrt_constants_v0::{CONTRACT_ACCOUNT_CLASS_HASH, EOA_CLASS_HASH},
        KAKAROT_ADDRESS,
    },
    utils::{address_to_felt252, compute_starknet_address, split_u256},
};
use crate::starknet_storage;

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

        let evm_address = address_to_felt252(evm_address);

        let mut storage = vec![
            starknet_storage!("evm_address", evm_address),
            starknet_storage!("is_initialized_", 1u8),
            starknet_storage!("Ownable_owner", KAKAROT_ADDRESS.0),
            starknet_storage!("bytecode_len_", code.len() as u32),
            starknet_storage!("kakarot_address", KAKAROT_ADDRESS.0),
        ];

        // Initialize the implementation and nonce based on account type.
        // The account is an EOA if it has no bytecode and no storage (or all storage is zero).
        let has_code_or_storage = !code.is_empty() || evm_storage.iter().any(|x| x.1 != U256::ZERO);
        let account_type = if !has_code_or_storage {
            storage.push(starknet_storage!(
                "_implementation",
                Felt252::from_bytes_be(&EOA_CLASS_HASH.0)
            ));
            AccountType::EOA
        } else {
            storage.append(&mut vec![
                starknet_storage!("nonce", nonce),
                starknet_storage!(
                    "_implementation",
                    Felt252::from_bytes_be(&CONTRACT_ACCOUNT_CLASS_HASH.0)
                ),
            ]);
            AccountType::Contract
        };

        // Initialize the bytecode storage var.
        let mut bytecode_storage = split_bytecode_to_starkfelt(code)
            .enumerate()
            .filter_map(|(i, bytes)| {
                Some((
                    get_storage_var_address("bytecode_", &[Felt252::from(i as u32)]).ok()?,
                    bytes,
                ))
            })
            .collect();
        storage.append(&mut bytecode_storage);

        // Initialize the storage vars.
        let mut evm_storage_storage: Vec<(Felt252, Felt252)> = evm_storage
            .iter()
            .filter_map(|(k, v)| {
                let keys = split_u256(*k).map(Into::into);
                let values = split_u256(*v).map(Into::<Felt252>::into);
                let low_key = get_storage_var_address("storage_", &keys).ok()?;
                let high_key = &low_key + 1u64; // can fail only if low is the max key
                Some(vec![(low_key, values[0]), (high_key, values[1])])
            })
            .flatten()
            .collect();
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

/// Splits a byte array into 16-byte chunks and converts each chunk to a StarkFelt.
pub fn split_bytecode_to_starkfelt(bytecode: &'_ Bytes) -> impl Iterator<Item = Felt252> + '_ {
    bytecode.chunks(16).map(|x| {
        let mut storage_value = [0u8; 16];
        storage_value[..x.len()].copy_from_slice(x);
        Felt252::from(u128::from_be_bytes(storage_value))
    })
}
