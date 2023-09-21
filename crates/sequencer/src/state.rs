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

/// Generic state structure for the sequencer.
/// The use of FxHashMap allows for a better performance.
/// This hash map is used by rustc. It uses a non cryptographic hash function
/// which is faster than the default hash function. Think about changing
/// if the test sequencer is used for tests outside of ef-tests.
/// See [rustc-hash](https://crates.io/crates/rustc-hash) for more information.
#[derive(Default)]
pub struct State {
    classes: FxHashMap<ClassHash, ContractClass>,
    compiled_class_hashes: FxHashMap<ClassHash, CompiledClassHash>,
    contracts: FxHashMap<ContractAddress, ClassHash>,
    storage: FxHashMap<ContractStorageKey, StarkFelt>,
    nonces: FxHashMap<ContractAddress, Nonce>,
}

/// State implementation for the sequencer. We use a mutable reference to the state
/// because this is what will be available during the implementation of the execution.
impl<'a> BlockifierState for &'a mut State {
    fn set_storage_at(
        &mut self,
        contract_address: ContractAddress,
        key: StorageKey,
        value: StarkFelt,
    ) {
        self.storage.insert((contract_address, key), value);
    }

    fn increment_nonce(&mut self, contract_address: ContractAddress) -> StateResult<()> {
        let current_nonce = self
            .nonces
            .get(&contract_address)
            .cloned()
            .unwrap_or_default();

        let mut current_nonce: u64 = current_nonce.0.try_into()?;
        current_nonce += 1;

        self.nonces
            .insert(contract_address, Nonce(StarkFelt::from(current_nonce)));

        Ok(())
    }

    fn set_class_hash_at(
        &mut self,
        contract_address: ContractAddress,
        class_hash: ClassHash,
    ) -> StateResult<()> {
        self.contracts.insert(contract_address, class_hash);
        Ok(())
    }

    fn set_contract_class(
        &mut self,
        class_hash: &ClassHash,
        contract_class: ContractClass,
    ) -> StateResult<()> {
        self.classes.insert(class_hash.to_owned(), contract_class);
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

    fn to_state_diff(&self) -> CommitmentStateDiff {
        unreachable!("to_state_diff should not be called in the sequencer")
    }
}

impl<'a> BlockifierStateReader for &'a mut State {
    /// Default: 0 for an uninitialized contract address.
    fn get_storage_at(
        &mut self,
        contract_address: ContractAddress,
        key: StorageKey,
    ) -> StateResult<StarkFelt> {
        Ok(self
            .storage
            .get(&(contract_address, key))
            .cloned()
            .unwrap_or_default())
    }

    /// Default: 0 for an uninitialized contract address.
    fn get_nonce_at(&mut self, contract_address: ContractAddress) -> StateResult<Nonce> {
        Ok(self
            .nonces
            .get(&contract_address)
            .cloned()
            .unwrap_or_default())
    }

    /// Default: 0 (uninitialized class hash) for an uninitialized contract address.
    fn get_class_hash_at(&mut self, contract_address: ContractAddress) -> StateResult<ClassHash> {
        Ok(self
            .contracts
            .get(&contract_address)
            .cloned()
            .unwrap_or_default())
    }

    /// Errors if the compiled class is not declared.
    fn get_compiled_contract_class(
        &mut self,
        class_hash: &ClassHash,
    ) -> StateResult<ContractClass> {
        self.classes
            .get(class_hash)
            .cloned()
            .ok_or_else(|| StateError::UndeclaredClassHash(class_hash.to_owned()))
    }

    /// Errors if the compiled class hash is not declared.
    fn get_compiled_class_hash(&mut self, class_hash: ClassHash) -> StateResult<CompiledClassHash> {
        self.compiled_class_hashes
            .get(&class_hash)
            .cloned()
            .ok_or_else(|| StateError::UndeclaredClassHash(class_hash))
    }
}

#[cfg(test)]
mod tests {
    use blockifier::execution::contract_class::ContractClassV0;

    use crate::constants::test_constants::{
        ONE_COMPILED_HASH, ONE_FELT, ONE_HASH, ONE_PATRICIA, TEST_CONTRACT_ADDRESS,
    };

    use super::*;

    #[test]
    fn test_storage() {
        // Given
        let mut state = &mut State::default();

        // When
        state.set_storage_at(*TEST_CONTRACT_ADDRESS, StorageKey(*ONE_PATRICIA), *ONE_FELT);

        // Then
        let expected = *ONE_FELT;
        let actual = state
            .get_storage_at(*TEST_CONTRACT_ADDRESS, StorageKey(*ONE_PATRICIA))
            .unwrap();
        assert_eq!(expected, actual);
    }

    #[test]
    fn test_nonce() {
        // Given
        let mut state = &mut State::default();

        // When
        state.increment_nonce(*TEST_CONTRACT_ADDRESS).unwrap();

        // Then
        let expected = Nonce(*ONE_FELT);
        let actual = state.get_nonce_at(*TEST_CONTRACT_ADDRESS).unwrap();
        assert_eq!(expected, actual);
    }

    #[test]
    fn test_class_hash() {
        // Given
        let mut state = &mut State::default();

        // When
        state
            .set_class_hash_at(*TEST_CONTRACT_ADDRESS, *ONE_HASH)
            .unwrap();

        // Then
        let expected = *ONE_HASH;
        let actual = state.get_class_hash_at(*TEST_CONTRACT_ADDRESS).unwrap();
        assert_eq!(expected, actual);
    }

    #[test]
    fn test_contract_class() {
        // Given
        let mut state = &mut State::default();

        // When
        state
            .set_contract_class(&ONE_HASH, ContractClass::V0(ContractClassV0::default()))
            .unwrap();

        // Then
        let expected = ContractClass::V0(ContractClassV0::default());
        let actual = state.get_compiled_contract_class(&ONE_HASH).unwrap();
        assert_eq!(expected, actual);
    }

    #[test]
    #[should_panic(
        expected = "UndeclaredClassHash(ClassHash(StarkFelt(\"0x0000000000000000000000000000000000000000000000000000000000000001\")))"
    )]
    fn test_uninitialized_contract_class() {
        // Given
        let mut state = &mut State::default();

        // When
        state.get_compiled_contract_class(&ONE_HASH).unwrap();
    }

    #[test]
    fn test_compiled_class_hash() {
        // Given
        let mut state = &mut State::default();

        // When
        state
            .set_compiled_class_hash(*ONE_HASH, *ONE_COMPILED_HASH)
            .unwrap();

        // Then
        let expected = *ONE_COMPILED_HASH;
        let actual = state.get_compiled_class_hash(*ONE_HASH).unwrap();
        assert_eq!(expected, actual);
    }

    #[test]
    #[should_panic(
        expected = "UndeclaredClassHash(ClassHash(StarkFelt(\"0x0000000000000000000000000000000000000000000000000000000000000001\"))"
    )]
    fn test_uninitialized_compiled_class_hash() {
        // Given
        let mut state = &mut State::default();

        // When
        state.get_compiled_class_hash(*ONE_HASH).unwrap();
    }
}
