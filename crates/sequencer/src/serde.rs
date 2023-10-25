use std::io;

use blockifier::{
    execution::contract_class::ContractClass, state::cached_state::ContractStorageKey,
};
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use starknet_api::{
    core::{ClassHash, CompiledClassHash, ContractAddress, Nonce},
    hash::StarkFelt,
};

use crate::state::State;
use thiserror::Error;

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
impl From<&State> for SerializableState {
    fn from(state: &State) -> Self {
        Self {
            classes: state.classes.clone(),
            compiled_classes_hash: state.compiled_class_hashes.clone(),
            contracts: state.contracts.clone(),
            storage: state.storage.clone(),
            nonces: state.nonces.clone(),
        }
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
            let contract_storage_key: ContractStorageKey =
                serde_json::from_str(key_str).map_err(|error| {
                    serde::de::Error::custom(format!(
                        "failed to deserialize contract_storage_key {},\n error {}",
                        key_str, error
                    ))
                })?;

            let storage_value: StarkFelt = serde_json::from_str(value_str).map_err(|error| {
                serde::de::Error::custom(format!(
                    "failed to deserialize storage_value {},\n error {}",
                    value_str, error
                ))
            })?;

            map.insert(contract_storage_key, storage_value);
        }
        Ok(map)
    }
}
