use cairo_vm::felt::Felt252;
use num_integer::Integer;
use num_traits::ToPrimitive;
use reth_primitives::{Address, Bytes, TransactionSigned};
use revm_primitives::U256;
use sequencer::{
    execution::{Execution as _, TransactionExecutionResult},
    state::StateResult,
};
use starknet::core::utils::starknet_keccak;
use starknet_api::core::L2_ADDRESS_UPPER_BOUND;
use starknet_crypto::{poseidon_hash_many, FieldElement};
use starknet_in_rust::{
    core::errors::state_errors::StateError,
    execution::TransactionExecutionInfo,
    state::state_api::{State as _, StateReader},
    transaction::error::TransactionError,
    utils::{
        felt_to_field_element, field_element_to_felt, get_erc20_balance_var_addresses,
        get_storage_var_address,
    },
};

use super::Evm;
use crate::evm_sequencer::{
    account::{AccountType, KakarotAccount},
    constants::{
        kkrt_constants_v1::{CONTRACT_ACCOUNT_CLASS_HASH, EOA_CLASS_HASH},
        BLOCK_CONTEXT, COINBASE_ADDRESS, ETH_FEE_TOKEN_ADDRESS, KAKAROT_ADDRESS,
    },
    sequencer::KakarotSequencer,
    utils::{compute_starknet_address, split_u256, to_starknet_transaction},
};

impl Evm for KakarotSequencer {
    /// Sets up the evm state (coinbase, block number, etc.)
    fn setup_state(&mut self) -> StateResult<()> {
        let coinbase = KakarotAccount::new(&COINBASE_ADDRESS, &Bytes::default(), U256::ZERO, &[])?;
        self.setup_account(coinbase)?;

        Ok(())
    }

    /// Sets up an EOA or contract account. Writes nonce, code and storage to the sequencer storage.
    fn setup_account(&mut self, account: KakarotAccount) -> StateResult<()> {
        let class_hash = if matches!(account.account_type, AccountType::EOA) {
            self.state
                .set_nonce(&account.starknet_address, account.nonce.clone());
            *EOA_CLASS_HASH
        } else {
            *CONTRACT_ACCOUNT_CLASS_HASH
        };

        for (k, v) in account.storage {
            self.state
                .set_storage_at(&(account.starknet_address.clone(), k.to_be_bytes()), v);
        }

        // Set up the contract class hash
        self.state
            .set_class_hash_at(account.starknet_address.clone(), class_hash)?;

        // Add the address tot the Kakarot evm to starknet mapping
        let registry_base_address =
            get_storage_var_address("address_registry", &[account.evm_address])?;
        self.state.set_storage_at(
            &(KAKAROT_ADDRESS.clone(), registry_base_address.to_be_bytes()),
            Felt252::from(account.account_type as u8),
        );
        self.state.set_storage_at(
            &(
                KAKAROT_ADDRESS.clone(),
                (registry_base_address + 1u32).to_be_bytes(),
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
        let (balance_key_low, balance_key_high) =
            get_erc20_balance_var_addresses(&starknet_address)?;
        storage.append(&mut vec![
            (
                Felt252::from_bytes_be(&balance_key_low[..]),
                Felt252::from(balance_values[0]),
            ),
            (
                Felt252::from_bytes_be(&balance_key_high[..]),
                Felt252::from(balance_values[1]),
            ),
        ]);

        // Initialize the allowance storage var.
        let allowance_key_low = get_storage_var_address(
            "ERC20_allowances",
            &[starknet_address.0, KAKAROT_ADDRESS.0.clone()],
        )?;
        let allowance_key_high = &allowance_key_low + 1u64;
        storage.append(&mut vec![
            (allowance_key_low, Felt252::from(u128::MAX)),
            (allowance_key_high, Felt252::from(u128::MAX)),
        ]);

        // Write all the storage vars to the sequencer state.
        for (k, v) in storage {
            self.state
                .set_storage_at(&(ETH_FEE_TOKEN_ADDRESS.clone(), k.to_be_bytes()), v);
        }
        Ok(())
    }

    /// Returns the storage value at the given key evm storage key.
    fn storage_at(&mut self, evm_address: &Address, key: U256) -> StateResult<U256> {
        let keys = split_u256(key).map(Into::into);
        let low_key = poseidon_storage_base_address("contract_account_storage_keys", &keys);
        let high_key = &low_key + 1u64;

        let starknet_address = compute_starknet_address(evm_address);

        let low = self
            .state
            .get_storage_at(&(starknet_address.clone(), low_key.to_be_bytes()))?;
        let high = self
            .state
            .get_storage_at(&(starknet_address, high_key.to_be_bytes()))?;

        let low = U256::from_be_bytes(low.to_be_bytes());
        let high = U256::from_be_bytes(high.to_be_bytes());

        Ok(high << 128 | low)
    }

    /// Returns the nonce of the given address. For an EOA, uses the protocol level nonce.
    /// For a contract account, uses the Kakarot managed nonce stored in the contract account's storage.
    fn nonce_at(&mut self, evm_address: &Address) -> StateResult<U256> {
        let starknet_address = compute_starknet_address(evm_address);

        let class_hash = self.state.get_class_hash_at(&starknet_address)?;

        let nonce = if class_hash == *EOA_CLASS_HASH {
            self.state.get_nonce_at(&starknet_address)?
        } else if class_hash == *CONTRACT_ACCOUNT_CLASS_HASH {
            let key = get_storage_var_address("contract_account_nonce", &[])?;
            self.state
                .get_storage_at(&(starknet_address, key.to_be_bytes()))?
        } else {
            // We can't throw an error here, because it could just be an uninitialized account.
            Felt252::from(0_u8)
        };

        Ok(U256::from_be_bytes(nonce.to_be_bytes()))
    }

    /// Returns the bytecode of the given address. For an EOA, the bytecode_len_ storage variable will return 0,
    /// and the function will return an empty vector. For a contract account, the function will return the bytecode
    /// stored in the contract_account_bytecode storage variables. The function assumes that the bytecode is stored
    /// in 31 byte big-endian chunks.
    fn code_at(&mut self, evm_address: &Address) -> StateResult<Bytes> {
        // Get all storage addresses.
        let starknet_address = compute_starknet_address(evm_address);
        let bytecode_base_address = get_storage_var_address("contract_account_bytecode", &[])?;
        let pending_word_address = &bytecode_base_address - 2u32;
        let pending_word_len_address = &bytecode_base_address - 1u32;

        // Handle early return.
        let bytecode_len = self.state.get_storage_at(&(
            starknet_address.clone(),
            bytecode_base_address.to_be_bytes(),
        ))?;
        let bytecode_len = bytecode_len.to_u64().ok_or_else(|| {
            StateError::CustomError("Failed to convert Felt252 to u64".to_string())
        })?;

        // Handle the pending word.
        let pending_word_len = self
            .state
            .get_storage_at(&(
                starknet_address.clone(),
                pending_word_len_address.to_be_bytes(),
            ))?
            .to_usize()
            .ok_or_else(|| {
                StateError::CustomError("Failed to convert Felt252 to usize".to_string())
            })?;

        if bytecode_len == 0 && pending_word_len == 0 {
            return Ok(Bytes::default());
        }

        // Bytecode is stored in chunks of 31 bytes. At bytecode_base_address,
        // we store the number of chunks.
        let num_chunks = bytecode_len;
        let mut bytecode: Vec<u8> = Vec::new();

        for chunk_index in 0..num_chunks {
            let index = chunk_index / 256;
            let offset = chunk_index % 256;
            let key = poseidon_hash_many(&[
                felt_to_field_element(&bytecode_base_address).map_err(|err| {
                    StateError::CustomError(format!(
                        "Failed to convert felt to field element {}",
                        err
                    ))
                })?,
                FieldElement::from(index),
            ]);
            let key = &field_element_to_felt(&key) + offset;
            let code = self
                .state
                .get_storage_at(&(starknet_address.clone(), key.to_be_bytes()))?;
            bytecode.append(&mut code.to_bytes_be()[1..].to_vec());
        }

        let pending_word = self
            .state
            .get_storage_at(&(starknet_address, pending_word_address.to_be_bytes()))?;
        bytecode.append(&mut pending_word.to_bytes_be()[32 - pending_word_len..].to_vec());

        Ok(Bytes::from(bytecode))
    }

    /// Returns the balance of native tokens at the given address.
    /// Makes use of the default StateReader implementation from Blockifier.
    fn balance_at(&mut self, evm_address: &Address) -> StateResult<U256> {
        let starknet_address = compute_starknet_address(evm_address);
        let (low, high) = self
            .state
            .get_fee_token_balance(&BLOCK_CONTEXT.clone(), &starknet_address)?;

        let low = U256::from_be_bytes(low.to_be_bytes());
        let high = U256::from_be_bytes(high.to_be_bytes());

        Ok(high << 128 | low)
    }

    /// Converts the given signed transaction to a Starknet-rs transaction and executes it.
    // Since we are still missing the validate for the EOA, the signature is not added
    // to the transaction.
    fn execute_transaction(
        &mut self,
        transaction: TransactionSigned,
    ) -> TransactionExecutionResult<TransactionExecutionInfo> {
        let starknet_transaction = to_starknet_transaction(&transaction)
            .map_err(|err| TransactionError::CustomError(err.to_string()))?;
        self.execute(&starknet_transaction)
    }
}

pub(crate) fn poseidon_storage_base_address(storage_var_name: &str, keys: &[Felt252]) -> Felt252 {
    let selector = starknet_keccak(storage_var_name.as_bytes());

    let data: Vec<_> = keys
        .iter()
        .filter_map(|d| felt_to_field_element(d).ok())
        .collect();
    let data = [&[selector], &data[..]].concat();

    let key = poseidon_hash_many(&data);
    let key = field_element_to_felt(&key);

    key.mod_floor(&Felt252::from_bytes_be(
        &L2_ADDRESS_UPPER_BOUND.to_bytes_be(),
    ))
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
    use starknet::core::types::FieldElement;
    use starknet_api::hash::StarkFelt;

    #[test]
    fn test_store_bytecode() {
        // Given
        let mut sequencer = KakarotSequencer::new();
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
            KakarotAccount::new(&TEST_CONTRACT_ADDRESS, &bytecode, U256::ZERO, &[]).unwrap();
        sequencer.setup_account(account).unwrap();

        // Then
        let code = sequencer.code_at(&TEST_CONTRACT_ADDRESS).unwrap();
        assert_eq!(code, bytecode);
    }

    #[test]
    fn test_execute_simple_contract() {
        // Given
        let mut sequencer = KakarotSequencer::new();

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

        // When
        sequencer.setup_state().unwrap();
        let bytecode = Bytes::from(vec![
            0x60, 0x01, 0x60, 0x00, 0x55, 0x60, 0x02, 0x60, 0x00, 0x53, 0x60, 0x01, 0x60, 0x00,
            0xf3,
        ]); // PUSH 01 PUSH 00 SSTORE
        let nonce = U256::from(0);
        let contract_account =
            KakarotAccount::new(&TEST_CONTRACT_ADDRESS, &bytecode, nonce, &[]).unwrap();
        let eoa = KakarotAccount::new(&PUBLIC_KEY, &Bytes::default(), nonce, &[]).unwrap();
        sequencer.setup_account(contract_account).unwrap();
        sequencer.setup_account(eoa).unwrap();
        sequencer.execute_transaction(transaction).unwrap();

        // Then
        let storage = sequencer
            .storage_at(&TEST_CONTRACT_ADDRESS, U256::ZERO)
            .unwrap();
        assert_eq!(storage, U256::from(1));
    }

    #[test]
    fn test_starkfelt_to_bytecode() {
        // Given
        let felt = StarkFelt::from(0x0102030405u64);
        let len = 5;

        // When
        let result = FieldElement::from(felt).to_bytes_be()[32 - len..].to_vec();

        // Then
        assert_eq!(result, vec![0x01, 0x02, 0x03, 0x04, 0x05]);
    }
}
