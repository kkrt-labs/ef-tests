use blockifier::abi::abi_utils::{
    get_erc20_balance_var_addresses, get_storage_var_address, get_uint256_storage_var_addresses,
};
use blockifier::execution::errors::EntryPointExecutionError;
use blockifier::state::state_api::{State, StateReader, StateResult};
use blockifier::transaction::errors::TransactionExecutionError;
use blockifier::transaction::objects::{TransactionExecutionInfo, TransactionExecutionResult};
use reth_primitives::{Address, Bytes, TransactionSigned};
use revm_primitives::U256;
use sequencer::execution::Execution as _;
use sequencer::transaction::BroadcastedTransactionWrapper;
use starknet::core::types::{BroadcastedTransaction, FieldElement};
use starknet_api::core::Nonce;
use starknet_api::hash::StarkFelt;
use starknet_api::state::StorageKey;

use super::account::{AccountType, KakarotAccount};
use super::Evm;
use super::EvmState;
use crate::evm_sequencer::constants::kkrt_constants_v0::{
    CONTRACT_ACCOUNT_CLASS_HASH, EOA_CLASS_HASH, PROXY_CLASS_HASH,
};
use crate::evm_sequencer::constants::KAKAROT_ADDRESS;
use crate::evm_sequencer::constants::{CHAIN_ID, ETH_FEE_TOKEN_ADDRESS};
use crate::evm_sequencer::sequencer::KakarotSequencer;
use crate::evm_sequencer::types::felt::FeltSequencer;
use crate::evm_sequencer::utils::{
    compute_starknet_address, high_16_bytes_of_felt_to_bytes, split_u256,
    to_broadcasted_starknet_transaction,
};

pub struct KakarotConfig {
    pub(crate) address: StarkFelt,
    pub(crate) eoa_class_hash: StarkFelt,
    pub(crate) contract_account_class_hash: StarkFelt,
}

impl Default for KakarotConfig {
    fn default() -> Self {
        Self {
            address: *KAKAROT_ADDRESS.0.key(),
            eoa_class_hash: EOA_CLASS_HASH.0,
            contract_account_class_hash: CONTRACT_ACCOUNT_CLASS_HASH.0,
        }
    }
}

impl Evm for KakarotSequencer {
    /// Sets up an EOA or contract account. Writes nonce, code and storage to the sequencer storage.
    fn setup_account(
        &mut self,
        evm_address: &Address,
        bytecode: &Bytes,
        nonce: U256,
        evm_storage: Vec<(U256, U256)>,
    ) -> StateResult<()> {
        let nonce = StarkFelt::from(TryInto::<u128>::try_into(nonce).map_err(|err| {
            StarknetApiError::OutOfRange {
                string: err.to_string(),
            }
        })?);
        let starknet_address = compute_starknet_address(evm_address);
        let evm_address = TryInto::<FeltSequencer>::try_into(*evm_address)
            .unwrap()
            .into(); // infallible

        let mut storage = vec![
            (("evm_address", vec![]), evm_address),
            (("is_initialized_", vec![]), StarkFelt::from(1u8)),
            (("Ownable_owner", vec![]), *KAKAROT_ADDRESS.0.key()),
            (
                ("bytecode_len_", vec![]),
                StarkFelt::from(bytecode.len() as u32),
            ),
            (("kakarot_address", vec![]), *KAKAROT_ADDRESS.0.key()),
        ];

        let starknet_address = starknet_address.try_into()?;
        // Initialize the implementation and nonce based on account type.
        // The account is an EOA if it has no bytecode and no storage (or all storage is zero).
        if bytecode.is_empty() && evm_storage.iter().all(|x| x.1 == U256::ZERO) {
            storage.push((("_implementation", vec![]), EOA_CLASS_HASH.0));
            self.state.set_nonce(starknet_address, Nonce(nonce));
        } else {
            storage.append(&mut vec![
                (("nonce", vec![]), nonce),
                (("_implementation", vec![]), CONTRACT_ACCOUNT_CLASS_HASH.0),
            ]);
        }

        // Initialize the bytecode storage var.
        let bytecode_storage = &mut split_bytecode_to_starkfelt(bytecode)
            .into_iter()
            .enumerate()
            .map(|(i, bytes)| (("bytecode_", vec![StarkFelt::from(i as u32)]), bytes))
            .collect();
        storage.append(bytecode_storage);

        // Initialize the storage vars.
        let evm_storage_storage: Vec<(StorageKey, StarkFelt)> = evm_storage
            .iter()
            .flat_map(|(k, v)| {
                let keys = split_u256(*k).map(Into::into);
                let values = split_u256(*v).map(Into::<StarkFelt>::into);
                let keys = get_uint256_storage_var_addresses("storage_", &keys).unwrap(); // safe unwrap: all vars are ASCII
                vec![(keys.0, values[0]), (keys.1, values[1])]
            })
            .collect();
        for (k, v) in evm_storage_storage {
            (&mut self.state).set_storage_at(starknet_address, k, v);
        }

        // Write all the storage vars to the sequencer state.
        for ((var, keys), v) in storage {
            (&mut self.state).set_storage_at(
                starknet_address,
                get_storage_var_address(var, &keys),
                v,
            );
        }

        // Set up the contract class hash.
        (&mut self.state).set_class_hash_at(starknet_address, *PROXY_CLASS_HASH)?;

        // Add the address to the Kakarot evm to starknet mapping
        (&mut self.state).set_storage_at(
            *KAKAROT_ADDRESS,
            get_storage_var_address("evm_to_starknet_address", &[account.evm_address]),
            *account.starknet_address.0.key(),
        );
        Ok(())
    }

    /// Funds an EOA or contract account. Also gives allowance to the Kakarot contract.
    fn fund(&mut self, evm_address: &Address, balance: U256) -> StateResult<()> {
        let starknet_address = compute_starknet_address(evm_address);
        let balance_values = split_u256(balance);
        let mut storage = vec![];

        // Initialize the balance storage var.
        let balance_keys = get_erc20_balance_var_addresses(&starknet_address.try_into()?)?;
        let balance_keys: [StorageKey; 2] = balance_keys.into();
        let balance_storage = &mut balance_keys
            .into_iter()
            .zip(balance_values)
            .map(|(k, v)| (k, StarkFelt::from(v)))
            .collect();
        storage.append(balance_storage);

        // Initialize the allowance storage var.
        let allowance_keys = get_uint256_storage_var_addresses(
            "ERC20_allowances",
            &[starknet_address.into(), *KAKAROT_ADDRESS.0.key()],
        )?;
        let allowance_keys: [StorageKey; 2] = allowance_keys.into();
        let allowance_storage = &mut allowance_keys
            .into_iter()
            .map(|k| (k, StarkFelt::from(u128::MAX)))
            .collect();
        storage.append(allowance_storage);

        // Write all the storage vars to the sequencer state.
        for (k, v) in storage {
            (&mut self.state).set_storage_at(*ETH_FEE_TOKEN_ADDRESS, k, v);
        }
        Ok(())
    }

    /// Returns the storage value at the given key evm storage key.
    fn get_storage_at(&mut self, evm_address: &Address, key: U256) -> StateResult<U256> {
        let keys = split_u256(key).map(Into::into);
        let keys = get_uint256_storage_var_addresses("storage_", &keys).unwrap(); // safe unwrap: all vars are ASCII

        let starknet_address = compute_starknet_address(evm_address);

        let low = (&mut self.state).get_storage_at(starknet_address.try_into()?, keys.0)?;
        let high = (&mut self.state).get_storage_at(starknet_address.try_into()?, keys.1)?;

        let low = U256::from_be_bytes(Into::<FieldElement>::into(low).to_bytes_be());
        let high = U256::from_be_bytes(Into::<FieldElement>::into(high).to_bytes_be());

        Ok(high << 128 | low)
    }

    /// Returns the nonce of the given address. For an EOA, uses the protocol level nonce.
    /// For a contract account, uses the Kakarot managed nonce stored in the contract account's storage.
    fn get_nonce_at(&mut self, evm_address: &Address) -> StateResult<U256> {
        let starknet_address = compute_starknet_address(evm_address);

        let implementation = (&mut self.state)
            .get_storage_at(
                starknet_address.try_into()?,
                get_storage_var_address("_implementation", &[]),
            )
            .unwrap();

        let nonce = if implementation == EOA_CLASS_HASH.0 {
            (&mut self.state)
                .get_nonce_at(starknet_address.try_into()?)?
                .0
        } else if implementation == CONTRACT_ACCOUNT_CLASS_HASH.0 {
            let key = get_storage_var_address("nonce", &[]);
            (&mut self.state).get_storage_at(starknet_address.try_into()?, key)?
        } else {
            // We can't throw an error here, because it could just be an uninitialized account.
            StarkFelt::from(0_u8)
        };

        Ok(U256::from_be_bytes(
            Into::<FieldElement>::into(nonce).to_bytes_be(),
        ))
    }

    /// Returns the bytecode of the given address. For an EOA, the bytecode_len_ storage variable will return 0,
    /// and the function will return an empty vector. For a contract account, the function will return the bytecode
    /// stored in the bytecode_ storage variables. The function assumes that the bytecode is stored in 16 byte big-endian chunks.
    fn get_code_at(&mut self, evm_address: &Address) -> StateResult<Bytes> {
        let starknet_address = compute_starknet_address(evm_address);

        let bytecode_len = (&mut self.state).get_storage_at(
            starknet_address.try_into()?,
            get_storage_var_address("bytecode_len_", &[]),
        )?;
        let bytecode_len: u64 = bytecode_len.try_into()?;
        if bytecode_len == 0 {
            return Ok(Bytes::default());
        }

        // Assumes that the bytecode is stored in 16 byte chunks.
        let num_chunks = bytecode_len / 16;
        let mut bytecode: Vec<u8> = Vec::new();

        for chunk_index in 0..num_chunks {
            let key = get_storage_var_address("bytecode_", &[StarkFelt::from(chunk_index)]);
            let code = (&mut self.state).get_storage_at(starknet_address.try_into()?, key)?;
            bytecode.append(&mut high_16_bytes_of_felt_to_bytes(&code.into(), 16).to_vec());
        }

        let remainder = bytecode_len % 16;
        let key = get_storage_var_address("bytecode_", &[StarkFelt::from(num_chunks)]);
        let code = (&mut self.state).get_storage_at(starknet_address.try_into()?, key)?;
        bytecode
            .append(&mut high_16_bytes_of_felt_to_bytes(&code.into(), remainder as usize).to_vec());

        Ok(Bytes::from(bytecode))
    }

    /// Returns the balance of native tokens at the given address.
    /// Makes use of the default StateReader implementation from Blockifier.
    fn get_balance_at(&mut self, evm_address: &Address) -> StateResult<U256> {
        let starknet_address = compute_starknet_address(evm_address);
        let (low, high) = (&mut self.state)
            .get_fee_token_balance(&starknet_address.try_into()?, &ETH_FEE_TOKEN_ADDRESS)?;

        let low = U256::from_be_bytes(Into::<FieldElement>::into(low).to_bytes_be());
        let high = U256::from_be_bytes(Into::<FieldElement>::into(high).to_bytes_be());

        Ok(high << 128 | low)
    }

    /// Converts the given signed transaction to a Starknet-rs transaction and executes it.
    fn execute_transaction(
        &mut self,
        transaction: TransactionSigned,
    ) -> TransactionExecutionResult<TransactionExecutionInfo> {
        let starknet_transaction =
            BroadcastedTransactionWrapper::new(BroadcastedTransaction::Invoke(
                to_broadcasted_starknet_transaction(&transaction).map_err(|err| {
                    TransactionExecutionError::ValidateTransactionError(
                        EntryPointExecutionError::InvalidExecutionInput {
                            input_descriptor: String::from("Signed transaction"),
                            info: err.to_string(),
                        },
                    )
                })?,
            ));

        self.execute(
            starknet_transaction
                .try_into_execution_transaction(FieldElement::from(*CHAIN_ID))
                .unwrap(),
        )
    }
}

/// Splits a byte array into 16-byte chunks and converts each chunk to a StarkFelt.
fn split_bytecode_to_starkfelt(bytecode: &Bytes) -> Vec<StarkFelt> {
    bytecode
        .chunks(16)
        .map(|x| {
            let mut storage_value = [0u8; 16];
            storage_value[..x.len()].copy_from_slice(x);
            StarkFelt::from(u128::from_be_bytes(storage_value))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evm_sequencer::{
        constants::{
            tests::{PRIVATE_KEY, PUBLIC_KEY, TEST_CONTRACT_ADDRESS},
            CHAIN_ID,
        },
        sequencer::InitializeSequencer,
    };
    use blockifier::{abi::abi_utils::get_storage_var_address, state::state_api::StateReader};
    use reth_primitives::{sign_message, AccessList, Signature, TransactionSigned, TxEip1559};
    use revm_primitives::B256;
    use sequencer::state::State as SequencerState;
    use starknet_api::hash::StarkFelt;

    #[test]
    fn test_execute_simple_contract() {
        // Given
        let sequencer =
            crate::evm_sequencer::sequencer::KakarotSequencer::new(SequencerState::default());
        let mut sequencer = sequencer.initialize().unwrap();

        let mut transaction = TransactionSigned {
            hash: B256::default(),
            signature: Signature::default(),
            transaction: reth_primitives::Transaction::Eip1559(TxEip1559 {
                chain_id: *CHAIN_ID,
                nonce: 0,
                gas_limit: 1_000_000,
                max_fee_per_gas: 0,
                max_priority_fee_per_gas: 0,
                to: reth_primitives::TransactionKind::Call(*TEST_CONTRACT_ADDRESS),
                value: 0,
                access_list: AccessList::default(),
                input: Bytes::default(),
            }),
        };
        let signature =
            sign_message(*PRIVATE_KEY, transaction.transaction.signature_hash()).unwrap();
        transaction.signature = signature;
        let kakarot_config = KakarotConfig::default();
        let bytecode = Bytes::from(vec![96, 1, 96, 0, 85]); // PUSH 01 PUSH 00 SSTORE
        let nonce = U256::from(0);

        // When
        let contract = KakarotAccount::new(
            &kakarot_config,
            &TEST_CONTRACT_ADDRESS,
            &bytecode,
            nonce,
            vec![],
        )
        .unwrap();
        let eoa = KakarotAccount::new(
            &kakarot_config,
            &PUBLIC_KEY,
            &Bytes::default(),
            nonce,
            vec![],
        )
        .unwrap();
        sequencer.setup_account(contract).unwrap();
        sequencer.setup_account(eoa).unwrap();
        sequencer.execute_transaction(transaction).unwrap();

        // Then
        let contract_starknet_address = compute_starknet_address(&TEST_CONTRACT_ADDRESS)
            .try_into()
            .unwrap();
        let storage = (&mut sequencer.state)
            .get_storage_at(
                contract_starknet_address,
                get_storage_var_address("storage_", &[StarkFelt::from(0u8), StarkFelt::from(0u8)]),
            )
            .unwrap();
        assert_eq!(storage, StarkFelt::from(1u8));
    }
}
