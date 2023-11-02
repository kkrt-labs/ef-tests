use blockifier::state::cached_state::CommitmentStateDiff;
use blockifier::state::errors::StateError;
use blockifier::state::state_api::{
    State as BlockifierState, StateReader as BlockifierStateReader, StateResult,
};
use blockifier::{
    execution::contract_class::ContractClass, state::cached_state::ContractStorageKey,
};
use rustc_hash::FxHashMap;
use starknet_api::core::CompiledClassHash;
use starknet_api::state::StorageKey;
use starknet_api::{
    core::{ClassHash, ContractAddress, Nonce},
    hash::StarkFelt,
};

use serde::{Deserialize, Serialize};

use crate::commit::Committer;

/// Generic state structure for the sequencer.
/// The use of `FxHashMap` allows for a better performance.
/// This hash map is used by rustc. It uses a non cryptographic hash function
/// which is faster than the default hash function. Think about changing
/// if the test sequencer is used for tests outside of ef-tests.
/// See [rustc-hash](https://crates.io/crates/rustc-hash) for more information.
#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct State {
    pub(crate) classes: FxHashMap<ClassHash, ContractClass>,
    pub(crate) compiled_class_hashes: FxHashMap<ClassHash, CompiledClassHash>,
    pub(crate) contracts: FxHashMap<ContractAddress, ClassHash>,
    pub(crate) storage: FxHashMap<ContractStorageKey, StarkFelt>,
    pub(crate) nonces: FxHashMap<ContractAddress, Nonce>,
}

impl State {
    /// Helper function allowing to set the nonce of a contract.
    pub fn set_nonce(&mut self, contract_address: ContractAddress, nonce: Nonce) {
        self.nonces.insert(contract_address, nonce);
    }
}

impl Committer<State> for &mut State {}

/// State implementation for the sequencer. We use a mutable reference to the state
/// because this is what will be available during the implementation of the execution.
impl BlockifierState for &mut State {
    fn set_storage_at(
        &mut self,
        contract_address: ContractAddress,
        key: StorageKey,
        value: StarkFelt,
    ) {
        self.storage.insert((contract_address, key), value);
    }

    /// # Errors
    ///
    /// If the nonce overflows.
    fn increment_nonce(&mut self, contract_address: ContractAddress) -> StateResult<()> {
        let current_nonce = self
            .nonces
            .get(&contract_address)
            .copied()
            .unwrap_or_default();

        let mut current_nonce: u64 = current_nonce.0.try_into()?;
        if current_nonce == u64::MAX {
            return Err(StateError::StateReadError("Nonce overflow".into()));
        }
        current_nonce += 1;

        self.nonces
            .insert(contract_address, Nonce(StarkFelt::from(current_nonce)));

        Ok(())
    }

    /// # Errors
    ///
    /// If the contract address is linked to a class hash.
    fn set_class_hash_at(
        &mut self,
        contract_address: ContractAddress,
        class_hash: ClassHash,
    ) -> StateResult<()> {
        if self.contracts.get(&contract_address).is_some() {
            Err(StateError::UnavailableContractAddress(contract_address))
        } else {
            self.contracts.insert(contract_address, class_hash);
            Ok(())
        }
    }

    fn set_contract_class(
        &mut self,
        class_hash: &ClassHash,
        contract_class: ContractClass,
    ) -> StateResult<()> {
        self.classes.insert(*class_hash, contract_class);
        Ok(())
    }

    fn set_compiled_class_hash(
        &mut self,
        class_hash: ClassHash,
        compiled_class_hash: CompiledClassHash,
    ) -> StateResult<()> {
        self.compiled_class_hashes
            .insert(class_hash, compiled_class_hash);
        Ok(())
    }

    fn to_state_diff(&mut self) -> CommitmentStateDiff {
        unreachable!("to_state_diff should not be called in the sequencer")
    }
}

impl BlockifierStateReader for &mut State {
    fn get_storage_at(
        &mut self,
        contract_address: ContractAddress,
        key: StorageKey,
    ) -> StateResult<StarkFelt> {
        Ok(self
            .storage
            .get(&(contract_address, key))
            .copied()
            .unwrap_or_default())
    }

    fn get_nonce_at(&mut self, contract_address: ContractAddress) -> StateResult<Nonce> {
        Ok(self
            .nonces
            .get(&contract_address)
            .copied()
            .unwrap_or_default())
    }

    fn get_class_hash_at(&mut self, contract_address: ContractAddress) -> StateResult<ClassHash> {
        Ok(self
            .contracts
            .get(&contract_address)
            .copied()
            .unwrap_or_default())
    }

    /// # Errors
    ///
    /// If the compiled class is not declared.
    fn get_compiled_contract_class(
        &mut self,
        class_hash: &ClassHash,
    ) -> StateResult<ContractClass> {
        self.classes
            .get(class_hash)
            .cloned()
            .ok_or_else(|| StateError::UndeclaredClassHash(*class_hash))
    }

    /// # Errors
    ///
    /// If the compiled class hash is not declared.
    fn get_compiled_class_hash(&mut self, class_hash: ClassHash) -> StateResult<CompiledClassHash> {
        self.compiled_class_hashes
            .get(&class_hash)
            .copied()
            .ok_or_else(|| StateError::UndeclaredClassHash(class_hash))
    }
}

#[cfg(test)]
mod tests {
    use blockifier::execution::contract_class::ContractClassV0;

    use crate::constants::test_constants::{
        ONE_CLASS_HASH, ONE_COMPILED_CLASS_HASH, ONE_FELT, ONE_PATRICIA, TEST_CONTRACT,
    };

    use super::*;

    #[test]
    fn test_storage() {
        // Given
        let mut state = &mut State::default();

        // When
        state.set_storage_at(*TEST_CONTRACT, StorageKey(*ONE_PATRICIA), *ONE_FELT);

        // Then
        let expected = *ONE_FELT;
        let actual = state
            .get_storage_at(*TEST_CONTRACT, StorageKey(*ONE_PATRICIA))
            .unwrap();
        assert_eq!(expected, actual);
    }

    #[test]
    fn test_nonce() {
        // Given
        let mut state = &mut State::default();

        // When
        state.increment_nonce(*TEST_CONTRACT).unwrap();

        // Then
        let expected = Nonce(*ONE_FELT);
        let actual = state.get_nonce_at(*TEST_CONTRACT).unwrap();
        assert_eq!(expected, actual);
    }

    #[test]
    fn test_class_hash() {
        // Given
        let mut state = &mut State::default();

        // When
        state
            .set_class_hash_at(*TEST_CONTRACT, *ONE_CLASS_HASH)
            .unwrap();

        // Then
        let expected = *ONE_CLASS_HASH;
        let actual = state.get_class_hash_at(*TEST_CONTRACT).unwrap();
        assert_eq!(expected, actual);
    }

    #[test]
    fn test_contract_class() {
        // Given
        let mut state = &mut State::default();

        // When
        state
            .set_contract_class(
                &ONE_CLASS_HASH,
                ContractClass::V0(ContractClassV0::default()),
            )
            .unwrap();

        // Then
        let expected = ContractClass::V0(ContractClassV0::default());
        let actual = state.get_compiled_contract_class(&ONE_CLASS_HASH).unwrap();
        assert_eq!(expected, actual);
    }

    #[test]
    #[should_panic(expected = "UndeclaredClassHash")]
    fn test_uninitialized_contract_class() {
        // Given
        let mut state = &mut State::default();

        // When
        state.get_compiled_contract_class(&ONE_CLASS_HASH).unwrap();
    }

    #[test]
    fn test_compiled_class_hash() {
        // Given
        let mut state = &mut State::default();

        // When
        state
            .set_compiled_class_hash(*ONE_CLASS_HASH, *ONE_COMPILED_CLASS_HASH)
            .unwrap();

        // Then
        let expected = *ONE_COMPILED_CLASS_HASH;
        let actual = state.get_compiled_class_hash(*ONE_CLASS_HASH).unwrap();
        assert_eq!(expected, actual);
    }

    #[test]
    #[should_panic(expected = "UndeclaredClassHash")]
    fn test_uninitialized_compiled_class_hash() {
        // Given
        let mut state = &mut State::default();

        // When
        state.get_compiled_class_hash(*ONE_CLASS_HASH).unwrap();
    }
}
