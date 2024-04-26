use core::num;

use blockifier::{
    abi::{
        abi_utils::{get_fee_token_var_address, get_storage_var_address, starknet_keccak},
        sierra_types::next_storage_key,
    },
    execution::{
        errors::EntryPointExecutionError,
        execution_utils::{felt_to_stark_felt, stark_felt_to_felt},
    },
    state::state_api::{State as _, StateReader as _, StateResult},
    transaction::{
        errors::TransactionExecutionError,
        objects::{TransactionExecutionInfo, TransactionExecutionResult},
    },
};
use cairo_vm::felt::Felt252;
use num_integer::Integer;
use reth_primitives::{Address, Bytes, TransactionSigned, U256};
use sequencer::{execution::Execution as _, transaction::BroadcastedTransactionWrapper};
use starknet::core::types::BroadcastedTransaction;
use starknet_api::{
    core::{PatriciaKey, L2_ADDRESS_UPPER_BOUND},
    hash::StarkFelt,
    state::StorageKey,
};
use starknet_crypto::{poseidon_hash_many, FieldElement};

use super::Evm;
use crate::{
    evm_sequencer::{
        account::{inner_byte_array_pointer, AccountType, KakarotAccount},
        constants::{
            storage_variables::{
                ACCOUNT_BYTECODE, ACCOUNT_IMPLEMENTATION, ACCOUNT_NONCE, ACCOUNT_STORAGE,
                KAKAROT_BASE_FEE, KAKAROT_BLOCK_GAS_LIMIT, KAKAROT_COINBASE,
                KAKAROT_EVM_TO_STARKNET_ADDRESS, KAKAROT_PREV_RANDAO, OWNABLE_OWNER,
            },
            ETH_FEE_TOKEN_ADDRESS, KAKAROT_ADDRESS,
        },
        sequencer::KakarotSequencer,
        types::felt::FeltSequencer,
        utils::{split_u256, to_broadcasted_starknet_transaction},
    },
    starknet_storage,
};

impl Evm for KakarotSequencer {
    /// Sets up the evm state (coinbase, block number, etc.)
    fn setup_state(
        &mut self,
        base_fee: U256,
        prev_randao: U256,
        block_gas_limit: U256,
    ) -> StateResult<()> {
        let kakarot_address = self.environment.kakarot_address;
        let coinbase_address: FeltSequencer = (*self.address()).try_into().unwrap(); // infallible

        // Set the coinbase address.
        self.state_mut().set_storage_at(
            kakarot_address,
            get_storage_var_address(KAKAROT_COINBASE, &[]),
            coinbase_address.into(),
        )?;

        // Set the base fee.
        let [low_fee, high_fee] = split_u256(base_fee);
        let basefee_address = get_storage_var_address(KAKAROT_BASE_FEE, &[]);
        self.state_mut().set_storage_at(
            kakarot_address,
            basefee_address,
            StarkFelt::from(low_fee),
        )?;
        self.state_mut().set_storage_at(
            kakarot_address,
            next_storage_key(&basefee_address)?,
            StarkFelt::from(high_fee),
        )?;

        // Set the previous randao.
        let [low_prev_randao, high_prev_randao] = split_u256(prev_randao);
        let prev_randao_address = get_storage_var_address(KAKAROT_PREV_RANDAO, &[]);
        self.state_mut().set_storage_at(
            kakarot_address,
            prev_randao_address,
            StarkFelt::from(low_prev_randao),
        )?;
        self.state_mut().set_storage_at(
            kakarot_address,
            next_storage_key(&prev_randao_address)?,
            StarkFelt::from(high_prev_randao),
        )?;

        // Set the block gas limit, considering it fits in a felt.
        let [block_gas_limit, _] = split_u256(block_gas_limit);
        let block_gas_limit = StarkFelt::from(block_gas_limit);
        let block_gas_limit_address = get_storage_var_address(KAKAROT_BLOCK_GAS_LIMIT, &[]);
        self.state_mut().set_storage_at(
            kakarot_address,
            block_gas_limit_address,
            StarkFelt::from(block_gas_limit),
        )?;

        Ok(())
    }

    /// Sets up an EOA or contract account. Writes nonce, code and storage to the sequencer storage.
    fn setup_account(&mut self, account: KakarotAccount) -> StateResult<()> {
        let evm_address = &account.evm_address().bytes()[12..];
        let evm_address = Address::from_slice(evm_address);
        let mut storage = account.storage;
        let starknet_address = self.compute_starknet_address(&evm_address)?;

        // Pick the class hash based on the account type.
        //TODO: remove account distinction
        if matches!(account.account_type, AccountType::EOA) {
            self.state_mut().set_nonce(starknet_address, account.nonce);

        storage.append(&mut vec![
            starknet_storage!(ACCOUNT_IMPLEMENTATION, self.environment.eoa_class_hash.0), // both EOA and CA CH are the same (for now)
            starknet_storage!(OWNABLE_OWNER, *self.environment.kakarot_address.0.key()),
        ]);

        // Write all the storage vars to the sequencer state.
        for (k, v) in storage {
            self.state_mut().set_storage_at(starknet_address, k, v)?;
        }

        let class_hash = self.environment.contract_account_class_hash;
        // Set up the contract class hash
        self.state_mut()
            .set_class_hash_at(starknet_address, class_hash)?;

        // Add the address to the Kakarot evm to starknet mapping
        let kakarot_address = self.environment.kakarot_address;
        self.state_mut().set_storage_at(
            kakarot_address,
            get_storage_var_address(KAKAROT_EVM_TO_STARKNET_ADDRESS, &[account.evm_address]),
            *starknet_address.0.key(),
        )?;

        Ok(())
    }

    /// Funds an EOA or contract account. Also gives allowance to the Kakarot contract.
    fn fund(&mut self, evm_address: &Address, balance: U256) -> StateResult<()> {
        let starknet_address = self.compute_starknet_address(evm_address)?;
        let balance_values = split_u256(balance);
        let mut storage = vec![];

        // Initialize the balance storage var.
        let balance_key_low = get_fee_token_var_address(starknet_address);
        let balance_key_high = next_storage_key(&balance_key_low)?;
        storage.append(&mut vec![
            (balance_key_low, StarkFelt::from(balance_values[0])),
            (balance_key_high, StarkFelt::from(balance_values[1])),
        ]);

        // Initialize the allowance storage var.
        let allowance_key_low = get_storage_var_address(
            "ERC20_allowances",
            &[*starknet_address.0.key(), *KAKAROT_ADDRESS.0.key()],
        );
        let allowance_key_high = next_storage_key(&allowance_key_low)?;
        storage.append(&mut vec![
            (allowance_key_low, StarkFelt::from(u128::MAX)),
            (allowance_key_high, StarkFelt::from(u128::MAX)),
        ]);

        // Write all the storage vars to the sequencer state.
        for (k, v) in storage {
            self.state_mut()
                .set_storage_at(*ETH_FEE_TOKEN_ADDRESS, k, v)?;
        }
        Ok(())
    }

    /// Returns the storage value at the given key evm storage key.
    fn storage_at(&mut self, evm_address: &Address, key: U256) -> StateResult<U256> {
        let keys = split_u256(key).map(Into::into);
        let key_low = get_storage_var_address(ACCOUNT_STORAGE, &keys);
        let key_high = next_storage_key(&key_low)?;

        let starknet_address = self.compute_starknet_address(evm_address)?;

        let low = self.state_mut().get_storage_at(starknet_address, key_low)?;
        let high = self
            .state_mut()
            .get_storage_at(starknet_address, key_high)?;

        let low = U256::from_be_bytes(Into::<FieldElement>::into(low).to_bytes_be());
        let high = U256::from_be_bytes(Into::<FieldElement>::into(high).to_bytes_be());

        Ok(high << 128 | low)
    }

    /// Returns the nonce of the given address. For an EOA, uses the protocol level nonce.
    /// For a contract account, uses the Kakarot managed nonce stored in the contract account's storage.
    fn nonce_at(&mut self, evm_address: &Address) -> StateResult<U256> {
        let starknet_address = self.compute_starknet_address(evm_address)?;

        let implementation = self
            .state_mut()
            .get_storage_at(
                starknet_address,
                get_storage_var_address(ACCOUNT_IMPLEMENTATION, &[]),
            )
            .unwrap();

        Ok(U256::from_be_bytes(
            Into::<FieldElement>::into(nonce).to_bytes_be(),
        ))
    }

    /// Returns the bytecode of the given address. For an EOA, the bytecode_len_ storage variable will return 0,
    /// and the function will return an empty vector. For a contract account, the function will return the bytecode
    /// stored in the contract_account_bytecode storage variables. The function assumes that the bytecode is stored
    /// in 31 byte big-endian chunks.
    fn code_at(&mut self, evm_address: &Address) -> StateResult<Bytes> {
        // Get all storage addresses.
        let starknet_address = self.compute_starknet_address(evm_address)?;
        let bytecode_base_address = get_storage_var_address(ACCOUNT_BYTECODE, &[]);

        // Handle early return.
        let bytecode_len = self
            .state_mut()
            .get_storage_at(starknet_address, bytecode_base_address)?;
        let bytecode_len: u64 = bytecode_len.try_into()?;

        if bytecode_len == 0 {
            return Ok(Bytes::default());
        }

        // Bytecode is stored in chunks of 31 bytes. At bytecode_base_address,
        // we store the number of chunks.
        let (num_chunks, pending_word_len) = bytecode_len.div_rem(&31);
        let mut bytecode: Vec<u8> = Vec::with_capacity(bytecode_len as usize * 31);

        for chunk_index in 0..num_chunks {
            let index = chunk_index / 256;
            let offset = chunk_index % 256;
            let storage_pointer = inner_byte_array_pointer(
                (*bytecode_base_address.0.key()).into(),
                FieldElement::from(index),
            );
            let key = offset_storage_key(
                StorageKey(PatriciaKey::try_from(StarkFelt::from(storage_pointer)).unwrap()),
                offset as i64,
            );
            let code = self.state_mut().get_storage_at(starknet_address, key)?;
            bytecode.append(&mut FieldElement::from(code).to_bytes_be()[1..].to_vec());
        }

        if pending_word_len != 0 {
            let storage_chunk_index = num_chunks / 256;
            let offset_in_chunk = num_chunks % 256;
            let storage_pointer = inner_byte_array_pointer(
                (*bytecode_base_address.0.key()).into(),
                storage_chunk_index.into(),
            );
            let key = offset_storage_key(
                StorageKey(PatriciaKey::try_from(StarkFelt::from(storage_pointer)).unwrap()),
                offset_in_chunk as i64,
            );

            let pending_word = self.state_mut().get_storage_at(starknet_address, key)?;
            bytecode.append(
                &mut FieldElement::from(pending_word).to_bytes_be()
                    [32 - pending_word_len as usize..]
                    .to_vec(),
            );
        }

        Ok(Bytes::from(bytecode))
    }

    /// Returns the balance of native tokens at the given address.
    /// Makes use of the default StateReader implementation from Blockifier.
    fn balance_at(&mut self, evm_address: &Address) -> StateResult<U256> {
        let starknet_address = self.compute_starknet_address(evm_address)?;
        let (low, high) = self
            .state_mut()
            .get_fee_token_balance(starknet_address, *ETH_FEE_TOKEN_ADDRESS)?;

        let low = U256::from_be_bytes(Into::<FieldElement>::into(low).to_bytes_be());
        let high = U256::from_be_bytes(Into::<FieldElement>::into(high).to_bytes_be());

        Ok(high << 128 | low)
    }

    /// Converts the given signed transaction to a Starknet-rs transaction and executes it.
    // Since we are still missing the validate for the EOA, the signature is not added
    // to the transaction.
    fn execute_transaction(
        &mut self,
        transaction: TransactionSigned,
    ) -> TransactionExecutionResult<TransactionExecutionInfo> {
        let evm_address = transaction.recover_signer().ok_or_else(|| {
            TransactionExecutionError::ValidateTransactionError(
                EntryPointExecutionError::InvalidExecutionInput {
                    input_descriptor: String::from("Signed transaction"),
                    info: "Missing signer in signed transaction".to_string(),
                },
            )
        })?;
        let starknet_address = self.compute_starknet_address(&evm_address)?;

        let starknet_transaction =
            BroadcastedTransactionWrapper::new(BroadcastedTransaction::Invoke(
                to_broadcasted_starknet_transaction(
                    &transaction,
                    (*starknet_address.0.key()).into(),
                )
                .map_err(|err| {
                    TransactionExecutionError::ValidateTransactionError(
                        EntryPointExecutionError::InvalidExecutionInput {
                            input_descriptor: String::from("Failed to convert transaction"),
                            info: err.to_string(),
                        },
                    )
                })?,
            ));

        let chain_id = self.chain_id();
        self.execute(
            starknet_transaction
                .try_into_execution_transaction(FieldElement::from(chain_id))
                .unwrap(),
        )
    }
}

pub(crate) fn compute_storage_base_address(
    storage_var_name: &str,
    keys: &[StarkFelt],
) -> StorageKey {
    let selector = starknet_keccak(storage_var_name.as_bytes());
    let selector = felt_to_stark_felt(&selector);

    let data = [&[selector], keys].concat();
    let data = data.into_iter().map(FieldElement::from).collect::<Vec<_>>();

    let key: StarkFelt = poseidon_hash_many(&data).into();
    let key = stark_felt_to_felt(key);

    let key_floored = felt_to_stark_felt(&key.mod_floor(&Felt252::from_bytes_be(
        &L2_ADDRESS_UPPER_BOUND.to_bytes_be(),
    )));

    StorageKey(PatriciaKey::try_from(key_floored).unwrap()) // infallible
}

pub(crate) fn offset_storage_key(key: StorageKey, offset: i64) -> StorageKey {
    let base_address = stark_felt_to_felt(*key.0.key()) + Felt252::from(offset);
    let base_address = felt_to_stark_felt(&base_address);

    StorageKey(PatriciaKey::try_from(base_address).unwrap()) // infallible
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        evm_sequencer::{
            constants::{
                tests::{PRIVATE_KEY, PUBLIC_KEY, TEST_CONTRACT_ADDRESS},
                ACCOUNT_CONTRACT_CLASS_HASH, CAIRO1_HELPERS_CLASS_HASH, CHAIN_ID,
                UNINITIALIZED_ACCOUNT_CLASS_HASH,
            },
            sequencer::{KakarotEnvironment, INITIAL_SEQUENCER_STATE},
        },
        models::result::extract_output_and_log_execution_result,
    };
    use reth_primitives::{sign_message, Signature, TransactionSigned, TxLegacy, B256};
    use starknet::core::types::FieldElement;
    use starknet_api::hash::StarkFelt;

    #[test]
    fn test_offset_storage_base_address() {
        // Given
        let base_address = StorageKey(
            PatriciaKey::try_from(StarkFelt::from(FieldElement::from(0x0102030405060708u64)))
                .unwrap(),
        );
        let offset = -1;

        // When
        let result = offset_storage_key(base_address, offset);

        // Then
        let expected = StorageKey(
            PatriciaKey::try_from(StarkFelt::from(FieldElement::from(0x0102030405060707u64)))
                .unwrap(),
        );
        assert!(result == expected);
    }

    #[test]
    fn test_store_bytecode() {
        // Given
        let kakarot_environment = KakarotEnvironment::new(
            *KAKAROT_ADDRESS,
            *UNINITIALIZED_ACCOUNT_CLASS_HASH,
            *CAIRO1_HELPERS_CLASS_HASH,
            *ACCOUNT_CONTRACT_CLASS_HASH,
        );
        let coinbase_address = Address::left_padding_from(&1234u64.to_be_bytes());
        let mut sequencer = KakarotSequencer::new(
            INITIAL_SEQUENCER_STATE.clone(),
            kakarot_environment,
            coinbase_address,
            CHAIN_ID,
            0,
            0,
        );
        let bytecode = Bytes::from(vec![
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
        ]);

        // When
        let account =
            KakarotAccount::new(&TEST_CONTRACT_ADDRESS, &bytecode, U256::ZERO, &[], false).unwrap();
        sequencer.setup_account(account).unwrap();

        // Then
        let code = sequencer.code_at(&TEST_CONTRACT_ADDRESS).unwrap();
        assert_eq!(code, bytecode);
    }

    #[test]
    fn test_execute_simple_contract() {
        // Given
        let kakarot_environment = KakarotEnvironment::new(
            *KAKAROT_ADDRESS,
            *UNINITIALIZED_ACCOUNT_CLASS_HASH,
            *CAIRO1_HELPERS_CLASS_HASH,
            *ACCOUNT_CONTRACT_CLASS_HASH,
        );
        let coinbase_address = Address::left_padding_from(&0xC01BA5Eu64.to_be_bytes());
        let mut sequencer = KakarotSequencer::new(
            INITIAL_SEQUENCER_STATE.clone(),
            kakarot_environment,
            coinbase_address,
            CHAIN_ID,
            0,
            0,
        );

        let mut transaction = TransactionSigned {
            hash: B256::default(),
            signature: Signature::default(),
            transaction: reth_primitives::Transaction::Legacy(TxLegacy {
                chain_id: Some(CHAIN_ID),
                gas_limit: 1_000_000,
                to: reth_primitives::TransactionKind::Call(*TEST_CONTRACT_ADDRESS),
                ..Default::default()
            }),
        };
        let signature =
            sign_message(*PRIVATE_KEY, transaction.transaction.signature_hash()).unwrap();
        transaction.signature = signature;
        let eoa_nonce = U256::from(0);
        let contract_bytecode = Bytes::from(vec![96, 1, 96, 0, 85]); // PUSH 01 PUSH 00 SSTORE
        let contract_nonce = U256::from(1);

        // When
        let contract = KakarotAccount::new(
            &TEST_CONTRACT_ADDRESS,
            &contract_bytecode,
            contract_nonce,
            &[],
            false,
        )
        .unwrap();
        let eoa =
            KakarotAccount::new(&PUBLIC_KEY, &Bytes::default(), eoa_nonce, &[], true).unwrap();
        sequencer.setup_account(contract).unwrap();
        sequencer.setup_account(eoa).unwrap();
        let execution_result = sequencer.execute_transaction(transaction);

        // Update the output with the execution result of the current transaction
        let tx_output = extract_output_and_log_execution_result(
            &execution_result,
            "test_case",
            "test_category",
        )
        .unwrap_or_default();

        assert!(tx_output.success);

        // Then
        let storage = sequencer
            .storage_at(&TEST_CONTRACT_ADDRESS, U256::ZERO)
            .unwrap();

        assert_eq!(storage, U256::from(1_u64));
    }
}
