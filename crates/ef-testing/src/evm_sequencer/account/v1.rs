use blockifier::abi::abi_utils::get_storage_var_address;
use blockifier::abi::sierra_types::next_storage_key;
use reth_primitives::{keccak256, Address, Bytes, KECCAK_EMPTY, U256};
use revm_interpreter::analysis::to_analysed;
use revm_primitives::Bytecode;
use starknet::core::types::Felt;
use starknet_api::{core::Nonce, state::StorageKey, StarknetApiError};

use super::KakarotAccount;
use super::{inner_byte_array_pointer, pack_byte_array_to_starkfelt_array};
use crate::evm_sequencer::constants::storage_variables::{
    ACCOUNT_BYTECODE_LEN, ACCOUNT_CODE_HASH, ACCOUNT_EVM_ADDRESS, ACCOUNT_IS_INITIALIZED,
    ACCOUNT_NONCE, ACCOUNT_STORAGE, ACCOUNT_VALID_JUMPDESTS,
};
use crate::evm_sequencer::{
    constants::storage_variables::ACCOUNT_BYTECODE, types::felt::FeltSequencer, utils::split_u256,
};
use crate::starknet_storage;

impl KakarotAccount {
    pub fn new(
        evm_address: &Address,
        code: &Bytes,
        nonce: U256,
        balance: U256,
        evm_storage: &[(U256, U256)],
    ) -> Result<Self, StarknetApiError> {
        let nonce = Felt::from(TryInto::<u128>::try_into(nonce).map_err(|err| {
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
        let mut bytecode_storage = pack_byte_array_to_starkfelt_array(code)
            .enumerate()
            .map(|(i, bytes)| (StorageKey::from(i as u32), bytes))
            .collect();
        storage.append(&mut bytecode_storage);

        // Initialize the code hash var
        let account_is_empty =
            code.is_empty() && nonce == Felt::from(0) && balance == U256::from(0);
        let code_hash = if account_is_empty {
            U256::from(0)
        } else if code.is_empty() {
            U256::from_be_slice(KECCAK_EMPTY.as_slice())
        } else {
            U256::from_be_slice(keccak256(code).as_slice())
        };

        let code_hash_values = split_u256(code_hash);
        let code_hash_low_key = get_storage_var_address(ACCOUNT_CODE_HASH, &[]);
        let code_hash_high_key = next_storage_key(&code_hash_low_key)?;
        storage.extend([
            (code_hash_low_key, Felt::from(code_hash_values[0])),
            (code_hash_high_key, Felt::from(code_hash_values[1])),
        ]);

        // Initialize the bytecode jumpdests.
        let bytecode = to_analysed(Bytecode::new_raw(code.clone()));
        let valid_jumpdests: Vec<usize> = match bytecode {
            Bytecode::LegacyAnalyzed(legacy_analyzed_bytecode) => legacy_analyzed_bytecode
                .jump_table()
                .0
                .iter()
                .enumerate()
                .filter_map(|(index, bit)| bit.as_ref().then(|| index))
                .collect(),
            _ => unreachable!("Bytecode should be analysed"),
        };

        let jumdpests_storage_address = get_storage_var_address(ACCOUNT_VALID_JUMPDESTS, &[]);
        let jumdpests_storage_address = Felt::from(jumdpests_storage_address);
        valid_jumpdests.into_iter().for_each(|index| {
            storage.push((
                (jumdpests_storage_address + Felt::from(index))
                    .try_into()
                    .unwrap(),
                Felt::ONE,
            ))
        });

        // Initialize the storage vars.
        let mut evm_storage_storage: Vec<(StorageKey, Felt)> = evm_storage
            .iter()
            .flat_map(|(k, v)| {
                let keys = split_u256(*k).map(Into::into);
                let values = split_u256(*v).map(Into::<Felt>::into);
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
