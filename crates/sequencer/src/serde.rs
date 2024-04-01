use std::{fs, io, path::Path};

use blockifier::execution::contract_class::ContractClass;
use hashbrown::HashMap;
use serde::{Deserialize, Serialize};
use starknet_api::{
    core::{ClassHash, CompiledClassHash, ContractAddress, Nonce},
    hash::StarkFelt,
};
use thiserror::Error;

use crate::state::{ContractStorageKey, State};

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

        let dump = serde_json::to_string(&serializable_state)?;

        fs::write(path, dump)?;

        Ok(())
    }

    /// This will read a dump from a file and initialize the state from it
    fn load_state_from_file(path: &Path) -> Result<Self, SerializationError> {
        let dump = fs::read(path)?;
        let serializable_state: SerializableState = serde_json::from_slice(&dump)?;

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
    pub classes: HashMap<ClassHash, ContractClass>,
    pub compiled_classes_hash: HashMap<ClassHash, CompiledClassHash>,
    pub contracts: HashMap<ContractAddress, ClassHash>,
    #[serde(with = "serialize_contract_storage")]
    pub storage: HashMap<ContractStorageKey, StarkFelt>,
    pub nonces: HashMap<ContractAddress, Nonce>,
}

mod serialize_contract_storage {
    use crate::state::ContractStorageKey;
    use hashbrown::HashMap;
    use serde::de::{Deserializer, MapAccess, Visitor};
    use serde::ser::{SerializeMap, Serializer};
    use starknet_api::hash::StarkFelt;
    use std::fmt;

    pub fn serialize<S>(
        map: &HashMap<ContractStorageKey, StarkFelt>,
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
    ) -> Result<HashMap<ContractStorageKey, StarkFelt>, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(MapContractStorageKeyVisitor)
    }

    struct MapContractStorageKeyVisitor;

    impl<'de> Visitor<'de> for MapContractStorageKeyVisitor {
        // The type that our Visitor is going to produce.
        type Value = HashMap<ContractStorageKey, StarkFelt>;

        // Format a message stating what data this Visitor expects to receive.
        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("ContractStorageKey to Value map")
        }

        // Deserialize Map from an abstract "map" provided by the
        // Deserializer. The MapAccess input is a callback provided by
        // the Deserializer to let us see each entry in the map.
        fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
        where
            M: MapAccess<'de>,
        {
            let mut map = HashMap::with_capacity(
                access.size_hint().unwrap_or(0)
            );

            // While there are entries remaining in the input, add them
            // into our map.
            while let Some((key, value)) = access.next_entry::<String, StarkFelt>()? {
                let key: ContractStorageKey = serde_json::from_str(&key).map_err(|error| {
                    serde::de::Error::custom(format!(
                        "failed to deserialize contract_storage_key {:?},\n error {}",
                        key, error
                    ))
                })?;
                map.insert(key, value);
            }

            Ok(map)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use blockifier::{
        execution::contract_class::{ContractClass, ContractClassV0},
        state::state_api::State as _,
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
        let storage_value = *ONE_FELT;
        let nonce = *TEST_NONCE;

        (&mut state)
            .set_contract_class(class_hash, contract_class)
            .expect("failed to set contract class");
        (&mut state)
            .set_compiled_class_hash(class_hash, compiled_class_hash)
            .expect("failed to set compiled class hash");
        (&mut state)
            .set_class_hash_at(contract_address, class_hash)
            .expect("failed to set class hash");
        (&mut state)
            .set_storage_at(contract_address, *TEST_STORAGE_KEY, storage_value)
            .expect("failed to set storage");
        state.set_nonce(contract_address, nonce);
        state.set_nonce(contract_address, nonce);

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

        assert_eq!(loaded_state, state);
    }
}
