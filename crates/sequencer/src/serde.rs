use std::{io, path::Path};

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

            map.insert(contract_storage_key, value_str.to_owned());
        }
        Ok(map)
    }
}
