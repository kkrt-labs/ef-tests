use blockifier::abi::{abi_utils::get_storage_var_address, sierra_types::next_storage_key};
use reth_primitives::{Address, U256};
use revm_interpreter::analysis::to_analysed;
use revm_primitives::{Bytecode, BytecodeState, Bytes, JumpMap};
use starknet_api::core::PatriciaKey;
use starknet_api::{core::Nonce, hash::StarkFelt, state::StorageKey, StarknetApiError};
use starknet_crypto::FieldElement;

use super::{split_bytecode_to_starkfelt, KakarotAccount};
use crate::evm_sequencer::constants::storage_variables::{
    ACCOUNT_BYTECODE_LEN, ACCOUNT_EVM_ADDRESS, ACCOUNT_IS_INITIALIZED, ACCOUNT_NONCE,
    ACCOUNT_STORAGE, ACCOUNT_VALID_JUMPDESTS,
};
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
        let mut bytecode = to_analysed(Bytecode::new_raw(code.clone()));
        let valid_jumpdests = match bytecode.state {
            BytecodeState::Raw | BytecodeState::Checked { .. } => Vec::new(),
            BytecodeState::Analysed { jump_map, .. } => Vec::from(jump_map.as_slice()),
        };

        let jumdpests_storage_address = get_storage_var_address(ACCOUNT_VALID_JUMPDESTS, &[]);
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
