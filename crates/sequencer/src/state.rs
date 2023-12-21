use cairo_lang_utils::bigint::BigUintAsHex;
use num_traits::cast::ToPrimitive;
use rustc_hash::FxHashMap;
use starknet_in_rust::core::errors::state_errors::StateError;
use starknet_in_rust::felt::Felt252;
use starknet_in_rust::services::api::contract_classes::compiled_class::CompiledClass;
use starknet_in_rust::state::state_api::{State as SequencerState, StateChangesCount, StateReader};
use starknet_in_rust::state::state_cache::StorageEntry;
use starknet_in_rust::state::StateDiff;
use starknet_in_rust::utils::{Address, ClassHash, CompiledClassHash};

use crate::commit::Committer;

/// Generic state structure for the sequencer.
/// The use of `FxHashMap` allows for a better performance.
/// This hash map is used by rustc. It uses a non cryptographic hash function
/// which is faster than the default hash function. Think about changing
/// if the test sequencer is used for tests outside of ef-tests.
/// See [rustc-hash](https://crates.io/crates/rustc-hash) for more information.
#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct State {
    classes: FxHashMap<ClassHash, CompiledClass>,
    sierra_programs: FxHashMap<ClassHash, Vec<BigUintAsHex>>,
    compiled_class_hashes: FxHashMap<Felt252, Felt252>,
    contracts: FxHashMap<Address, ClassHash>,
    storage: FxHashMap<StorageEntry, Felt252>,
    nonces: FxHashMap<Address, Felt252>,
}

impl State {
    /// Helper function allowing to set the nonce of a contract.
    pub fn set_nonce(&mut self, contract_address: &Address, nonce: Felt252) {
        self.nonces.insert(contract_address.clone(), nonce);
    }
}

impl Committer for State {}

pub type StateResult<T> = Result<T, StateError>;

/// State implementation for the sequencer. We use a mutable reference to the state
/// because this is what will be available during the implementation of the execution.
impl SequencerState for State {
    fn set_storage_at(&mut self, storage_entry: &StorageEntry, value: Felt252) {
        self.storage.insert(storage_entry.clone(), value);
    }

    /// # Errors
    ///
    /// If the nonce overflows.
    fn increment_nonce(&mut self, contract_address: &Address) -> StateResult<()> {
        let current_nonce = self
            .nonces
            .get(contract_address)
            .cloned()
            .unwrap_or_default();

        let current_nonce = current_nonce.to_u64();
        if current_nonce.is_none() {
            return Err(StateError::CustomError("Nonce overflow".into()));
        }
        let mut current_nonce = current_nonce.unwrap();
        current_nonce += 1;

        self.nonces
            .insert(contract_address.clone(), Felt252::from(current_nonce));

        Ok(())
    }

    /// # Errors
    ///
    /// If the contract address is linked to a class hash.
    fn set_class_hash_at(
        &mut self,
        contract_address: Address,
        class_hash: ClassHash,
    ) -> StateResult<()> {
        if self.contracts.get(&contract_address).is_some() {
            Err(StateError::ContractAddressUnavailable(contract_address))
        } else {
            self.contracts.insert(contract_address, class_hash);
            Ok(())
        }
    }

    fn set_contract_class(
        &mut self,
        class_hash: &ClassHash,
        contract_class: &CompiledClass,
    ) -> StateResult<()> {
        self.classes.insert(*class_hash, contract_class.clone());
        Ok(())
    }

    fn set_compiled_class_hash(
        &mut self,
        class_hash: &Felt252,
        compiled_class_hash: &Felt252,
    ) -> StateResult<()> {
        self.compiled_class_hashes
            .insert(class_hash.clone(), compiled_class_hash.clone());
        Ok(())
    }

    fn deploy_contract(
        &mut self,
        contract_address: Address,
        class_hash: ClassHash,
    ) -> Result<(), StateError> {
        self.set_class_hash_at(contract_address, class_hash)
    }

    fn set_sierra_program(
        &mut self,
        compiled_class_hash: &Felt252,
        sierra_program: Vec<BigUintAsHex>,
    ) -> Result<(), StateError> {
        self.sierra_programs
            .insert(ClassHash::from(compiled_class_hash.clone()), sierra_program);
        Ok(())
    }

    fn apply_state_update(&mut self, state_updates: &StateDiff) -> Result<(), StateError> {
        for (address, class_hash) in state_updates.address_to_class_hash() {
            self.set_class_hash_at(address.clone(), *class_hash)?;
        }
        for (address, nonce) in state_updates.address_to_nonce() {
            self.set_nonce(address, nonce.clone());
        }
        for (class_hash, compiled_class_hash) in state_updates.class_hash_to_compiled_class() {
            self.set_compiled_class_hash(
                &Felt252::from_bytes_be(&class_hash.0[..]),
                &Felt252::from_bytes_be(&compiled_class_hash.0[..]),
            )?;
        }
        for (address, storage_updates) in state_updates.storage_updates() {
            for (storage_key, storage_value) in storage_updates {
                self.set_storage_at(
                    &(address.clone(), storage_key.to_be_bytes()),
                    storage_value.clone(),
                );
            }
        }
        Ok(())
    }

    fn count_actual_state_changes(
        &mut self,
        _fee_token_and_sender_address: Option<(&Address, &Address)>,
    ) -> Result<StateChangesCount, StateError> {
        todo!()
    }

    fn get_class_hash_at(&mut self, contract_address: &Address) -> Result<ClassHash, StateError> {
        Ok(self
            .contracts
            .get(contract_address)
            .copied()
            .unwrap_or_default())
    }

    fn get_nonce_at(&mut self, contract_address: &Address) -> Result<Felt252, StateError> {
        Ok(self
            .nonces
            .get(contract_address)
            .cloned()
            .unwrap_or_default())
    }

    fn get_storage_at(&mut self, storage_entry: &StorageEntry) -> Result<Felt252, StateError> {
        Ok(self.storage.get(storage_entry).cloned().unwrap_or_default())
    }

    fn get_compiled_class_hash(
        &mut self,
        class_hash: &ClassHash,
    ) -> Result<CompiledClassHash, StateError> {
        self.compiled_class_hashes
            .get(&Felt252::from_bytes_be(class_hash.to_bytes_be()))
            .cloned()
            .map(CompiledClassHash::from)
            .ok_or_else(|| StateError::NoneCompiledHash(*class_hash))
    }

    fn get_contract_class(&mut self, class_hash: &ClassHash) -> Result<CompiledClass, StateError> {
        self.classes
            .get(class_hash)
            .cloned()
            .ok_or_else(|| StateError::NoneCompiledClass(*class_hash))
    }

    fn get_sierra_program(
        &mut self,
        class_hash: &ClassHash,
    ) -> Result<Vec<BigUintAsHex>, StateError> {
        self.sierra_programs
            .get(class_hash)
            .cloned()
            .ok_or_else(|| StateError::NoneCompiledHash(*class_hash))
    }
}

/// StateReader implementation for the state structure.
impl StateReader for State {
    fn get_contract_class(&self, class_hash: &ClassHash) -> Result<CompiledClass, StateError> {
        self.classes
            .get(class_hash)
            .cloned()
            .ok_or_else(|| StateError::NoneCompiledClass(*class_hash))
    }

    fn get_class_hash_at(&self, contract_address: &Address) -> Result<ClassHash, StateError> {
        Ok(self
            .contracts
            .get(contract_address)
            .copied()
            .unwrap_or_default())
    }

    fn get_nonce_at(&self, contract_address: &Address) -> Result<Felt252, StateError> {
        Ok(self
            .nonces
            .get(contract_address)
            .cloned()
            .unwrap_or_default())
    }

    fn get_storage_at(&self, storage_entry: &StorageEntry) -> Result<Felt252, StateError> {
        Ok(self.storage.get(storage_entry).cloned().unwrap_or_default())
    }

    fn get_compiled_class_hash(
        &self,
        class_hash: &ClassHash,
    ) -> Result<CompiledClassHash, StateError> {
        self.compiled_class_hashes
            .get(&Felt252::from_bytes_be(class_hash.to_bytes_be()))
            .cloned()
            .map(CompiledClassHash::from)
            .ok_or_else(|| StateError::NoneCompiledHash(*class_hash))
    }
}

#[cfg(test)]
mod tests {

    use std::sync::Arc;

    use starknet_in_rust::CasmContractClass;

    use crate::constants::test_constants::{
        ONE, ONE_CLASS_HASH, TEST_CONTRACT, TWO, TWO_CLASS_HASH,
    };

    use super::*;

    #[test]
    fn test_storage() {
        // Given
        let state = &mut State::default();

        // When
        state.set_storage_at(&(TEST_CONTRACT.clone(), ONE.to_be_bytes()), ONE.clone());

        // Then
        let expected = ONE.clone();
        let actual = state
            .get_storage_at(&(TEST_CONTRACT.clone(), ONE.to_be_bytes()))
            .unwrap();
        assert_eq!(expected, actual);
    }

    #[test]
    fn test_nonce() {
        // Given
        let state = &mut State::default();

        // When
        state.increment_nonce(&TEST_CONTRACT).unwrap();

        // Then
        let expected = ONE.clone();
        let actual = state.get_nonce_at(&TEST_CONTRACT).unwrap();
        assert_eq!(expected, actual);
    }

    #[test]
    fn test_class_hash() {
        // Given
        let state = &mut State::default();

        // When
        state
            .set_class_hash_at(TEST_CONTRACT.clone(), *ONE_CLASS_HASH)
            .unwrap();

        // Then
        let expected = *ONE_CLASS_HASH;
        let actual = state.get_class_hash_at(&TEST_CONTRACT).unwrap();
        assert_eq!(expected, actual);
    }

    #[test]
    fn test_contract_class() {
        // Given
        let state = &mut State::default();

        // When
        state
            .set_contract_class(
                &ONE_CLASS_HASH,
                &CompiledClass::Casm(Arc::new(CasmContractClass::default())),
            )
            .unwrap();

        // Then
        let expected = CompiledClass::Casm(Arc::new(CasmContractClass::default()));
        let actual = state.get_contract_class(&ONE_CLASS_HASH).unwrap();
        assert_eq!(expected, actual);
    }

    #[test]
    #[should_panic(expected = "NoneCompiledClass(0x1)")]
    fn test_uninitialized_contract_class() {
        // Given
        let state = &mut State::default();

        // When
        state.get_contract_class(&ONE_CLASS_HASH).unwrap();
    }

    #[test]
    fn test_compiled_class_hash() {
        // Given
        let state = &mut State::default();

        // When
        state.set_compiled_class_hash(&ONE, &TWO).unwrap();

        // Then
        let expected = *TWO_CLASS_HASH;
        let actual = state.get_compiled_class_hash(&ONE_CLASS_HASH).unwrap();
        assert_eq!(expected, actual);
    }

    #[test]
    #[should_panic(expected = "NoneCompiledHash(0x1)")]
    fn test_uninitialized_compiled_class_hash() {
        // Given
        let state = &mut State::default();

        // When
        state.get_compiled_class_hash(&ONE_CLASS_HASH).unwrap();
    }
}
