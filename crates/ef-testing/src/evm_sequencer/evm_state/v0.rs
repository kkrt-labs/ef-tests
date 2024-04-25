use blockifier::abi::abi_utils::{get_fee_token_var_address, get_storage_var_address};
use blockifier::abi::sierra_types::next_storage_key;
use blockifier::execution::errors::EntryPointExecutionError;
use blockifier::state::state_api::{State, StateReader, StateResult};
use blockifier::transaction::errors::TransactionExecutionError;
use blockifier::transaction::objects::{TransactionExecutionInfo, TransactionExecutionResult};
use reth_primitives::{Address, Bytes, TransactionSigned, U256};
use sequencer::execution::Execution as _;
use sequencer::transaction::BroadcastedTransactionWrapper;
use starknet::core::types::{BroadcastedTransaction, FieldElement};
use starknet_api::hash::StarkFelt;
use starknet_api::state::StorageKey;

use super::Evm;
use crate::evm_sequencer::account::{AccountType, KakarotAccount};
use crate::evm_sequencer::constants::storage_variables::{
    ACCOUNT_BYTECODE_LEN, ACCOUNT_CAIRO1_HELPERS_CLASS, ACCOUNT_IMPLEMENTATION, ACCOUNT_NONCE,
    ACCOUNT_STORAGE, KAKAROT_BASE_FEE, KAKAROT_BLOCK_GAS_LIMIT, KAKAROT_COINBASE,
    KAKAROT_EVM_TO_STARKNET_ADDRESS, KAKAROT_PREV_RANDAO, OWNABLE_OWNER,
};
use crate::evm_sequencer::constants::ETH_FEE_TOKEN_ADDRESS;
use crate::evm_sequencer::sequencer::KakarotSequencer;
use crate::evm_sequencer::types::felt::FeltSequencer;
use crate::evm_sequencer::utils::{felt_to_bytes, split_u256, to_broadcasted_starknet_transaction};
use crate::starknet_storage;

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
    /// Uses the KakarotSequencer environment to set the class hash, contract owner and Kakarot address.
    fn setup_account(&mut self, account: KakarotAccount) -> StateResult<()> {
        let evm_address = &account.evm_address().bytes()[12..];
        let evm_address = Address::from_slice(evm_address);
        let mut storage = account.storage;
        let starknet_address = self.compute_starknet_address(&evm_address)?;

        // Pick the class hash based on the account type.

        storage.append(&mut vec![
            starknet_storage!(ACCOUNT_IMPLEMENTATION, self.environment.eoa_class_hash.0), // both EOA and CA CH are the same (for now)
            starknet_storage!(
                ACCOUNT_CAIRO1_HELPERS_CLASS,
                self.environment.cairo1_helpers_class_hash.0
            ), // both EOA and CA CH are the same (for now)
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
        let balance_low_key = get_fee_token_var_address(starknet_address);
        let balance_high_key = next_storage_key(&balance_low_key)?;
        storage.append(&mut vec![
            (balance_low_key, StarkFelt::from(balance_values[0])),
            (balance_high_key, StarkFelt::from(balance_values[1])),
        ]);

        // Initialize the allowance storage var.
        let allowance_key_low = get_storage_var_address(
            "ERC20_allowances",
            &[
                *starknet_address.0.key(),
                *self.environment.kakarot_address.0.key(),
            ],
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

    /// Returns the nonce of the given address.
    fn nonce_at(&mut self, evm_address: &Address) -> StateResult<U256> {
        let starknet_address = self.compute_starknet_address(evm_address)?;

        let key = get_storage_var_address(ACCOUNT_NONCE, &[]);
        let nonce = self.state_mut().get_storage_at(starknet_address, key)?;

        Ok(U256::from_be_bytes(
            Into::<FieldElement>::into(nonce).to_bytes_be(),
        ))
    }

    /// Returns the bytecode of the given address. For an EOA, the bytecode_len_ storage variable will return 0,
    /// and the function will return an empty vector. For a contract account, the function will return the bytecode
    /// stored in the bytecode_ storage variables. The function assumes that the bytecode is stored in 31 byte big-endian chunks.
    fn code_at(&mut self, evm_address: &Address) -> StateResult<Bytes> {
        let starknet_address = self.compute_starknet_address(evm_address)?;

        let bytecode_len = self.state_mut().get_storage_at(
            starknet_address,
            get_storage_var_address(ACCOUNT_BYTECODE_LEN, &[]),
        )?;
        let bytecode_len: u64 = bytecode_len.try_into()?;
        if bytecode_len == 0 {
            return Ok(Bytes::default());
        }

        // Assumes that the bytecode is stored in 31 byte chunks.
        let num_chunks = bytecode_len / 31;
        let mut bytecode: Vec<u8> = Vec::with_capacity(bytecode_len as usize);

        for chunk_index in 0..num_chunks {
            let key = StorageKey::from(chunk_index);
            let code = self.state_mut().get_storage_at(starknet_address, key)?;
            bytecode.append(&mut felt_to_bytes(&code.into(), 1).to_vec());
        }

        let remainder = bytecode_len % 31;
        let key = StorageKey::from(num_chunks);
        let code = self.state_mut().get_storage_at(starknet_address, key)?;
        bytecode.append(&mut felt_to_bytes(&code.into(), (32 - remainder) as usize).to_vec());

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
                            input_descriptor: String::from("Signed transaction"),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        evm_sequencer::{
            constants::{
                tests::{PRIVATE_KEY, PUBLIC_KEY, TEST_CONTRACT_ADDRESS},
                ACCOUNT_CONTRACT_CLASS_HASH, CAIRO1_HELPERS_CLASS_HASH, CHAIN_ID, KAKAROT_ADDRESS,
                UNINITIALIZED_ACCOUNT_CLASS_HASH,
            },
            sequencer::{v0::INITIAL_SEQUENCER_STATE, KakarotEnvironment},
            utils::compute_starknet_address,
        },
        models::result::extract_output_and_log_execution_result,
    };
    use blockifier::{abi::abi_utils::get_storage_var_address, state::state_api::StateReader};
    use reth_primitives::{
        sign_message, AccessList, Signature, TransactionSigned, TxEip1559, B256,
    };
    use starknet_api::hash::StarkFelt;

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
            transaction: reth_primitives::Transaction::Eip1559(TxEip1559 {
                chain_id: CHAIN_ID,
                nonce: 0,
                gas_limit: 1_000_000,
                max_fee_per_gas: 0,
                max_priority_fee_per_gas: 0,
                to: reth_primitives::TransactionKind::Call(*TEST_CONTRACT_ADDRESS),
                value: U256::ZERO,
                access_list: AccessList::default(),
                input: Bytes::default(),
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
        let evm_address: FeltSequencer = (*TEST_CONTRACT_ADDRESS).try_into().unwrap(); // infallible
        let kakarot_address: FieldElement = (*KAKAROT_ADDRESS.0.key()).into();
        let contract_starknet_address = compute_starknet_address(
            &TEST_CONTRACT_ADDRESS,
            UNINITIALIZED_ACCOUNT_CLASS_HASH.0.into(),
            &[kakarot_address, evm_address.into()],
        )
        .try_into()
        .unwrap();

        let storage = sequencer
            .state_mut()
            .get_storage_at(
                contract_starknet_address,
                get_storage_var_address(
                    ACCOUNT_STORAGE,
                    &[StarkFelt::from(0u8), StarkFelt::from(0u8)],
                ),
            )
            .unwrap();
        assert_eq!(storage, StarkFelt::from(1u8));
    }
}
