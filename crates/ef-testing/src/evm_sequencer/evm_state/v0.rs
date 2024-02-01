use blockifier::abi::abi_utils::{get_fee_token_var_address, get_storage_var_address};
use blockifier::abi::sierra_types::next_storage_key;
use blockifier::execution::errors::EntryPointExecutionError;
use blockifier::state::state_api::{State, StateReader, StateResult};
use blockifier::transaction::errors::TransactionExecutionError;
use blockifier::transaction::objects::{TransactionExecutionInfo, TransactionExecutionResult};
use reth_primitives::{Address, Bytes, TransactionSigned};
use revm_primitives::U256;
use sequencer::execution::Execution as _;
use sequencer::transaction::BroadcastedTransactionWrapper;
use starknet::core::types::{BroadcastedTransaction, FieldElement};
use starknet_api::hash::StarkFelt;
use starknet_api::state::StorageKey;

use super::Evm;
use crate::evm_sequencer::account::{AccountType, KakarotAccount};
use crate::evm_sequencer::constants::kkrt_constants_v0::{
    CONTRACT_ACCOUNT_CLASS_HASH, EOA_CLASS_HASH, PROXY_CLASS_HASH,
};
use crate::evm_sequencer::constants::KAKAROT_ADDRESS;
use crate::evm_sequencer::constants::{CHAIN_ID, ETH_FEE_TOKEN_ADDRESS};
use crate::evm_sequencer::sequencer::KakarotSequencer;
use crate::evm_sequencer::utils::{
    compute_starknet_address, felt_to_bytes, split_u256, to_broadcasted_starknet_transaction,
};

impl Evm for KakarotSequencer {
    /// Sets up the evm state (coinbase, block number, etc.)
    fn setup_state(&mut self) -> StateResult<()> {
        Ok(())
    }

    /// Sets up an EOA or contract account. Writes nonce, code and storage to the sequencer storage.
    fn setup_account(&mut self, account: KakarotAccount) -> StateResult<()> {
        if matches!(account.account_type, AccountType::EOA) {
            self.state
                .set_nonce(account.starknet_address, account.nonce);
        }

        for (k, v) in account.storage {
            (&mut self.state).set_storage_at(account.starknet_address, k, v);
        }

        // Set up the contract class hash.
        (&mut self.state).set_class_hash_at(account.starknet_address, *PROXY_CLASS_HASH)?;

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
        let balance_low_key = get_fee_token_var_address(&starknet_address.try_into()?);
        let balance_high_key = next_storage_key(&balance_low_key)?;
        storage.append(&mut vec![
            (balance_low_key, StarkFelt::from(balance_values[0])),
            (balance_high_key, StarkFelt::from(balance_values[1])),
        ]);

        // Initialize the allowance storage var.
        let allowance_key_low = get_storage_var_address(
            "ERC20_allowances",
            &[starknet_address.into(), *KAKAROT_ADDRESS.0.key()],
        );
        let allowance_key_high = next_storage_key(&allowance_key_low)?;
        storage.append(&mut vec![
            (allowance_key_low, StarkFelt::from(u128::MAX)),
            (allowance_key_high, StarkFelt::from(u128::MAX)),
        ]);

        // Write all the storage vars to the sequencer state.
        for (k, v) in storage {
            (&mut self.state).set_storage_at(*ETH_FEE_TOKEN_ADDRESS, k, v);
        }
        Ok(())
    }

    /// Returns the storage value at the given key evm storage key.
    fn storage_at(&mut self, evm_address: &Address, key: U256) -> StateResult<U256> {
        let keys = split_u256(key).map(Into::into);
        let key_low = get_storage_var_address("storage_", &keys);
        let key_high = next_storage_key(&key_low)?;

        let starknet_address = compute_starknet_address(evm_address);

        let low = (&mut self.state).get_storage_at(starknet_address.try_into()?, key_low)?;
        let high = (&mut self.state).get_storage_at(starknet_address.try_into()?, key_high)?;

        let low = U256::from_be_bytes(Into::<FieldElement>::into(low).to_bytes_be());
        let high = U256::from_be_bytes(Into::<FieldElement>::into(high).to_bytes_be());

        Ok(high << 128 | low)
    }

    /// Returns the nonce of the given address. For an EOA, uses the protocol level nonce.
    /// For a contract account, uses the Kakarot managed nonce stored in the contract account's storage.
    fn nonce_at(&mut self, evm_address: &Address) -> StateResult<U256> {
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
    /// stored in the bytecode_ storage variables. The function assumes that the bytecode is stored in 31 byte big-endian chunks.
    fn code_at(&mut self, evm_address: &Address) -> StateResult<Bytes> {
        let starknet_address = compute_starknet_address(evm_address);

        let bytecode_len = (&mut self.state).get_storage_at(
            starknet_address.try_into()?,
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
            let code = (&mut self.state).get_storage_at(starknet_address.try_into()?, key)?;
            bytecode.append(&mut felt_to_bytes(&code.into(), 1).to_vec());
        }

        let remainder = bytecode_len % 31;
        let key = StorageKey::from(num_chunks);
        let code = (&mut self.state).get_storage_at(starknet_address.try_into()?, key)?;
        bytecode.append(&mut felt_to_bytes(&code.into(), (32 - remainder) as usize).to_vec());

        Ok(Bytes::from(bytecode))
    }

    /// Returns the balance of native tokens at the given address.
    /// Makes use of the default StateReader implementation from Blockifier.
    fn balance_at(&mut self, evm_address: &Address) -> StateResult<U256> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evm_sequencer::constants::{
        tests::{PRIVATE_KEY, PUBLIC_KEY, TEST_CONTRACT_ADDRESS},
        CHAIN_ID,
    };
    use blockifier::{abi::abi_utils::get_storage_var_address, state::state_api::StateReader};
    use reth_primitives::{sign_message, AccessList, Signature, TransactionSigned, TxEip1559};
    use revm_primitives::B256;
    use starknet_api::hash::StarkFelt;

    #[test]
    fn test_execute_simple_contract() {
        // Given
        let mut sequencer =
            crate::evm_sequencer::sequencer::KakarotSequencer::new(Address::from(1234u64), 0, 0);

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
        let bytecode = Bytes::from(vec![96, 1, 96, 0, 85]); // PUSH 01 PUSH 00 SSTORE
        let nonce = U256::from(0);

        // When
        let contract = KakarotAccount::new(&TEST_CONTRACT_ADDRESS, &bytecode, nonce, &[]).unwrap();
        let eoa = KakarotAccount::new(&PUBLIC_KEY, &Bytes::default(), nonce, &[]).unwrap();
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
