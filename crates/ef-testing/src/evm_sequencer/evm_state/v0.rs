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
        block_gaslimit: U256,
    ) -> StateResult<()> {
        let kakarot_address = self.environment.kakarot_address;
        let coinbase_address: FeltSequencer = (*self.address()).try_into().unwrap(); // infallible

        // Set the coinbase address.
        self.state_mut().set_storage_at(
            kakarot_address,
            get_storage_var_address("coinbase", &[]),
            coinbase_address.into(),
        );

        // Set the base fee.
        let low_fee = base_fee & U256::from(u128::MAX);
        let low_fee: u128 = low_fee.try_into().unwrap(); // safe unwrap <= U128::MAX.
        let high_fee = base_fee >> U256::from(128);
        let high_fee: u128 = high_fee.try_into().unwrap(); // safe unwrap <= U128::MAX.

        let basefee_address = get_storage_var_address("base_fee", &[]);
        self.state_mut()
            .set_storage_at(kakarot_address, basefee_address, StarkFelt::from(low_fee));
        self.state_mut().set_storage_at(
            kakarot_address,
            next_storage_key(&basefee_address)?,
            StarkFelt::from(high_fee),
        );

        // Set the previous randao.
        let prev_randao_low = prev_randao & U256::from(u128::MAX);
        let prev_randao_low: u128 = prev_randao_low.try_into().unwrap(); // safe unwrap <= U128::MAX.
        let prev_randao_high = prev_randao >> U256::from(128);
        let prev_randao_high: u128 = prev_randao_high.try_into().unwrap(); // safe unwrap <= U128::MAX.

        let prev_randao_address = get_storage_var_address("prev_randao", &[]);
        self.state_mut().set_storage_at(
            kakarot_address,
            prev_randao_address,
            StarkFelt::from(prev_randao_low),
        );
        self.state_mut().set_storage_at(
            kakarot_address,
            next_storage_key(&prev_randao_address)?,
            StarkFelt::from(prev_randao_high),
        );

        // Set the block gas limit, using the 128 lower bits.
        let block_gaslimit_low = block_gaslimit & U256::from(u128::MAX);
        let block_gaslimit_low: u128 = block_gaslimit_low.try_into().unwrap(); // safe unwrap <= U128::MAX.
        let block_gaslimit_address = get_storage_var_address("block_gaslimit", &[]);
        self.state_mut().set_storage_at(
            kakarot_address,
            block_gaslimit_address,
            StarkFelt::from(block_gaslimit_low),
        );

        Ok(())
    }

    /// Sets up an EOA or contract account. Writes nonce, code and storage to the sequencer storage.
    /// Uses the KakarotSequencer environment to set the class hash, contract owner and Kakarot address.
    fn setup_account(&mut self, account: KakarotAccount) -> StateResult<()> {
        let evm_address = &account.evm_address().bytes()[12..];
        let evm_address = Address::from_slice(evm_address);
        let mut storage = account.storage;
        let starknet_address = self.compute_starknet_address(&evm_address)?;

        // Set up the account implementation.
        match account.account_type {
            AccountType::EOA => {
                storage.push(starknet_storage!(
                    "_implementation",
                    self.environment.eoa_class_hash.0
                ));
                self.state_mut().set_nonce(starknet_address, account.nonce);
            }
            AccountType::Contract => {
                storage.append(&mut vec![
                    starknet_storage!(
                        "_implementation",
                        self.environment.contract_account_class_hash.0
                    ),
                    starknet_storage!("Ownable_owner", *self.environment.kakarot_address.0.key()),
                ]);
            }
            _ => {}
        }

        // Set the Kakarot address.
        storage.push(starknet_storage!(
            "kakarot_address",
            *self.environment.kakarot_address.0.key()
        ));

        // Write all the storage vars to the sequencer state.
        for (k, v) in storage {
            self.state_mut().set_storage_at(starknet_address, k, v);
        }

        // Set up the contract class hash.
        let proxy_class_hash = self.environment.base_account_class_hash;
        self.state_mut()
            .set_class_hash_at(starknet_address, proxy_class_hash)?;

        // Add the address to the Kakarot evm to starknet mapping
        let kakarot_address = self.environment.kakarot_address;
        self.state_mut().set_storage_at(
            kakarot_address,
            get_storage_var_address("evm_to_starknet_address", &[account.evm_address]),
            *starknet_address.0.key(),
        );
        Ok(())
    }

    /// Funds an EOA or contract account. Also gives allowance to the Kakarot contract.
    fn fund(&mut self, evm_address: &Address, balance: U256) -> StateResult<()> {
        let starknet_address = self.compute_starknet_address(evm_address)?;
        let balance_values = split_u256(balance);
        let mut storage = vec![];

        // Initialize the balance storage var.
        let balance_low_key = get_fee_token_var_address(&starknet_address);
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
                .set_storage_at(*ETH_FEE_TOKEN_ADDRESS, k, v);
        }
        Ok(())
    }

    /// Returns the storage value at the given key evm storage key.
    fn storage_at(&mut self, evm_address: &Address, key: U256) -> StateResult<U256> {
        let keys = split_u256(key).map(Into::into);
        let key_low = get_storage_var_address("storage_", &keys);
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
                get_storage_var_address("_implementation", &[]),
            )
            .unwrap();

        let nonce = if implementation == self.environment.eoa_class_hash.0 {
            self.state_mut().get_nonce_at(starknet_address)?.0
        } else if implementation == self.environment.contract_account_class_hash.0 {
            let key = get_storage_var_address("nonce", &[]);
            self.state_mut().get_storage_at(starknet_address, key)?
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
    /// stored in the bytecode_ storage variables. The function assumes that the bytecode is stored in 31 byte big-endian chunks.
    fn code_at(&mut self, evm_address: &Address) -> StateResult<Bytes> {
        let starknet_address = self.compute_starknet_address(evm_address)?;

        let bytecode_len = self.state_mut().get_storage_at(
            starknet_address,
            get_storage_var_address("bytecode_len_", &[]),
        )?;
        let bytecode_len: u64 = bytecode_len.try_into()?;
        if bytecode_len == 0 {
            return Ok(Bytes::default());
        }

        // Assumes that the bytecode is stored in 31 byte chunks.
        let num_chunks = bytecode_len / 31;
        let mut bytecode: Vec<u8> = Vec::new();

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
            .get_fee_token_balance(&starknet_address, &ETH_FEE_TOKEN_ADDRESS)?;

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
    use crate::evm_sequencer::{
        constants::{
            tests::{PRIVATE_KEY, PUBLIC_KEY, TEST_CONTRACT_ADDRESS},
            CHAIN_ID, CONTRACT_ACCOUNT_CLASS_HASH, EOA_CLASS_HASH, KAKAROT_ADDRESS,
            PROXY_CLASS_HASH,
        },
        sequencer::{v0::INITIAL_SEQUENCER_STATE, KakarotEnvironment},
        utils::compute_starknet_address,
    };
    use blockifier::{abi::abi_utils::get_storage_var_address, state::state_api::StateReader};
    use reth_primitives::{
        sign_message, AccessList, Signature, TransactionSigned, TxEip1559, TxValue, B256,
    };
    use ruint::aliases::U160;
    use starknet_api::hash::StarkFelt;

    #[test]
    fn test_execute_simple_contract() {
        // Given
        let kakarot_environment = KakarotEnvironment::new(
            *KAKAROT_ADDRESS,
            *PROXY_CLASS_HASH,
            *EOA_CLASS_HASH,
            *CONTRACT_ACCOUNT_CLASS_HASH,
        );
        let coinbase_address = Address::from(U160::from(1234u64));
        let mut sequencer = KakarotSequencer::new(
            INITIAL_SEQUENCER_STATE.clone(),
            kakarot_environment,
            coinbase_address,
            *CHAIN_ID,
            0,
            0,
        );

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
                value: TxValue::from(U256::ZERO),
                access_list: AccessList::default(),
                input: Bytes::default(),
            }),
        };
        let signature =
            sign_message(*PRIVATE_KEY, transaction.transaction.signature_hash()).unwrap();
        transaction.signature = signature;
        let bytecode = Bytes::from(vec![96, 1, 96, 0, 85]); // PUSH 01 PUSH 00 SSTORE
        let nonce = U256::from(0);

        // When
        let contract =
            KakarotAccount::new(&TEST_CONTRACT_ADDRESS, &bytecode, nonce, &[], false).unwrap();
        let eoa = KakarotAccount::new(&PUBLIC_KEY, &Bytes::default(), nonce, &[], true).unwrap();
        sequencer.setup_account(contract).unwrap();
        sequencer.setup_account(eoa).unwrap();
        sequencer.execute_transaction(transaction).unwrap();

        // Then
        let contract_starknet_address = compute_starknet_address(
            &TEST_CONTRACT_ADDRESS,
            (*KAKAROT_ADDRESS.0.key()).into(),
            PROXY_CLASS_HASH.0.into(),
            &[],
        )
        .try_into()
        .unwrap();
        let storage = sequencer
            .state_mut()
            .get_storage_at(
                contract_starknet_address,
                get_storage_var_address("storage_", &[StarkFelt::from(0u8), StarkFelt::from(0u8)]),
            )
            .unwrap();
        assert_eq!(storage, StarkFelt::from(1u8));
    }
}
