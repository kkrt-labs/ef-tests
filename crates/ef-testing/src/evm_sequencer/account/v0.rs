use blockifier::abi::{abi_utils::get_storage_var_address, sierra_types::next_storage_key};
use reth_primitives::{Address, Bytes, U256};
use starknet_api::core::PatriciaKey;
use starknet_api::{core::Nonce, hash::StarkFelt, state::StorageKey, StarknetApiError};
use starknet_crypto::FieldElement;

use super::{split_bytecode_to_starkfelt, KakarotAccount};
use crate::evm_sequencer::constants::storage_variables::{
    ACCOUNT_BYTECODE_LEN, ACCOUNT_EVM_ADDRESS, ACCOUNT_IS_INITIALIZED, ACCOUNT_JUMPDESTS,
    ACCOUNT_NONCE, ACCOUNT_STORAGE,
};
use crate::evm_sequencer::{types::felt::FeltSequencer, utils::split_u256};
use crate::starknet_storage;

impl KakarotAccount {
    pub fn new(
        evm_address: &Address,
        code: &Bytes,
        nonce: U256,
        evm_storage: &[(U256, U256)],
        is_eoa: bool,
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
            starknet_storage!(ACCOUNT_EVM_ADDRESS, evm_address),
            starknet_storage!(ACCOUNT_IS_INITIALIZED, 1u8),
            starknet_storage!(ACCOUNT_BYTECODE_LEN, code.len() as u32),
        ];

        // Write the nonce of the account is written to storage after each tx.
        storage.append(&mut vec![starknet_storage!(ACCOUNT_NONCE, nonce)]);

        // Initialize the bytecode storage var.
        let mut bytecode_storage = split_bytecode_to_starkfelt(code)
            .enumerate()
            .map(|(i, bytes)| (StorageKey::from(i as u32), bytes))
            .collect();
        storage.append(&mut bytecode_storage);

        // Initialize the bytecode jumpdests.
        let mut valid_jumpdests = analyze(code);
        let jumdpests_storage_address = get_storage_var_address(ACCOUNT_JUMPDESTS, &[]);
        valid_jumpdests.iter().for_each(|index| {
            storage.push((
                StorageKey(
                    PatriciaKey::try_from(StarkFelt::from(
                        FieldElement::from(*jumdpests_storage_address.0.key())
                            + FieldElement::from(*index),
                    ))
                    .unwrap(),
                ),
                StarkFelt::from_u128(1),
            ));
        });

        // Initialize the storage vars.
        let mut evm_storage_storage: Vec<(StorageKey, StarkFelt)> = evm_storage
            .iter()
            .flat_map(|(k, v)| {
                let keys = split_u256(*k).map(Into::into);
                let values = split_u256(*v).map(Into::<StarkFelt>::into);
                let low_key = get_storage_var_address(ACCOUNT_STORAGE, &keys);
                let high_key = next_storage_key(&low_key).unwrap(); // can fail only if low is the max key
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

/// Analyze bytecode to build a jump map.
/// author: REVM <https://github.com/bluealloy/revm/blob/main/crates/interpreter/src/interpreter/analysis.rs#L50>
fn analyze(code: &Bytes) -> Vec<usize> {
    let mut jumps: Vec<usize> = Vec::new();

    let range = code.as_ptr_range();
    let start = range.start;
    let mut iterator = start;
    let end = range.end;
    while iterator < end {
        let opcode = unsafe { *iterator };
        if 0x5b == opcode {
            // SAFETY: jumps are max length of the code
            unsafe { jumps.push(iterator.offset_from(start) as usize) }
            iterator = unsafe { iterator.offset(1) };
        } else {
            let push_offset = opcode.wrapping_sub(0x60);
            if push_offset < 32 {
                // SAFETY: iterator access range is checked in the while loop
                iterator = unsafe { iterator.offset((push_offset + 2) as isize) };
            } else {
                // SAFETY: iterator access range is checked in the while loop
                iterator = unsafe { iterator.offset(1) };
            }
        }
    }

    jumps
}
