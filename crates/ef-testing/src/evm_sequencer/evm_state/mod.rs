use crate::evm_sequencer::constants::storage_variables::ACCOUNT_BYTECODE_LEN;
use crate::evm_sequencer::constants::RELAYER_ADDRESS;
use crate::evm_sequencer::utils::felt_to_bytes;
use crate::{
    evm_sequencer::{
        account::KakarotAccount,
        constants::{
            storage_variables::{
                ACCOUNT_IMPLEMENTATION, ACCOUNT_NONCE, ACCOUNT_STORAGE, KAKAROT_BASE_FEE,
                KAKAROT_BLOCK_GAS_LIMIT, KAKAROT_COINBASE, KAKAROT_EVM_TO_STARKNET_ADDRESS,
                KAKAROT_PREV_RANDAO, OWNABLE_OWNER,
            },
            ETH_FEE_TOKEN_ADDRESS, KAKAROT_ADDRESS,
        },
        sequencer::KakarotSequencer,
        utils::{split_u256, to_broadcasted_starknet_transaction},
    },
    starknet_storage,
};
use alloy_primitives::Address;
use alloy_primitives::Bytes;
use alloy_primitives::U256;
use blockifier::{
    abi::{
        abi_utils::{get_fee_token_var_address, get_storage_var_address},
        sierra_types::next_storage_key,
    },
    execution::errors::EntryPointExecutionError,
    state::state_api::{State as _, StateReader as _, StateResult},
    transaction::{
        errors::TransactionExecutionError,
        objects::{TransactionExecutionInfo, TransactionExecutionResult},
    },
};
use reth_primitives::TransactionSigned;
use sequencer::{execution::Execution as _, transaction::BroadcastedTransactionWrapper};
use starknet::core::types::BroadcastedTransaction;
use starknet_api::state::StorageKey;
use starknet_crypto::Felt;

/// EVM state interface. Used to setup the evm state, EOA and contract accounts,
/// fund them and get their state (balance, nonce, code, storage).
/// Default implementation is used when no feature flag is enabled.
pub trait Evm {
    // TODO enforce using a marker type that you can only proceed
    // with execution if the state is initialized.
    fn setup_state(
        &mut self,
        _base_fee: U256,
        _prev_randao: U256,
        _block_gas_limit: U256,
    ) -> StateResult<()> {
        panic!("Not implemented, use features flag \"v0\" or \"v1\"")
    }

    fn setup_account(&mut self, _account: KakarotAccount) -> StateResult<()> {
        panic!("Not implemented, use features flag \"v0\" or \"v1\"")
    }

    fn fund(&mut self, _evm_address: &Address, _balance: U256) -> StateResult<()> {
        panic!("Not implemented, use features flag \"v0\" or \"v1\"")
    }

    fn storage_at(&mut self, _evm_address: &Address, _key: U256) -> StateResult<U256> {
        panic!("Not implemented, use features flag \"v0\" or \"v1\"")
    }

    fn nonce_at(&mut self, _evm_address: &Address) -> StateResult<U256> {
        panic!("Not implemented, use features flag \"v0\" or \"v1\"")
    }

    fn code_at(&mut self, _evm_address: &Address) -> StateResult<Bytes> {
        panic!("Not implemented, use features flag \"v0\" or \"v1\"")
    }

    fn balance_at(&mut self, _evm_address: &Address) -> StateResult<U256> {
        panic!("Not implemented, use features flag \"v0\" or \"v1\"")
    }

    fn execute_transaction(
        &mut self,
        _transaction: TransactionSigned,
    ) -> TransactionExecutionResult<TransactionExecutionInfo> {
        panic!("Not implemented, use features flag \"v0\" or \"v1\"")
    }
}

impl Evm for KakarotSequencer {
    /// Sets up the evm state (coinbase, block number, etc.)
    fn setup_state(
        &mut self,
        base_fee: U256,
        prev_randao: U256,
        block_gas_limit: U256,
    ) -> StateResult<()> {
        let kakarot_address = self.environment.kakarot_address;
        let coinbase_address = Felt::from_bytes_be_slice(&self.address().0[..]);

        // Set the coinbase address.
        self.state_mut().set_storage_at(
            kakarot_address,
            get_storage_var_address(KAKAROT_COINBASE, &[]),
            coinbase_address,
        )?;

        // Set the base fee at index 'current_block'
        let [low_fee, high_fee] = split_u256(base_fee);
        let key = Felt::from_bytes_be_slice(&"current_block".as_bytes());
        println!("key: {:?}", key);
        let basefee_address = get_storage_var_address(KAKAROT_BASE_FEE, &[key]);
        self.state_mut()
            .set_storage_at(kakarot_address, basefee_address, low_fee.into())?;
        self.state_mut().set_storage_at(
            kakarot_address,
            next_storage_key(&basefee_address)?,
            high_fee.into(),
        )?;

        // Set the previous randao.
        let [low_prev_randao, high_prev_randao] = split_u256(prev_randao);
        let prev_randao_address = get_storage_var_address(KAKAROT_PREV_RANDAO, &[]);
        self.state_mut().set_storage_at(
            kakarot_address,
            prev_randao_address,
            low_prev_randao.into(),
        )?;
        self.state_mut().set_storage_at(
            kakarot_address,
            next_storage_key(&prev_randao_address)?,
            high_prev_randao.into(),
        )?;

        // Set the block gas limit, considering it fits in a felt.
        let [block_gas_limit, _] = split_u256(block_gas_limit);
        let block_gas_limit_address = get_storage_var_address(KAKAROT_BLOCK_GAS_LIMIT, &[]);
        self.state_mut().set_storage_at(
            kakarot_address,
            block_gas_limit_address,
            block_gas_limit.into(),
        )?;

        Ok(())
    }

    /// Sets up an EOA or contract account. Writes nonce, code and storage to the sequencer storage.
    fn setup_account(&mut self, account: KakarotAccount) -> StateResult<()> {
        let evm_address = &account.evm_address().to_bytes_be()[12..];
        let evm_address = Address::from_slice(evm_address);
        let mut storage = account.storage;
        let starknet_address = self.compute_starknet_address(&evm_address)?;

        self.state_mut().set_nonce(starknet_address, account.nonce);

        storage.append(&mut vec![
            starknet_storage!(
                ACCOUNT_IMPLEMENTATION,
                self.environment.account_contract_class_hash.0
            ),
            starknet_storage!(OWNABLE_OWNER, *self.environment.kakarot_address.0.key()),
        ]);

        // Write all the storage vars to the sequencer state.
        for (k, v) in storage {
            self.state_mut().set_storage_at(starknet_address, k, v)?;
        }

        let class_hash = self.environment.account_contract_class_hash;
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
            (balance_key_low, balance_values[0].into()),
            (balance_key_high, balance_values[1].into()),
        ]);

        // Initialize the allowance storage var.
        let allowance_key_low = get_storage_var_address(
            "ERC20_allowances",
            &[*starknet_address.0.key(), *KAKAROT_ADDRESS.0.key()],
        );
        let allowance_key_high = next_storage_key(&allowance_key_low)?;
        storage.append(&mut vec![
            (allowance_key_low, u128::MAX.into()),
            (allowance_key_high, u128::MAX.into()),
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

        let low = U256::from_be_bytes(low.to_bytes_be());
        let high = U256::from_be_bytes(high.to_bytes_be());

        Ok(high << 128 | low)
    }

    /// Returns the nonce of the given address.
    /// Uses the Kakarot managed nonce stored in the contract account's storage.
    fn nonce_at(&mut self, evm_address: &Address) -> StateResult<U256> {
        let starknet_address = self.compute_starknet_address(evm_address)?;

        let key = get_storage_var_address(ACCOUNT_NONCE, &[]);
        let nonce = self.state_mut().get_storage_at(starknet_address, key)?;

        Ok(U256::from_be_bytes(nonce.to_bytes_be()))
    }

    /// Returns the bytecode of the given address. For an EOA, the bytecode_len_ storage variable will return 0,
    /// and the function will return an empty vector. For a contract account, the function will return the bytecode
    /// stored in the contract_account_bytecode storage variables. The function assumes that the bytecode is stored
    /// in 31 byte big-endian chunks.
    fn code_at(&mut self, evm_address: &Address) -> StateResult<Bytes> {
        // Get all storage addresses.
        let starknet_address = self.compute_starknet_address(evm_address)?;

        let bytecode_len = self.state_mut().get_storage_at(
            starknet_address,
            get_storage_var_address(ACCOUNT_BYTECODE_LEN, &[]),
        )?;
        let bytecode_len: u64 = bytecode_len.to_biguint().try_into()?;

        if bytecode_len == 0 {
            return Ok(Bytes::default());
        }

        // Assumes that the bytecode is stored in 31 byte chunks.
        let num_chunks = bytecode_len / 31;
        let mut bytecode: Vec<u8> = Vec::with_capacity(bytecode_len as usize);

        for chunk_index in 0..num_chunks {
            let key = StorageKey::from(chunk_index);
            let code = self.state_mut().get_storage_at(starknet_address, key)?;
            bytecode.append(&mut felt_to_bytes(&code, 1).to_vec());
        }

        let remainder = bytecode_len % 31;
        let key = StorageKey::from(num_chunks);
        let code = self.state_mut().get_storage_at(starknet_address, key)?;
        bytecode.append(&mut felt_to_bytes(&code, (32 - remainder) as usize).to_vec());

        Ok(Bytes::from(bytecode))
    }

    /// Returns the balance of native tokens at the given address.
    /// Makes use of the default StateReader implementation from Blockifier.
    fn balance_at(&mut self, evm_address: &Address) -> StateResult<U256> {
        let starknet_address = self.compute_starknet_address(evm_address)?;
        let (low, high) = self
            .state_mut()
            .get_fee_token_balance(starknet_address, *ETH_FEE_TOKEN_ADDRESS)?;

        let low = U256::from_be_bytes(low.to_bytes_be());
        let high = U256::from_be_bytes(high.to_bytes_be());

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
            TransactionExecutionError::ValidateTransactionError {
                error: EntryPointExecutionError::InvalidExecutionInput {
                    input_descriptor: String::from("Signed transaction"),
                    info: "Missing signer in signed transaction".to_string(),
                },
                class_hash: Default::default(),
                storage_address: Default::default(),
                selector: Default::default(),
            }
        })?;
        let starknet_address = self.compute_starknet_address(&evm_address)?;
        let relayer_nonce = self.state_mut().get_nonce_at(*RELAYER_ADDRESS).unwrap();

        let starknet_transaction =
            BroadcastedTransactionWrapper::new(BroadcastedTransaction::Invoke(
                to_broadcasted_starknet_transaction(
                    &transaction,
                    Felt::from(starknet_address),
                    relayer_nonce.0.into(),
                )
                .map_err(|err| {
                    TransactionExecutionError::ValidateTransactionError {
                        error: EntryPointExecutionError::InvalidExecutionInput {
                            input_descriptor: String::from("Failed to convert transaction"),
                            info: err.to_string(),
                        },
                        class_hash: Default::default(),
                        storage_address: Default::default(),
                        selector: Default::default(),
                    }
                })?,
            ));

        let chain_id = self.chain_id();
        self.execute(
            starknet_transaction
                .try_into_execution_transaction(Felt::from(chain_id))
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
            sequencer::{KakarotEnvironment, INITIAL_SEQUENCER_STATE},
        },
        models::result::extract_output_and_log_execution_result,
    };
    use alloy_consensus::TxEip1559;
    use alloy_eips::eip2930::AccessList;
    use alloy_primitives::B256;
    use ef_tests::models::Account;
    use reth_primitives::{sign_message, Signature, TransactionSigned};

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
            1,
        );

        let mut transaction = TransactionSigned {
            hash: B256::default(),
            signature: Signature::from_rs_and_parity(U256::ZERO, U256::ZERO, false).unwrap(),
            transaction: reth_primitives::Transaction::Eip1559(TxEip1559 {
                chain_id: CHAIN_ID,
                nonce: 0,
                gas_limit: 1_000_000,
                max_fee_per_gas: 0,
                max_priority_fee_per_gas: 0,
                to: alloy_primitives::TxKind::Call(TEST_CONTRACT_ADDRESS),
                value: U256::ZERO,
                access_list: AccessList::default(),
                input: Bytes::default(),
            }),
        };
        let signature =
            sign_message(PRIVATE_KEY, transaction.transaction.signature_hash()).unwrap();
        transaction.signature = signature;
        let eoa_nonce = U256::ZERO;
        let contract_bytecode = Bytes::from(vec![96, 1, 96, 0, 85]); // PUSH 01 PUSH 00 SSTORE
        let contract_nonce = U256::from(1);

        // When
        let contract = KakarotAccount::new(
            &TEST_CONTRACT_ADDRESS,
            Account {
                code: contract_bytecode,
                nonce: contract_nonce,
                ..Default::default()
            },
        )
        .unwrap();
        let eoa = KakarotAccount::new(
            &PUBLIC_KEY,
            Account {
                nonce: eoa_nonce,
                ..Default::default()
            },
        )
        .unwrap();
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
