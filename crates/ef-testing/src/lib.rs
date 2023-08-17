use std::collections::HashMap;

use conformance_test_utils::types::{ContractAddress, StorageKey, StorageValue};
use starknet_api::{hash::StarkFelt, state::StorageKey as StarknetStorageKey};

fn _madara_to_katana_storage(
    source: Vec<((ContractAddress, StorageKey), StorageValue)>,
    destination: &mut HashMap<StarknetStorageKey, StarkFelt>,
) {
    for ((_, k), v) in source {
        let key = StarknetStorageKey(Into::<StarkFelt>::into(k.0).try_into().unwrap());
        let value = Into::<StarkFelt>::into(v.0);
        destination.insert(key, value);
    }
}
