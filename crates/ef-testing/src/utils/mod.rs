use std::collections::BTreeMap;

use alloy_primitives::{Address, Bytes, U256};
use ef_tests::models::{Account, State};

pub(crate) fn update_post_state(
    mut post_state: BTreeMap<Address, Account>,
    pre_state: State,
) -> BTreeMap<Address, Account> {
    for (k, pre_account) in pre_state.iter() {
        // If the post account's storage does not contain a key from the pre-state,
        // It means its storage was deleted
        // We need to insert in the tree that we use for assertions value 0x00 at this storage key.
        let post_account = post_state.entry(*k).or_insert_with(|| Account {
            nonce: U256::ZERO,
            balance: U256::ZERO,
            code: Bytes::default(),
            storage: BTreeMap::new(),
        });

        for storage_key in pre_account.storage.keys() {
            post_account
                .storage
                .entry(*storage_key)
                .or_insert(U256::ZERO);
        }
    }
    post_state
}

#[cfg(test)]
mod tests {
    use super::*;
    use ef_tests::models::State;

    #[test]
    fn test_update_post_state_empty_pre_state() {
        let post_state = BTreeMap::new();
        let pre_state: State = serde_json::from_str(r#"{}"#).unwrap();
        let updated_state = update_post_state(post_state, pre_state);
        assert!(updated_state.is_empty());
    }

    #[test]
    fn test_update_post_state() {
        // Mock pre and post states
        let pre_state: State = serde_json::from_str(r#"{
        "0x0000000000000000000000000000000000000110":{"balance":"0x0ba1a9ce0ba1a9ce","code":"0x6000808080804181f100","nonce":"0x01","storage":{}},
        "0x0000000000000000000000000000000000000100":{"balance":"0x0ba1a9ce0ba1a9ce","code":"0x6000808080804181f100","nonce":"0x01","storage":{}},"0x0000000000000000000000000000000000000200":{"balance":"0x0ba1a9ce0ba1a9ce","code":"0x6000808080804181f200","nonce":"0x01","storage":{}},"0x0000000000000000000000000000000000000300":{"balance":"0x0ba1a9ce0ba1a9ce","code":"0x60008080804181f400","nonce":"0x01","storage":{}},"0x0000000000000000000000000000000000000400":{"balance":"0x0ba1a9ce0ba1a9ce","code":"0x60008080804181fa00","nonce":"0x01","storage":{}},"0x000f3df6d732807ef1319fb7b8bb8522d0beac02":{"balance":"0x00","code":"0x3373fffffffffffffffffffffffffffffffffffffffe14604d57602036146024575f5ffd5b5f35801560495762001fff810690815414603c575f5ffd5b62001fff01545f5260205ff35b5f5ffd5b62001fff42064281555f359062001fff015500","nonce":"0x01","storage":{}},"0x2adc25665018aa1fe0e6bc666dac8fc2697ff9ba":{"balance":"0x0ba1a9ce0ba1a9ce","code":"0x","nonce":"0x01","storage":{}},"0xa94f5374fce5edbc8e2a8697c15331677e6ebf0b":{"balance":"0x0ba1a9ce0ba1a9ce","code":"0x","nonce":"0x01","storage":{}},"0xcccccccccccccccccccccccccccccccccccccccc":{"balance":"0x0ba1a9ce0ba1a9ce","code":"0x600080808080600435606481806101001460405780610200146040578061030014603857610400146031575bf1600055005b601801602b565b50601801602b565b50601b01602b56","nonce":"0x01","storage":{}}}"#).expect("Error while reading the pre state");
        let post_state: BTreeMap<Address, Account> = serde_json::from_str(r#"{"0x0000000000000000000000000000000000000100":{"balance":"0x0ba1a9ce0ba1a9ce","code":"0x6000808080804181f100","nonce":"0x01","storage":{}},"0x0000000000000000000000000000000000000200":{"balance":"0x0ba1a9ce0ba1a9ce","code":"0x6000808080804181f200","nonce":"0x01","storage":{}},"0x0000000000000000000000000000000000000300":{"balance":"0x0ba1a9ce0ba1a9ce","code":"0x60008080804181f400","nonce":"0x01","storage":{}},"0x0000000000000000000000000000000000000400":{"balance":"0x0ba1a9ce0ba1a9ce","code":"0x60008080804181fa00","nonce":"0x01","storage":{}},"0x000f3df6d732807ef1319fb7b8bb8522d0beac02":{"balance":"0x00","code":"0x3373fffffffffffffffffffffffffffffffffffffffe14604d57602036146024575f5ffd5b5f35801560495762001fff810690815414603c575f5ffd5b62001fff01545f5260205ff35b5f5ffd5b62001fff42064281555f359062001fff015500","nonce":"0x01","storage":{"0x03e8":"0x03e8"}},"0x2adc25665018aa1fe0e6bc666dac8fc2697ff9ba":{"balance":"0x0ba1a9ce0ba1a9ce","code":"0x","nonce":"0x01","storage":{}},"0xa94f5374fce5edbc8e2a8697c15331677e6ebf0b":{"balance":"0x0ba1a9ce0b9aa048","code":"0x","nonce":"0x02","storage":{}},"0xcccccccccccccccccccccccccccccccccccccccc":{"balance":"0x0ba1a9ce0ba1a9ce","code":"0x600080808080600435606481806101001460405780610200146040578061030014603857610400146031575bf1600055005b601801602b565b50601801602b565b50601b01602b56","nonce":"0x01","storage":{"0x00":"0x01"}}}"#).expect("Error while reading the post state");

        // Update the post state
        let updated_state = update_post_state(post_state, pre_state);

        // Expected post state with deleted storage for first account
        let expected_post_state: BTreeMap<Address, Account> = serde_json::from_str(r#"{
        "0x0000000000000000000000000000000000000110":{"balance":"0x","code":"0x","nonce":"0x","storage":{}},"0x0000000000000000000000000000000000000100":{"balance":"0x0ba1a9ce0ba1a9ce","code":"0x6000808080804181f100","nonce":"0x01","storage":{}},"0x0000000000000000000000000000000000000200":{"balance":"0x0ba1a9ce0ba1a9ce","code":"0x6000808080804181f200","nonce":"0x01","storage":{}},"0x0000000000000000000000000000000000000300":{"balance":"0x0ba1a9ce0ba1a9ce","code":"0x60008080804181f400","nonce":"0x01","storage":{}},"0x0000000000000000000000000000000000000400":{"balance":"0x0ba1a9ce0ba1a9ce","code":"0x60008080804181fa00","nonce":"0x01","storage":{}},"0x000f3df6d732807ef1319fb7b8bb8522d0beac02":{"balance":"0x00","code":"0x3373fffffffffffffffffffffffffffffffffffffffe14604d57602036146024575f5ffd5b5f35801560495762001fff810690815414603c575f5ffd5b62001fff01545f5260205ff35b5f5ffd5b62001fff42064281555f359062001fff015500","nonce":"0x01","storage":{"0x03e8":"0x03e8"}},"0x2adc25665018aa1fe0e6bc666dac8fc2697ff9ba":{"balance":"0x0ba1a9ce0ba1a9ce","code":"0x","nonce":"0x01","storage":{}},"0xa94f5374fce5edbc8e2a8697c15331677e6ebf0b":{"balance":"0x0ba1a9ce0b9aa048","code":"0x","nonce":"0x02","storage":{}},"0xcccccccccccccccccccccccccccccccccccccccc":{"balance":"0x0ba1a9ce0ba1a9ce","code":"0x600080808080600435606481806101001460405780610200146040578061030014603857610400146031575bf1600055005b601801602b565b50601801602b565b50601b01602b56","nonce":"0x01","storage":{"0x00":"0x01"}}}"#).expect("Error while reading the post state");

        // Assert the updated state
        assert_eq!(updated_state, expected_post_state);
    }
}
