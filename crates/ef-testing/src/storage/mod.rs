pub mod contract;
pub mod eoa;
pub mod fee_token;
pub mod models;

use ef_tests::models::Account;
use ef_tests::models::State;
use hive_utils::{
    kakarot::compute_starknet_address,
    types::{ContractAddress, StorageKey, StorageValue},
};
use kakarot_rpc_core::models::felt::Felt252Wrapper;
use katana_core::{
    backend::state::{MemDb, StorageRecord},
    constants::FEE_TOKEN_ADDRESS,
};
use starknet::core::types::FieldElement;
use starknet_api::{
    core::{ClassHash, ContractAddress as StarknetContractAddress, Nonce},
    hash::StarkFelt,
    state::StorageKey as StarknetStorageKey,
};
use tokio::sync::RwLockWriteGuard;

use crate::{models::error::RunnerError, utils::starknet::get_starknet_storage_key};

use self::{
    contract::{generate_evm_contract_storage, initialize_contract_account},
    eoa::initialize_eoa,
    fee_token::initialize_fee_token_storage,
    models::ClassHashes,
};

/// Writes the blockchain test state to the Starknet storage
pub fn write_test_state(
    state: &State,
    kakarot_address: FieldElement,
    class_hashes: ClassHashes,
    starknet: &mut RwLockWriteGuard<'_, MemDb>,
) -> Result<(), RunnerError> {
    // iterate through pre-state addresses
    for (address, account_info) in state.iter() {
        let mut starknet_contract_storage = Vec::new();
        let address = Felt252Wrapper::from(address.to_owned()).into();
        let starknet_address =
            compute_starknet_address(kakarot_address, class_hashes.proxy_class_hash, address);

        // Write evm storage
        starknet_contract_storage.append(&mut generate_evm_contract_storage(account_info)?);

        // get the implementation class hash and initialize the account
        let proxy_implementation_class_hash = if is_account_eoa(account_info) {
            starknet_contract_storage.append(&mut initialize_eoa(kakarot_address, address)?);
            class_hashes.eoa_class_hash
        } else {
            starknet_contract_storage.append(&mut initialize_contract_account(
                kakarot_address,
                address,
                &account_info.code,
            )?);
            class_hashes.contract_account_class_hash
        };

        // Write implementation state of proxy
        starknet_contract_storage.push(starknet_storage_key_value(
            "_implementation",
            &[],
            proxy_implementation_class_hash,
        )?);

        // Update the sequencer state with the eth->starknet address
        let address =
            StarknetContractAddress(Into::<StarkFelt>::into(starknet_address).try_into()?);
        let account_nonce: FieldElement = Felt252Wrapper::try_from(account_info.nonce.0)?.into();
        let storage_record = StorageRecord {
            nonce: Nonce(StarkFelt::from(account_nonce)),
            class_hash: ClassHash(class_hashes.proxy_class_hash.into()),
            storage: starknet_contract_storage.into_iter().collect(),
        };
        starknet.storage.insert(address, storage_record);

        // Update the sequencer state with the fee token balance and allowance
        let fee_token_storage = initialize_fee_token_storage(
            kakarot_address,
            starknet_address,
            account_info.balance.0,
        )?;
        let address =
            StarknetContractAddress(Into::<StarkFelt>::into(*FEE_TOKEN_ADDRESS).try_into()?);
        for (k, v) in fee_token_storage.iter() {
            starknet
                .storage
                .get_mut(&address)
                .ok_or_else(|| {
                    RunnerError::Other(format!("missing fee token address {:?}", address))
                })?
                .storage
                .insert(*k, *v);
        }
    }
    Ok(())
}

/// Converts a madara storage tuple to a katana storage tuple.
pub fn madara_to_katana_storage(
    source: Vec<((ContractAddress, StorageKey), StorageValue)>,
) -> Result<Vec<(StarknetStorageKey, StarkFelt)>, RunnerError> {
    source
        .into_iter()
        .map(|((_, k), v)| {
            let key = StarknetStorageKey(Into::<StarkFelt>::into(k.0).try_into()?);
            let value = Into::<StarkFelt>::into(v.0);
            Ok((key, value))
        })
        .collect()
}

pub(crate) fn starknet_storage_key_value(
    storage_var: &str,
    keys: &[FieldElement],
    value: FieldElement,
) -> Result<(StarknetStorageKey, StarkFelt), RunnerError> {
    let storage_key = get_starknet_storage_key(storage_var, keys, 0)?;
    let storage_value = Into::<StarkFelt>::into(value);
    Ok((storage_key, storage_value))
}

/// Returns the is_initialized storage tuple.
pub(crate) fn generate_is_initialized_storage(
) -> Result<(StarknetStorageKey, StarkFelt), RunnerError> {
    starknet_storage_key_value("is_initialized_", &[], FieldElement::ONE)
}

/// Returns the evm address storage tuple.
fn generate_evm_address_storage(
    evm_address: FieldElement,
) -> Result<(StarknetStorageKey, StarkFelt), RunnerError> {
    starknet_storage_key_value("evm_address", &[], evm_address)
}

/// Checks if an account is an EOA or a contract account.
pub fn is_account_eoa(account_info: &Account) -> bool {
    // an account contract might have both no code nor storage
    // however, an empty CA cannot make any update to its storage and nonce
    // so pre-state and post-state will be the same
    // therefore, we can set it as an EOA
    account_info.code.is_empty() && account_info.storage.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::*;

    #[rstest]
    #[case(
        r#"{
        "balance" : "0x00",
        "code" : "0x",
        "nonce" : "0x00",
        "storage" : {
            "0x01" : "0x01"
        }
    }"#,
        false
    )]
    #[case(
        r#"{
        "balance" : "0x00",
        "code" : "0x12",
        "nonce" : "0x00",
        "storage" : {
        }
    }"#,
        false
    )]
    #[case(
        r#"{
        "balance" : "0x00",
        "code" : "0x",
        "nonce" : "0x00",
        "storage" : {}
    }"#,
        true
    )]
    fn test_implementation_class_hash(#[case] input: &str, #[case] expected: bool) {
        // Given
        let account_info: Account = serde_json::from_str(input).unwrap();

        // When
        let result = is_account_eoa(&account_info);

        // Then
        assert_eq!(result, expected);
    }
}
