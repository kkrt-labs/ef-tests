use crate::commit::Committer;
use crate::serde::SerializableState;
use blockifier::execution::contract_class::{ContractClass, NativeContractClassV1};
use blockifier::state::errors::StateError;
use blockifier::state::state_api::{
    State as BlockifierState, StateReader as BlockifierStateReader, StateResult,
};
use hashbrown::HashMap;
use serde::{Deserialize, Serialize};
use starknet::core::types::Felt;
use starknet_api::core::CompiledClassHash;
use starknet_api::core::{ClassHash, ContractAddress, Nonce};
use starknet_api::state::StorageKey;

pub type ContractStorageKey = (ContractAddress, StorageKey);

/// Generic state structure for the sequencer.
/// The use of `HashMap` implementation from hashbrown allows for a better performance.
/// See [Performance](https://github.com/rust-lang/hashbrown?tab=readme-ov-file#performance)
#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct State {
    classes: HashMap<ClassHash, ContractClass>,
    compiled_class_hashes: HashMap<ClassHash, CompiledClassHash>,
    contracts: HashMap<ContractAddress, ClassHash>,
    storage: HashMap<ContractStorageKey, Felt>,
    nonces: HashMap<ContractAddress, Nonce>,
}

impl From<State> for SerializableState {
    fn from(state: State) -> Self {
        Self {
            classes: state.classes,
            compiled_classes_hash: state.compiled_class_hashes,
            contracts: state.contracts,
            storage: state.storage,
            nonces: state.nonces,
        }
    }
}

impl From<SerializableState> for State {
    fn from(serializable_state: SerializableState) -> Self {
        Self {
            classes: serializable_state.classes,
            compiled_class_hashes: serializable_state.compiled_classes_hash,
            contracts: serializable_state.contracts,
            storage: serializable_state.storage,
            nonces: serializable_state.nonces,
        }
    }
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
        value: Felt,
    ) -> StateResult<()> {
        self.storage.insert((contract_address, key), value);
        Ok(())
    }

    /// # Errors
    ///
    /// If the nonce overflows.
    fn increment_nonce(&mut self, contract_address: ContractAddress) -> StateResult<()> {
        let mut current_nonce = self
            .nonces
            .get(&contract_address)
            .copied()
            .unwrap_or_default();

        if current_nonce == Nonce(Felt::from(u64::MAX)) {
            return Err(StateError::StateReadError("Nonce overflow".into()));
        }
        current_nonce.0 += Felt::ONE;

        self.nonces.insert(contract_address, current_nonce);

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
        class_hash: ClassHash,
        contract_class: ContractClass,
    ) -> StateResult<()> {
        match &contract_class {
            &ContractClass::V0(_) => println!("Setting V0 contract class"),
            &ContractClass::V1(_) => println!("Setting V1 contract class"),
            &ContractClass::V1Native(_) => println!("Setting V1Native contract class"),
        };
        self.classes.insert(class_hash, contract_class);
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

    fn add_visited_pcs(&mut self, _class_hash: ClassHash, _pcs: &std::collections::HashSet<usize>) {
        unreachable!("add_visited_pcs should not be called in the sequencer")
    }
}

impl BlockifierStateReader for &mut State {
    fn get_storage_at(
        &self,
        contract_address: ContractAddress,
        key: StorageKey,
    ) -> StateResult<Felt> {
        Ok(self
            .storage
            .get(&(contract_address, key))
            .copied()
            .unwrap_or_default())
    }

    fn get_nonce_at(&self, contract_address: ContractAddress) -> StateResult<Nonce> {
        Ok(self
            .nonces
            .get(&contract_address)
            .copied()
            .unwrap_or_default())
    }

    fn get_class_hash_at(&self, contract_address: ContractAddress) -> StateResult<ClassHash> {
        Ok(self
            .contracts
            .get(&contract_address)
            .copied()
            .unwrap_or_default())
    }

    /// # Errors
    ///
    /// If the compiled class is not declared.
    fn get_compiled_contract_class(&self, class_hash: ClassHash) -> StateResult<ContractClass> {
        let res = self.classes
            .get(&class_hash)
            .cloned()
            .ok_or_else(|| StateError::UndeclaredClassHash(class_hash))?;
        Ok(res)
    }

    /// # Errors
    ///
    /// If the compiled class hash is not declared.
    fn get_compiled_class_hash(&self, class_hash: ClassHash) -> StateResult<CompiledClassHash> {
        self.compiled_class_hashes
            .get(&class_hash)
            .copied()
            .ok_or_else(|| StateError::UndeclaredClassHash(class_hash))
    }
}



#[cfg(test)]
mod tests {
    use blockifier::execution::contract_class::ContractClassV0;

    use crate::constants::test_constants::{ONE_PATRICIA, TEST_CONTRACT};

    use super::*;

    #[test]
    fn test_storage() {
        // Given
        let mut state = &mut State::default();

        // When
        state
            .set_storage_at(*TEST_CONTRACT, StorageKey(*ONE_PATRICIA), Felt::ONE)
            .expect("failed to set storage");

        // Then
        let expected = Felt::ONE;
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
        let expected = Nonce(Felt::ONE);
        let actual = state.get_nonce_at(*TEST_CONTRACT).unwrap();
        assert_eq!(expected, actual);
    }

    #[test]
    fn test_class_hash() {
        // Given
        let mut state = &mut State::default();

        // When
        state
            .set_class_hash_at(*TEST_CONTRACT, ClassHash(Felt::ONE))
            .unwrap();

        // Then
        let expected = ClassHash(Felt::ONE);
        let actual: ClassHash = state.get_class_hash_at(*TEST_CONTRACT).unwrap();
        assert_eq!(expected, actual);
    }

    #[test]
    fn test_contract_class() {
        // Given
        let mut state = &mut State::default();

        // When
        state
            .set_contract_class(
                ClassHash(Felt::ONE),
                ContractClass::V0(ContractClassV0::default()),
            )
            .unwrap();

        // Then
        let expected = ContractClass::V0(ContractClassV0::default());
        let actual = state
            .get_compiled_contract_class(ClassHash(Felt::ONE))
            .unwrap();
        assert_eq!(expected, actual);
    }

    #[test]
    #[should_panic(expected = "UndeclaredClassHash")]
    fn test_uninitialized_contract_class() {
        // Given
        let state = &mut State::default();

        // When
        state
            .get_compiled_contract_class(ClassHash(Felt::ONE))
            .unwrap();
    }

    #[test]
    fn test_compiled_class_hash() {
        // Given
        let mut state = &mut State::default();

        // When
        state
            .set_compiled_class_hash(ClassHash(Felt::ONE), CompiledClassHash(Felt::ONE))
            .unwrap();

        // Then
        let expected = CompiledClassHash(Felt::ONE);
        let actual = state.get_compiled_class_hash(ClassHash(Felt::ONE)).unwrap();
        assert_eq!(expected, actual);
    }

    #[test]
    #[should_panic(expected = "UndeclaredClassHash")]
    fn test_uninitialized_compiled_class_hash() {
        // Given
        let state = &mut State::default();

        // When
        state.get_compiled_class_hash(ClassHash(Felt::ONE)).unwrap();
    }
}
