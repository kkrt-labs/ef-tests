pub mod contract;
pub mod eoa;
pub mod fee_token;
pub mod models;

use ef_tests::models::{Account, State};
use hive_utils::{
    kakarot::compute_starknet_address,
    madara::utils::genesis_set_storage_kakarot_contract_account,
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
    contract::initialize_contract_account, eoa::initialize_eoa, fee_token::get_fee_token_storage,
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
        starknet_contract_storage.append(&mut get_evm_contract_storage(account_info)?);

        // Write implementation state
        let proxy_implementation_class_hash = if account_info.code.is_empty() {
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
        starknet_contract_storage.push(get_starknet_storage(
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
        let fee_token_storage =
            get_fee_token_storage(kakarot_address, starknet_address, account_info.balance.0)?;
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

fn get_evm_contract_storage(
    account: &Account,
) -> Result<Vec<(StarknetStorageKey, StarkFelt)>, RunnerError> {
    account
        .storage
        .iter()
        .flat_map(|(key, value)| {
            // Call genesis_set_storage_kakarot_contract_account util to get the storage tuples
            let storage =
                genesis_set_storage_kakarot_contract_account(FieldElement::ZERO, key.0, value.0);
            match madara_to_katana_storage(storage) {
                Ok(storage) => storage.into_iter().map(Ok).collect::<Vec<_>>(),
                Err(err) => vec![Err(err)],
            }
        })
        .collect::<Result<Vec<(StarknetStorageKey, StarkFelt)>, RunnerError>>()
}

pub(crate) fn get_starknet_storage(
    storage_var: &str,
    keys: &[FieldElement],
    value: FieldElement,
) -> Result<(StarknetStorageKey, StarkFelt), RunnerError> {
    let storage_key = get_starknet_storage_key(storage_var, keys, 0)?;
    let storage_value = Into::<StarkFelt>::into(value);
    Ok((storage_key, storage_value))
}

/// Returns the is_initialized storage tuple.
pub(crate) fn get_is_initialized() -> Result<(StarknetStorageKey, StarkFelt), RunnerError> {
    get_starknet_storage("is_initialized_", &[], FieldElement::ONE)
}

/// Returns the evm address storage tuple.
fn get_evm_address(
    evm_address: FieldElement,
) -> Result<(StarknetStorageKey, StarkFelt), RunnerError> {
    get_starknet_storage("evm_address", &[], evm_address)
}
