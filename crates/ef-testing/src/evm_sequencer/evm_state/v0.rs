use cairo_vm::Felt252;
use num_traits::ToPrimitive;
use reth_primitives::{Address, Bytes, TransactionSigned};
use revm_primitives::U256;
use sequencer::execution::{Execution as _, TransactionExecutionResult};
use sequencer::state::StateResult;
use starknet_in_rust::core::errors::state_errors::StateError;
use starknet_in_rust::execution::TransactionExecutionInfo;
use starknet_in_rust::state::state_api::{State as _, StateReader};
use starknet_in_rust::transaction::error::TransactionError;
use starknet_in_rust::utils::{
    get_erc20_balance_var_addresses, get_storage_var_address, ClassHash,
};

use super::Evm;
use crate::evm_sequencer::account::{AccountType, KakarotAccount};
use crate::evm_sequencer::constants::kkrt_constants_v0::{
    CONTRACT_ACCOUNT_CLASS_HASH, EOA_CLASS_HASH, PROXY_CLASS_HASH,
};
use crate::evm_sequencer::constants::KAKAROT_ADDRESS;
use crate::evm_sequencer::constants::{BLOCK_CONTEXT, ETH_FEE_TOKEN_ADDRESS};
use crate::evm_sequencer::sequencer::KakarotSequencer;
use crate::evm_sequencer::utils::{
    compute_starknet_address, high_16_bytes_of_felt_to_bytes, split_u256, to_starknet_transaction,
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
                .set_nonce(&account.starknet_address, account.nonce);
        }

        for (k, v) in account.storage {
            self.state
                .set_storage_at(&(account.starknet_address.clone(), k.to_bytes_be()), v);
        }

        // Set up the contract class hash.
        self.state
            .set_class_hash_at(account.starknet_address.clone(), *PROXY_CLASS_HASH)?;

        // Add the address to the Kakarot evm to starknet mapping
        self.state.set_storage_at(
            &(
                KAKAROT_ADDRESS.clone(),
                get_storage_var_address("evm_to_starknet_address", &[account.evm_address])?
                    .to_bytes_be(),
            ),
            account.starknet_address.0,
        );
        Ok(())
    }

    /// Funds an EOA or contract account. Also gives allowance to the Kakarot contract.
    fn fund(&mut self, evm_address: &Address, balance: U256) -> StateResult<()> {
        let starknet_address = compute_starknet_address(evm_address);
        let balance_values = split_u256(balance);
        let mut storage = vec![];

        // Initialize the balance storage var.
        let (balance_low_key, balance_high_key) =
            get_erc20_balance_var_addresses(&starknet_address)?;
        storage.append(&mut vec![
            (
                Felt252::from_bytes_be(&balance_low_key),
                Felt252::from(balance_values[0]),
            ),
            (
                Felt252::from_bytes_be(&balance_high_key),
                Felt252::from(balance_values[1]),
            ),
        ]);

        // Initialize the allowance storage var.
        let allowance_key_low =
            get_storage_var_address("ERC20_allowances", &[starknet_address.0, KAKAROT_ADDRESS.0])?;
        let allowance_key_high = &allowance_key_low + 1u64;
        storage.append(&mut vec![
            (allowance_key_low, Felt252::from(u128::MAX)),
            (allowance_key_high, Felt252::from(u128::MAX)),
        ]);

        // Write all the storage vars to the sequencer state.
        for (k, v) in storage {
            self.state
                .set_storage_at(&(ETH_FEE_TOKEN_ADDRESS.clone(), k.to_bytes_be()), v);
        }
        Ok(())
    }

    /// Returns the storage value at the given key evm storage key.
    fn storage_at(&mut self, evm_address: &Address, key: U256) -> StateResult<U256> {
        let keys = split_u256(key).map(Into::into);
        let key_low = get_storage_var_address("storage_", &keys)?;
        let key_high = &key_low + 1u64;

        let starknet_address = compute_starknet_address(evm_address);

        let low = self
            .state
            .get_storage_at(&(starknet_address.clone(), key_low.to_bytes_be()))?;
        let high = self
            .state
            .get_storage_at(&(starknet_address, key_high.to_bytes_be()))?;

        let low = U256::from_be_bytes(low.to_bytes_be());
        let high = U256::from_be_bytes(high.to_bytes_be());

        Ok(high << 128 | low)
    }

    /// Returns the nonce of the given address. For an EOA, uses the protocol level nonce.
    /// For a contract account, uses the Kakarot managed nonce stored in the contract account's storage.
    fn nonce_at(&mut self, evm_address: &Address) -> StateResult<U256> {
        let starknet_address = compute_starknet_address(evm_address);

        let implementation: ClassHash = self
            .state
            .get_storage_at(&(
                starknet_address.clone(),
                get_storage_var_address("_implementation", &[])?.to_bytes_be(),
            ))
            .unwrap()
            .into();

        let nonce = if implementation == *EOA_CLASS_HASH {
            self.state.get_nonce_at(&starknet_address)?
        } else if implementation == *CONTRACT_ACCOUNT_CLASS_HASH {
            let key = get_storage_var_address("nonce", &[])?;
            self.state
                .get_storage_at(&(starknet_address, key.to_bytes_be()))?
        } else {
            // We can't throw an error here, because it could just be an uninitialized account.
            Felt252::from(0_u8)
        };

        Ok(U256::from_be_bytes(nonce.to_bytes_be()))
    }

    /// Returns the bytecode of the given address. For an EOA, the bytecode_len_ storage variable will return 0,
    /// and the function will return an empty vector. For a contract account, the function will return the bytecode
    /// stored in the bytecode_ storage variables. The function assumes that the bytecode is stored in 16 byte big-endian chunks.
    fn code_at(&mut self, evm_address: &Address) -> StateResult<Bytes> {
        let starknet_address = compute_starknet_address(evm_address);

        let bytecode_len = self.state.get_storage_at(&(
            starknet_address.clone(),
            get_storage_var_address("bytecode_len_", &[])?.to_bytes_be(),
        ))?;
        let bytecode_len = bytecode_len.to_u64().ok_or_else(|| {
            StateError::CustomError("Failed to convert Felt252 to u64".to_string())
        })?;
        if bytecode_len == 0 {
            return Ok(Bytes::default());
        }

        // Assumes that the bytecode is stored in 16 byte chunks.
        let num_chunks = bytecode_len / 16;
        let mut bytecode: Vec<u8> = Vec::new();

        for chunk_index in 0..num_chunks {
            let key = get_storage_var_address("bytecode_", &[Felt252::from(chunk_index)])?;
            let code = self
                .state
                .get_storage_at(&(starknet_address.clone(), key.to_bytes_be()))?;
            bytecode.append(&mut high_16_bytes_of_felt_to_bytes(&code, 16).to_vec());
        }

        let remainder = bytecode_len % 16;
        let key = get_storage_var_address("bytecode_", &[Felt252::from(num_chunks)])?;
        let code = self
            .state
            .get_storage_at(&(starknet_address, key.to_bytes_be()))?;
        bytecode.append(&mut high_16_bytes_of_felt_to_bytes(&code, remainder as usize).to_vec());

        Ok(Bytes::from(bytecode))
    }

    /// Returns the balance of native tokens at the given address.
    /// Makes use of the default StateReader implementation from Blockifier.
    fn balance_at(&mut self, evm_address: &Address) -> StateResult<U256> {
        let starknet_address = compute_starknet_address(evm_address);
        let (low, high) = self
            .state
            .get_fee_token_balance(&BLOCK_CONTEXT.clone(), &starknet_address)?;

        let low = U256::from_be_bytes(low.to_bytes_be());
        let high = U256::from_be_bytes(high.to_bytes_be());

        Ok(high << 128 | low)
    }

    /// Converts the given signed transaction to a Starknet-rs transaction and executes it.
    fn execute_transaction(
        &mut self,
        transaction: TransactionSigned,
    ) -> TransactionExecutionResult<TransactionExecutionInfo> {
        let starknet_transaction = to_starknet_transaction(&transaction)
            .map_err(|err| TransactionError::CustomError(err.to_string()))?;
        self.execute(&starknet_transaction, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evm_sequencer::constants::{
        tests::{PRIVATE_KEY, PUBLIC_KEY, TEST_CONTRACT_ADDRESS},
        CHAIN_ID,
    };
    use reth_primitives::{sign_message, AccessList, Signature, TransactionSigned, TxEip1559};
    use revm_primitives::B256;

    #[test]
    fn test_execute_simple_contract() {
        // Given
        let mut sequencer = crate::evm_sequencer::sequencer::KakarotSequencer::new();

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
        let contract_starknet_address = compute_starknet_address(&TEST_CONTRACT_ADDRESS);
        let storage = sequencer
            .state
            .get_storage_at(&(
                contract_starknet_address,
                get_storage_var_address("storage_", &[Felt252::from(0u8), Felt252::from(0u8)])
                    .unwrap()
                    .to_bytes_be(),
            ))
            .unwrap();
        assert_eq!(storage, Felt252::from(1u8));
    }
}
