use std::fs::{self, File};
use std::path::PathBuf;

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
use thiserror::Error;

use crate::commit::Committer;
use crate::serde::SerializableState;

/// Generic state structure for the sequencer.
/// The use of `FxHashMap` allows for a better performance.
/// This hash map is used by rustc. It uses a non cryptographic hash function
/// which is faster than the default hash function. Think about changing
/// if the test sequencer is used for tests outside of ef-tests.
/// See [rustc-hash](https://crates.io/crates/rustc-hash) for more information.
#[derive(Default, Debug, Serialize, Deserialize, PartialEq)]
pub struct State {
    pub classes: FxHashMap<ClassHash, ContractClass>,
    pub compiled_class_hashes: FxHashMap<ClassHash, CompiledClassHash>,
    pub contracts: FxHashMap<ContractAddress, ClassHash>,
    pub storage: FxHashMap<ContractStorageKey, StarkFelt>,
    pub nonces: FxHashMap<ContractAddress, Nonce>,
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

#[derive(Error, Debug)]
pub enum SerializationError {
    #[error("{reason:?}")]
    IoError {
        reason: String,
        context: std::io::Error,
    },
    #[error("{reason:?}")]
    SerdeJsonError {
        reason: String,
        context: serde_json::Error,
    },
}

impl State {
    /// This will serialize the current state, and will save it to a path
    pub fn dump_state_to_file(&self, path: &PathBuf) -> Result<(), SerializationError> {
        let serializable_state: SerializableState = self.into();
        let dump_file = File::options()
            .create(true)
            .read(true)
            .write(true)
            .append(false)
            .open(&path)
            .map_err(|error| SerializationError::IoError {
                reason: format!("failed to open file at path {:?}", &path),
                context: error,
            })?;

        serde_json::to_writer_pretty(dump_file, &serializable_state).map_err(|error| {
            SerializationError::SerdeJsonError {
                reason: format!("failed to serialize state to path {:?}", path),
                context: error,
            }
        })
    }

    /// This will read a dump from a file and initialize the state from it
    pub fn load_state_from_file(path: &PathBuf) -> Result<Self, SerializationError> {
        let dump = fs::read(path).unwrap();
        let serialiable_state: SerializableState = serde_json::from_slice(&dump).unwrap();

        Ok(serialiable_state.into())
    }
}

impl From<SerializableState> for State {
    fn from(serializable_state: SerializableState) -> Self {
        let state = Self {
            classes: serializable_state.classes.clone(),
            compiled_class_hashes: serializable_state.compiled_classes_hash.clone(),
            contracts: serializable_state.contracts.clone(),
            storage: serializable_state.storage.clone(),
            nonces: serializable_state.nonces.clone(),
        };

        state
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use blockifier::execution::contract_class::ContractClassV0;

    use crate::{
        constants::test_constants::{
            ONE_CLASS_HASH, ONE_COMPILED_CLASS_HASH, ONE_FELT, ONE_PATRICIA, TEST_CONTRACT,
            TEST_CONTRACT_ADDRESS, TEST_NONCE, TEST_STORAGE_KEY,
        },
        serde::SerializableState,
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

    #[test]
    pub fn dump_and_load_state() {
        let mut state = State::default();

        // setting up entry for state.classes
        let class_hash = *ONE_CLASS_HASH;
        let contract_class = include_str!("./test_data/cairo_0/compiled_classes/counter.json");
        let contract_class: ContractClassV0 =  serde_json::from_str(contract_class).expect("failed to deserialize ContractClass from ./crates/sequencer/test_data/cairo_1/compiled_classes/account.json");
        let contract_class = ContractClass::V0(contract_class);

        let compiled_class_hash = *ONE_COMPILED_CLASS_HASH;
        let contract_address = *TEST_CONTRACT_ADDRESS;
        let contract_storage_key: ContractStorageKey = (contract_address, *TEST_STORAGE_KEY);
        let storage_value = *ONE_FELT;
        let nonce = *TEST_NONCE;

        state.classes.insert(class_hash, contract_class);
        state
            .compiled_class_hashes
            .insert(class_hash, compiled_class_hash);
        state.contracts.insert(contract_address, class_hash);
        state.storage.insert(contract_storage_key, storage_value);
        state.nonces.insert(contract_address, nonce);

        let dump_file_path = PathBuf::from("./src/test_data/katana_dump.json");

        state
            .dump_state_to_file(&dump_file_path)
            .unwrap_or_else(|error| {
                panic!(
                    "failed to save state to path {:?},\n error {:?}",
                    dump_file_path, error
                )
            });

        let loaded_state = State::load_state_from_file(&dump_file_path).unwrap_or_else(|error| {
            panic!(
                "failed loading state from path {:?},\n error {}",
                dump_file_path, error
            )
        });

        let s1: SerializableState = SerializableState::from(&state);
        let s2: SerializableState = SerializableState::from(&loaded_state);

        assert_eq!(state, loaded_state);

        fs::remove_file(&dump_file_path).unwrap_or_else(|error| {
            panic!(
                "error in removing file from path {:?},\n error {}",
                dump_file_path, error
            )
        });
    }
}
