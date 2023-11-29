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
use reth_primitives::{Address, Bytes, TransactionSigned};
use revm_primitives::U256;
use sequencer::{execution::Execution as _, transaction::BroadcastedTransactionWrapper};
use starknet::core::types::BroadcastedTransaction;
use starknet_api::{
    core::{PatriciaKey, L2_ADDRESS_UPPER_BOUND},
    hash::StarkFelt,
    state::StorageKey,
};
use starknet_crypto::{poseidon_hash_many, FieldElement};

use super::Evm;
use crate::evm_sequencer::{
    account::{AccountType, KakarotAccount},
    constants::{
        kkrt_constants_v1::{CONTRACT_ACCOUNT_CLASS_HASH, EOA_CLASS_HASH},
        CHAIN_ID, ETH_FEE_TOKEN_ADDRESS, KAKAROT_ADDRESS,
    },
    sequencer::KakarotSequencer,
    utils::{compute_starknet_address, split_u256, to_broadcasted_starknet_transaction},
};

impl Evm for KakarotSequencer {
    /// Sets up an EOA or contract account. Writes nonce, code and storage to the sequencer storage.
    fn setup_account(&mut self, account: KakarotAccount) -> StateResult<()> {
        let class_hash = if matches!(account.account_type, AccountType::EOA) {
            self.state
                .set_nonce(account.starknet_address, account.nonce);
            *EOA_CLASS_HASH
        } else {
            *CONTRACT_ACCOUNT_CLASS_HASH
        };

        for (k, v) in account.storage {
            (&mut self.state).set_storage_at(account.starknet_address, k, v);
        }

        // Set up the contract class hash
        (&mut self.state).set_class_hash_at(account.starknet_address, class_hash)?;

        // Add the address tot the Kakarot evm to starknet mapping
        let registry_base_address =
            get_storage_var_address("address_registry", &[account.evm_address]);
        (&mut self.state).set_storage_at(
            *KAKAROT_ADDRESS,
            registry_base_address,
            StarkFelt::from(account.account_type as u8),
        );
        (&mut self.state).set_storage_at(
            *KAKAROT_ADDRESS,
            offset_storage_base_address(registry_base_address, 1),
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
        let balance_key_low = get_fee_token_var_address(&starknet_address.try_into()?);
        let balance_key_high = next_storage_key(&balance_key_low)?;
        storage.append(&mut vec![
            (balance_key_low, StarkFelt::from(balance_values[0])),
            (balance_key_high, StarkFelt::from(balance_values[1])),
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
    fn get_storage_at(&mut self, evm_address: &Address, key: U256) -> StateResult<U256> {
        let keys = split_u256(key).map(Into::into);
        let low_key = compute_storage_base_address("contract_account_storage_keys", &keys);
        let high_key = offset_storage_base_address(low_key, 1);

        let starknet_address = compute_starknet_address(evm_address);

        let low = (&mut self.state).get_storage_at(starknet_address.try_into()?, low_key)?;
        let high = (&mut self.state).get_storage_at(starknet_address.try_into()?, high_key)?;

        let low = U256::from_be_bytes(Into::<FieldElement>::into(low).to_bytes_be());
        let high = U256::from_be_bytes(Into::<FieldElement>::into(high).to_bytes_be());

        Ok(high << 128 | low)
    }

    /// Returns the nonce of the given address. For an EOA, uses the protocol level nonce.
    /// For a contract account, uses the Kakarot managed nonce stored in the contract account's storage.
    fn get_nonce_at(&mut self, evm_address: &Address) -> StateResult<U256> {
        let starknet_address = compute_starknet_address(evm_address);

        let class_hash = (&mut self.state).get_class_hash_at(starknet_address.try_into()?)?;

        let nonce = if class_hash == *EOA_CLASS_HASH {
            (&mut self.state)
                .get_nonce_at(starknet_address.try_into()?)?
                .0
        } else if class_hash == *CONTRACT_ACCOUNT_CLASS_HASH {
            let key = compute_storage_base_address("contract_account_nonce", &[]);
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
    /// stored in the contract_account_bytecode storage variables. The function assumes that the bytecode is stored
    /// in 31 byte big-endian chunks.
    fn get_code_at(&mut self, evm_address: &Address) -> StateResult<Bytes> {
        // Get all storage addresses.
        let starknet_address = compute_starknet_address(evm_address);
        let bytecode_base_address = get_storage_var_address("contract_account_bytecode", &[]);
        let pending_word_address = offset_storage_base_address(bytecode_base_address, -2);
        let pending_word_len_address = offset_storage_base_address(bytecode_base_address, -1);

        // Handle early return.
        let bytecode_len = (&mut self.state)
            .get_storage_at(starknet_address.try_into()?, bytecode_base_address)?;
        let bytecode_len: u64 = bytecode_len.try_into()?;

        // Handle the pending word.
        let pending_word_len: usize = (&mut self.state)
            .get_storage_at(starknet_address.try_into()?, pending_word_len_address)?
            .try_into()?;

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
                (*bytecode_base_address.0.key()).into(),
                FieldElement::from(index),
            ]);
            let key = offset_storage_base_address(
                StorageKey(PatriciaKey::try_from(StarkFelt::from(key)).unwrap()),
                offset as i64,
            );
            let code = (&mut self.state).get_storage_at(starknet_address.try_into()?, key)?;
            bytecode.append(&mut FieldElement::from(code).to_bytes_be()[1..].to_vec());
        }

        let pending_word =
            (&mut self.state).get_storage_at(starknet_address.try_into()?, pending_word_address)?;
        bytecode.append(
            &mut FieldElement::from(pending_word).to_bytes_be()[32 - pending_word_len..].to_vec(),
        );

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
    // Since we are still missing the validate for the EOA, the signature is not added
    // to the transaction.
    fn execute_transaction(
        &mut self,
        transaction: TransactionSigned,
    ) -> TransactionExecutionResult<TransactionExecutionInfo> {
        let starknet_transaction =
            BroadcastedTransactionWrapper::new(BroadcastedTransaction::Invoke(
                to_broadcasted_starknet_transaction(&transaction).map_err(|err| {
                    TransactionExecutionError::ValidateTransactionError(
                        EntryPointExecutionError::InvalidExecutionInput {
                            input_descriptor: String::from("Failed to convert transaction"),
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

pub(crate) fn offset_storage_base_address(base_address: StorageKey, offset: i64) -> StorageKey {
    let base_address = stark_felt_to_felt(*base_address.0.key()) + Felt252::from(offset);
    let base_address = felt_to_stark_felt(&base_address);

    StorageKey(PatriciaKey::try_from(base_address).unwrap()) // infallible
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
    use reth_primitives::{sign_message, AccessList, Signature, TransactionSigned, TxEip1559};
    use revm_primitives::B256;
    use sequencer::state::State as SequencerState;
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
        let result = offset_storage_base_address(base_address, offset);

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
        let mut sequencer = KakarotSequencer::new(SequencerState::default());
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
            KakarotAccount::new(&TEST_CONTRACT_ADDRESS, &bytecode, U256::ZERO, vec![]).unwrap();
        sequencer.setup_account(account).unwrap();

        // Then
        let code = sequencer.get_code_at(&TEST_CONTRACT_ADDRESS).unwrap();
        assert_eq!(code, bytecode);
    }

    #[test]
    fn test_execute_simple_contract() {
        // Given
        let sequencer = KakarotSequencer::new(SequencerState::default());
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

        // When
        let bytecode = Bytes::from(vec![
            0x60, 0x01, 0x60, 0x00, 0x55, 0x60, 0x02, 0x60, 0x00, 0x53, 0x60, 0x01, 0x60, 0x00,
            0xf3,
        ]); // PUSH 01 PUSH 00 SSTORE
        let nonce = U256::from(0);
        let contract_account =
            KakarotAccount::new(&TEST_CONTRACT_ADDRESS, &bytecode, nonce, vec![]).unwrap();
        let eoa = KakarotAccount::new(&PUBLIC_KEY, &Bytes::default(), nonce, vec![]).unwrap();
        sequencer.setup_account(contract_account).unwrap();
        sequencer.setup_account(eoa).unwrap();
        sequencer.execute_transaction(transaction).unwrap();

        // Then
        let storage = sequencer
            .get_storage_at(&TEST_CONTRACT_ADDRESS, U256::ZERO)
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
