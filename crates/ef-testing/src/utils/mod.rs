use std::{collections::BTreeMap, fs, path::Path};

use ef_tests::models::{Account, State};
use reth_primitives::{Bytes, JsonU256};
use revm_primitives::{B160, U256};

use crate::models::error::RunnerError;

pub(crate) fn load_file(path: &Path) -> Result<String, RunnerError> {
    fs::read_to_string(path).map_err(|error| RunnerError::Io {
        path: path.into(),
        error: error.to_string(),
    })
}

pub(crate) fn update_post_state(
    mut post_state: BTreeMap<B160, Account>,
    pre_state: State,
) -> BTreeMap<B160, Account> {
    for (k, _) in pre_state.iter() {
        // If the post account's storage does not contain a key from the pre-state,
        // It means its storage was deleted
        // We need to insert in the tree that we use for assertions value 0x00 at this storage key.
        if post_state.contains_key(k) {
            let post_account_storage = &post_state
                .get(k)
                .map(|x| x.storage.clone())
                .unwrap_or_default();

            let pre_account_storage = pre_state
                .get(k)
                .map(|x| x.storage.clone())
                .unwrap_or_default();

            for storage_key in pre_account_storage.keys() {
                if !post_account_storage.contains_key(storage_key) {
                    post_state
                        .get_mut(k)
                        .unwrap()
                        .storage
                        .insert(*storage_key, U256::ZERO.into());
                }
            }
        }

        if !post_state.contains_key(k) {
            post_state.insert(
                *k,
                Account {
                    nonce: JsonU256::default(),
                    balance: JsonU256::default(),
                    code: Bytes::default(),
                    storage: BTreeMap::new(),
                },
            );
        }
    }
    post_state
}
