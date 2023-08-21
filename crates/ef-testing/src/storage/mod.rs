pub mod contract;
pub mod eoa;

use std::collections::HashMap;

use hive_utils::types::{ContractAddress, StorageKey, StorageValue};
use starknet_api::{hash::StarkFelt, state::StorageKey as StarknetStorageKey};

/// Converts a madara storage tuple to a katana storage tuple.
pub fn madara_to_katana_storage(
    source: Vec<((ContractAddress, StorageKey), StorageValue)>,
) -> Vec<(StarknetStorageKey, StarkFelt)> {
    source
        .into_iter()
        .map(|((_, k), v)| {
            let key = StarknetStorageKey(Into::<StarkFelt>::into(k.0).try_into().unwrap());
            let value = Into::<StarkFelt>::into(v.0);
            (key, value)
        })
        .collect()
}

/// Writes a katana storage tuple to the katana storage.
pub fn write_katana_storage(
    data: Vec<(StarknetStorageKey, StarkFelt)>,
    destination: &mut HashMap<StarknetStorageKey, StarkFelt>,
) {
    for (key, value) in data {
        destination.insert(key, value);
    }
}

/// Writes a madara storage tuple to the katana storage.
pub fn write_madara_to_katana_storage(
    source: Vec<((ContractAddress, StorageKey), StorageValue)>,
    destination: &mut HashMap<StarknetStorageKey, StarkFelt>,
) {
    let reformatted_data = madara_to_katana_storage(source);
    write_katana_storage(reformatted_data, destination);
}
