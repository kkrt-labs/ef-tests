pub mod contract;
pub mod eoa;

use std::collections::HashMap;

use ef_tests::models::State;
use hive_utils::{
    kakarot::compute_starknet_address,
    madara::utils::{
        genesis_approve_kakarot, genesis_fund_starknet_address,
        genesis_set_storage_kakarot_contract_account, genesis_set_storage_starknet_contract,
    },
    types::{ContractAddress, StorageKey, StorageValue},
};
use kakarot_rpc_core::models::felt::Felt252Wrapper;
use katana_core::{
    backend::state::{MemDb, StorageRecord},
    constants::FEE_TOKEN_ADDRESS,
};
use revm_primitives::U256;
use starknet::core::types::FieldElement;
use starknet_api::{
    core::{ClassHash, ContractAddress as StarknetContractAddress, Nonce, PatriciaKey},
    hash::StarkFelt,
    state::StorageKey as StarknetStorageKey,
};
use tokio::sync::{RwLockReadGuard, RwLockWriteGuard};

use crate::utils::starknet::get_starknet_storage_key;

use self::{contract::initialize_contract_account, eoa::initialize_eoa};

pub struct ClassHashes {
    pub proxy_class_hash: FieldElement,
    pub eoa_class_hash: FieldElement,
    pub contract_account_class_hash: FieldElement,
}

impl ClassHashes {
    pub fn new(
        proxy_class_hash: FieldElement,
        eoa_class_hash: FieldElement,
        contract_account_class_hash: FieldElement,
    ) -> Self {
        Self {
            proxy_class_hash,
            eoa_class_hash,
            contract_account_class_hash,
        }
    }
}

/// Writes the blockchain test state to the Starknet storage
pub fn write_test_state(
    state: &State,
    kakarot_address: FieldElement,
    class_hashes: ClassHashes,
    starknet: &mut RwLockWriteGuard<'_, MemDb>,
) -> Result<(), ef_tests::Error> {
    // iterate through pre-state addresses
    for (address, account_info) in state.iter() {
        let mut storage = HashMap::new();
        let address = Felt252Wrapper::from(address.to_owned()).into();
        let starknet_address =
            compute_starknet_address(kakarot_address, class_hashes.proxy_class_hash, address);

        // balance
        write_fee_token(
            kakarot_address,
            starknet_address,
            account_info.balance.0,
            starknet,
        )
        .unwrap();

        // storage
        account_info.storage.iter().for_each(|(key, value)| {
            // Call genesis_set_storage_kakarot_contract_account util to get the storage tuples
            let storage_tuples =
                genesis_set_storage_kakarot_contract_account(starknet_address, key.0, value.0);
            write_madara_to_katana_storage(storage_tuples, &mut storage);
        });

        let proxy_implementation_class_hash = if account_info.code.is_empty() {
            initialize_eoa(kakarot_address, address, &mut storage);
            class_hashes.eoa_class_hash
        } else {
            initialize_contract_account(kakarot_address, address, &account_info.code, &mut storage);
            class_hashes.contract_account_class_hash
        };

        // write implementation state of proxy
        let proxy_implementation_storage_tuples = genesis_set_storage_starknet_contract(
            starknet_address,
            "_implementation",
            &[],
            proxy_implementation_class_hash,
            0, // 0 since it's storage value is felt
        );

        write_madara_to_katana_storage(vec![proxy_implementation_storage_tuples], &mut storage);

        // now, finally, we update the sequencer state with the eth->starknet address
        let address = StarknetContractAddress(
            Into::<StarkFelt>::into(starknet_address)
                .try_into()
                .unwrap(),
        );
        let account_nonce: FieldElement = Felt252Wrapper::try_from(account_info.nonce.0)
            .unwrap()
            .into();
        let storage_record = StorageRecord {
            nonce: Nonce(StarkFelt::from(account_nonce)),
            class_hash: ClassHash(class_hashes.proxy_class_hash.into()),
            storage: storage.clone(),
        };
        starknet.storage.insert(address, storage_record);
    }
    Ok(())
}

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

/// Writes a katana storage tuple to a hashmap.
pub fn write_katana_storage(
    data: Vec<(StarknetStorageKey, StarkFelt)>,
    destination: &mut HashMap<StarknetStorageKey, StarkFelt>,
) {
    for (key, value) in data {
        destination.insert(key, value);
    }
}

/// Writes a madara storage tuple to a hashmap.
pub fn write_madara_to_katana_storage(
    source: Vec<((ContractAddress, StorageKey), StorageValue)>,
    destination: &mut HashMap<StarknetStorageKey, StarkFelt>,
) {
    let reformatted_data = madara_to_katana_storage(source);
    write_katana_storage(reformatted_data, destination);
}

/// Writes the fee token balance and allowance to the katana storage.
pub fn write_fee_token(
    kakarot_address: FieldElement,
    starknet_address: FieldElement,
    balance: U256,
    starknet: &mut RwLockWriteGuard<'_, MemDb>,
) -> Result<(), eyre::Error> {
    write_balance(starknet_address, balance, starknet)?;
    write_allowance(kakarot_address, starknet_address, starknet)?;
    Ok(())
}

/// Writes the balance of an account to the katana storage.
pub fn write_balance(
    starknet_address: FieldElement,
    balance: U256,
    starknet: &mut RwLockWriteGuard<'_, MemDb>,
) -> Result<(), eyre::Error> {
    let balance_storage_tuples_madara = genesis_fund_starknet_address(starknet_address, balance);
    let fee_token_address =
        StarknetContractAddress(TryInto::<PatriciaKey>::try_into(*FEE_TOKEN_ADDRESS)?);
    let balance_storage = madara_to_katana_storage(balance_storage_tuples_madara);

    // funding balance
    for (storage_key, balance) in balance_storage {
        starknet
            .storage
            .entry(fee_token_address)
            .or_default()
            .storage
            .insert(storage_key, balance);
    }
    Ok(())
}

/// Writes the allowance of an account to the katana storage.
pub fn write_allowance(
    kakarot_address: FieldElement,
    starknet_address: FieldElement,
    starknet: &mut RwLockWriteGuard<'_, MemDb>,
) -> Result<(), eyre::Error> {
    let allowance = genesis_approve_kakarot(starknet_address, kakarot_address, U256::MAX);
    let balance_storage = madara_to_katana_storage(allowance);

    let fee_token_address =
        StarknetContractAddress(TryInto::<PatriciaKey>::try_into(*FEE_TOKEN_ADDRESS)?);

    // funding balance
    for (storage_key, balance) in balance_storage {
        starknet
            .storage
            .entry(fee_token_address)
            .or_default()
            .storage
            .insert(storage_key, balance);
    }
    Ok(())
}

/// Reads the balance of an account of the katana storage.
pub fn read_balance(
    starknet_address: FieldElement,
    starknet: &RwLockReadGuard<'_, MemDb>,
) -> Result<StarkFelt, eyre::Error> {
    let fee_token_address =
        StarknetContractAddress(TryInto::<PatriciaKey>::try_into(*FEE_TOKEN_ADDRESS)?);

    let storage_key = get_starknet_storage_key("ERC20_balances", &[starknet_address], 0);
    Ok(*starknet
        .storage
        .get(&fee_token_address)
        .unwrap()
        .storage
        .get(&storage_key)
        .unwrap())
}
