use std::{fs, io, path::Path};

use crate::state::State;
use blockifier::{
    execution::contract_class::ContractClass, state::cached_state::ContractStorageKey,
};
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use starknet_api::{
    core::{ClassHash, CompiledClassHash, ContractAddress, Nonce},
    hash::StarkFelt,
};

use thiserror::Error;

pub trait DumpLoad {
    fn dump_state_to_file(self, file_path: &Path) -> Result<(), SerializationError>;

    fn load_state_from_file(file_path: &Path) -> Result<Self, SerializationError>
    where
        Self: Sized;
}

impl DumpLoad for State {
    /// This will serialize the current state, and will save it to a path
    fn dump_state_to_file(self, path: &Path) -> Result<(), SerializationError> {
        let serializable_state: SerializableState = self.into();

        let dump = serde_json::to_string(&serializable_state)
            .map_err(SerializationError::SerdeJsonError)?;

        fs::write(path, dump).map_err(SerializationError::IoError)?;

        Ok(())
    }

    /// This will read a dump from a file and initialize the state from it
    fn load_state_from_file(path: &Path) -> Result<Self, SerializationError> {
        let dump = fs::read(path).unwrap();
        let serializable_state: SerializableState =
            serde_json::from_slice(&dump).map_err(SerializationError::SerdeJsonError)?;

        Ok(serializable_state.into())
    }
}

#[derive(Error, Debug)]
pub enum SerializationError {
    #[error(transparent)]
    IoError(#[from] io::Error),
    #[error(transparent)]
    SerdeJsonError(#[from] serde_json::Error),
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct SerializableState {
    pub classes: FxHashMap<ClassHash, ContractClass>,
    pub compiled_classes_hash: FxHashMap<ClassHash, CompiledClassHash>,
    pub contracts: FxHashMap<ContractAddress, ClassHash>,
    #[serde(with = "serialize_contract_storage")]
    pub storage: FxHashMap<ContractStorageKey, StarkFelt>,
    pub nonces: FxHashMap<ContractAddress, Nonce>,
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
            classes: serializable_state.classes.clone(),
            compiled_class_hashes: serializable_state.compiled_classes_hash.clone(),
            contracts: serializable_state.contracts.clone(),
            storage: serializable_state.storage.clone(),
            nonces: serializable_state.nonces,
        }
    }
}

mod serialize_contract_storage {
    use blockifier::state::cached_state::ContractStorageKey;
    use rustc_hash::FxHashMap;
    use serde::de::{Deserialize, Deserializer};
    use serde::ser::{Serialize, SerializeMap, Serializer};
    use starknet_api::hash::StarkFelt;
    use std::collections::HashMap;

    pub fn serialize<S>(
        map: &FxHashMap<ContractStorageKey, StarkFelt>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut serialized_map = serializer.serialize_map(Some(map.len()))?;
        for (k, v) in map {
            let key = serde_json::to_string(k).map_err(|error| {
                serde::ser::Error::custom(format!(
                    "failed to deserialize contract_storage_key {:?},\n error {}",
                    k, error
                ))
            })?;

            serialized_map.serialize_entry(&key, &v)?;
        }
        serialized_map.end()
    }

    pub fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<FxHashMap<ContractStorageKey, StarkFelt>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let transformed = HashMap::<String, StarkFelt>::deserialize(deserializer)?;
        let mut map: FxHashMap<ContractStorageKey, StarkFelt> = FxHashMap::default();
        for (key_str, value_str) in transformed.iter() {
            let contract_storage_key: ContractStorageKey =
                serde_json::from_str(key_str).map_err(|error| {
                    serde::de::Error::custom(format!(
                        "failed to deserialize contract_storage_key {},\n error {}",
                        key_str, error
                    ))
                })?;
#[cfg(test)]
mod tests {
    use super::*;
    use blockifier::{
        execution::contract_class::{ContractClass, ContractClassV0},
        state::cached_state::ContractStorageKey,
    };

    use crate::{
        constants::test_constants::{
            ONE_CLASS_HASH, ONE_COMPILED_CLASS_HASH, ONE_FELT, TEST_CONTRACT, TEST_NONCE,
            TEST_STORAGE_KEY,
        },
        state::State,
    };

    #[test]
    pub fn dump_and_load_state() {
        let mut state = State::default();

        // setting up entry for state.classes
        let class_hash = *ONE_CLASS_HASH;
        let contract_class = include_str!("./test_data/cairo_0/compiled_classes/counter.json");
        let contract_class: ContractClassV0 =  serde_json::from_str(contract_class).expect("failed to deserialize ContractClass from ./crates/sequencer/test_data/cairo_1/compiled_classes/account.json");
        let contract_class = ContractClass::V0(contract_class);

        let compiled_class_hash = *ONE_COMPILED_CLASS_HASH;
        let contract_address = *TEST_CONTRACT;
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

        let temp_file = tempfile::NamedTempFile::new().expect("failed open named temp file");
        let dump_file_path = temp_file.into_temp_path();

        state
            .clone()
            .dump_state_to_file(&dump_file_path)
            .expect("failed to save dump to file");

        let loaded_state =
            State::load_state_from_file(&dump_file_path).expect("failed to load state from file");
        assert_eq!(state, loaded_state);

        dump_file_path.close().expect("failed to close temp file");
    }
}
