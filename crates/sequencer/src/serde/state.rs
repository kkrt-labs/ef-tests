use std::{fs, path::PathBuf};

use blockifier::state::cached_state::ContractStorageKey;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use starknet_api::{
    core::{ClassHash, CompiledClassHash, ContractAddress, Nonce},
    hash::StarkFelt,
};
use thiserror::Error;

use crate::state::State;

use super::contract::SerializableContractClass;

/// TODO: wrap the underlying errors in them
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

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct SerializableState {
    pub classes: FxHashMap<ClassHash, SerializableContractClass>,
    pub compiled_classes_hash: FxHashMap<ClassHash, CompiledClassHash>,
    pub contracts: FxHashMap<ContractAddress, ClassHash>,
    #[serde(with = "serialize_contract_storage")]
    pub storage: FxHashMap<ContractStorageKey, StarkFelt>,
    pub nonces: FxHashMap<ContractAddress, Nonce>,
}

impl SerializableState {
    pub fn dump_state(&self) -> Result<Vec<u8>, SerializationError> {
        let serialized_state_buffer =
            serde_json::to_vec(&self).map_err(|error| SerializationError::SerdeJsonError {
                reason: "Failed at serializing state to a buffer".to_string(),
                context: error,
            })?;

        Ok(serialized_state_buffer)
    }

    pub fn load_state(path: &PathBuf) -> Result<Self, SerializationError> {
        let serialized_state_buffer =
            fs::read(path).map_err(|error| SerializationError::IoError {
                reason: format!("Failed to read file at path {:?}", path),
                context: error,
            })?;

        serde_json::from_slice(&serialized_state_buffer).map_err(|error| {
            SerializationError::SerdeJsonError {
                reason: format!(
                    "Failed to deserialize the buffer {:?}",
                    serialized_state_buffer
                ),
                context: error,
            }
        })
    }
}

impl From<&State> for SerializableState {
    fn from(state: &State) -> Self {
        let mut serializable_state = Self {
            classes: FxHashMap::default(),
            compiled_classes_hash: state.compiled_class_hashes.clone(),
            contracts: state.contracts.clone(),
            storage: state.storage.clone(),
            nonces: state.nonces.clone(),
        };

        state
            .classes
            .iter()
            .for_each(|(class_hash, contract_class)| {
                serializable_state
                    .classes
                    .insert(*class_hash, contract_class.clone().into());
            });

        serializable_state
    }
}

mod serialize_contract_storage {
    use blockifier::state::cached_state::ContractStorageKey;
    use rustc_hash::FxHashMap;
    use serde::de::{Deserialize, Deserializer};
    use serde::ser::{Serialize, Serializer};
    use starknet_api::hash::StarkFelt;
    use std::collections::HashMap;

    pub fn serialize<S>(
        map: &FxHashMap<ContractStorageKey, StarkFelt>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut transformed: HashMap<String, String> = HashMap::new();
        for (contract_storage_key, storage_value) in map.iter() {
            let key_str = serde_json::to_string(&contract_storage_key).unwrap();
            let value_str = serde_json::to_string(&storage_value).unwrap();
            transformed.insert(key_str, value_str);
        }
        transformed.serialize(serializer)
    }

    pub fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<FxHashMap<ContractStorageKey, StarkFelt>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let transformed = HashMap::<String, String>::deserialize(deserializer)?;
        let mut map = FxHashMap::default();
        for (key_str, value_str) in transformed.iter() {
            let contract_storage_key: ContractStorageKey = serde_json::from_str(key_str)
                .unwrap_or_else(|error| {
                    panic!(
                        "failed to deserialize contract_storage_key {},\n error {}",
                        key_str, error
                    )
                });

            let storage_value: StarkFelt =
                serde_json::from_str(value_str).unwrap_or_else(|error| {
                    panic!(
                        "failed to deserialize storage_value {},\n error {}",
                        value_str, error
                    )
                });

            map.insert(contract_storage_key, storage_value);
        }
        Ok(map)
    }
}
