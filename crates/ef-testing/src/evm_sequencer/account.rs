use blockifier::abi::abi_utils::{get_storage_var_address, get_uint256_storage_var_addresses};
use reth_primitives::{Address, Bytes};
use revm_primitives::U256;
use starknet_api::{
    core::{ContractAddress, Nonce},
    hash::StarkFelt,
    state::StorageKey,
    StarknetApiError,
};

use crate::evm_sequencer::utils::split_u256;

use super::{
    evm_state::KakarotConfig,
    types::FeltSequencer,
    utils::{compute_starknet_address, split_bytecode_to_starkfelt},
};

pub struct KakarotAccount {
    pub(crate) starknet_address: ContractAddress,
    pub(crate) evm_address: StarkFelt,
    pub(crate) nonce: Nonce,
    pub(crate) storage: Vec<(StorageKey, StarkFelt)>,
    pub(crate) account_type: AccountType,
}

pub enum AccountType {
    EOA,
    Contract,
}

impl KakarotAccount {
    pub fn new(
        kakarot_config: &KakarotConfig,
        evm_address: &Address,
        code: &Bytes,
        nonce: U256,
        evm_storage: Vec<(U256, U256)>,
    ) -> Result<Self, StarknetApiError> {
        let nonce = StarkFelt::from(TryInto::<u128>::try_into(nonce).map_err(|err| {
            StarknetApiError::OutOfRange {
                string: err.to_string(),
            }
        })?);

        let starknet_address = compute_starknet_address(evm_address);
        let starknet_address = ContractAddress::try_from(starknet_address)?;

        let evm_address = TryInto::<FeltSequencer>::try_into(*evm_address)
            .unwrap()
            .into(); // infallible

        let mut storage = vec![
            (get_storage_var_address("evm_address", &[]), evm_address),
            (
                get_storage_var_address("is_initialized_", &[]),
                StarkFelt::from(1u8),
            ),
            (
                get_storage_var_address("Ownable_owner", &[]),
                kakarot_config.address,
            ),
            (
                get_storage_var_address("bytecode_len_", &[]),
                StarkFelt::from(code.len() as u32),
            ),
            (
                get_storage_var_address("kakarot_address", &[]),
                kakarot_config.address,
            ),
        ];

        // Initialize the implementation and nonce based on account type.
        // The account is an EOA if it has no bytecode and no storage (or all storage is zero).
        let has_code_or_storage = !code.is_empty() || evm_storage.iter().any(|x| x.1 != U256::ZERO);
        let account_type = if !has_code_or_storage {
            storage.push((
                get_storage_var_address("_implementation", &[]),
                kakarot_config.eoa_class_hash,
            ));
            AccountType::EOA
        } else {
            storage.append(&mut vec![
                (get_storage_var_address("nonce", &[]), nonce),
                (
                    get_storage_var_address("_implementation", &[]),
                    kakarot_config.contract_account_class_hash,
                ),
            ]);
            AccountType::Contract
        };

        // Initialize the bytecode storage var.
        let bytecode_storage = &mut split_bytecode_to_starkfelt(code)
            .into_iter()
            .enumerate()
            .map(|(i, bytes)| {
                (
                    get_storage_var_address("bytecode_", &[StarkFelt::from(i as u32)]),
                    bytes,
                )
            })
            .collect();
        storage.append(bytecode_storage);

        // Initialize the storage vars.
        let mut evm_storage_storage: Vec<(StorageKey, StarkFelt)> = evm_storage
            .iter()
            .flat_map(|(k, v)| {
                let keys = split_u256(*k).map(Into::into);
                let values = split_u256(*v).map(Into::<StarkFelt>::into);
                let keys = get_uint256_storage_var_addresses("storage_", &keys).unwrap(); // safe unwrap: all vars are ASCII
                vec![(keys.0, values[0]), (keys.1, values[1])]
            })
            .collect();
        storage.append(&mut evm_storage_storage);

        Ok(Self {
            account_type,
            storage,
            starknet_address,
            evm_address,
            nonce: Nonce(nonce),
        })
    }
}
