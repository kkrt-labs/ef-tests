use std::collections::HashMap;

use lazy_static::lazy_static;
use reth_primitives::alloy_primitives::{address, Address};
use serde::de::DeserializeOwned;
use starknet::core::types::contract::legacy::LegacyContractClass;
use starknet::core::types::contract::CompiledClass;
use starknet_api::{
    contract_address,
    core::{ClassHash, ContractAddress, PatriciaKey},
    hash::{StarkFelt, StarkHash},
    patricia_key,
};

fn load_contract_class<T>(path: &str) -> Result<T, eyre::Error>
where
    T: DeserializeOwned,
{
    serde_json::from_str::<T>(&std::fs::read_to_string(path)?).map_err(eyre::Error::from)
}

// Chain params
pub const CHAIN_ID: u64 = 0x4b4b5254;

// Block params
pub const BLOCK_GAS_LIMIT: u64 = 20_000_000;

lazy_static! {
    // Vm resources: maps resource name to fee cost.
    pub static ref VM_RESOURCES: HashMap<String, f64> = [
        (String::from("n_steps"), 1_f64),
        ("pedersen_builtin".to_string(), 1_f64),
        ("range_check_builtin".to_string(), 1_f64),
        ("ecdsa_builtin".to_string(), 1_f64),
        ("bitwise_builtin".to_string(), 1_f64),
        ("poseidon_builtin".to_string(), 1_f64),
        ("output_builtin".to_string(), 1_f64),
        ("ec_op_builtin".to_string(), 1_f64),
        ("keccak_builtin".to_string(), 1_f64),
        ("segment_arena_builtin".to_string(), 1_f64),
        ]
        .into_iter()
        .collect();

    // EVM Addresses
    pub static ref BEACON_ROOT_ADDRESS: Address = address!("000f3df6d732807ef1319fb7b8bb8522d0beac02");

    // Main addresses
    pub static ref ETH_FEE_TOKEN_ADDRESS: ContractAddress = contract_address!("0x049D36570D4e46f48e99674bd3fcc84644DdD6b96F7C741B1562B82f9e004dC7");
    pub static ref STRK_FEE_TOKEN_ADDRESS: ContractAddress = contract_address!("0xCa14007Eff0dB1f8135f4C25B34De49AB0d42766");
    pub static ref KAKAROT_ADDRESS: ContractAddress =
        ContractAddress(TryInto::<PatriciaKey>::try_into(StarkFelt::from(1_u8)).unwrap());
    pub static ref KAKAROT_OWNER_ADDRESS: ContractAddress =
        ContractAddress(TryInto::<PatriciaKey>::try_into(StarkFelt::from(2_u8)).unwrap());

    pub static ref FEE_TOKEN_CLASS: LegacyContractClass = load_contract_class("../../build/common/ERC20.json").expect("Failed to load FeeToken contract class");
    pub static ref FEE_TOKEN_CLASS_HASH: ClassHash = ClassHash(FEE_TOKEN_CLASS.class_hash().unwrap().into());

    pub static ref CAIRO1_HELPERS_CLASS: CompiledClass = load_contract_class("../../build/common/cairo1_helpers.json").expect("Failed to load precompiles contract class");
    pub static ref CAIRO1_HELPERS_CLASS_HASH: ClassHash = ClassHash(CAIRO1_HELPERS_CLASS.class_hash().unwrap().into());

}

#[cfg(feature = "v0")]
lazy_static! {
    // Main contract classes v0
    pub static ref KAKAROT_CLASS: LegacyContractClass = load_contract_class("../../build/v0/kakarot.json").expect("Failed to load Kakarot contract class");
    pub static ref ACCOUNT_CONTRACT_CLASS: LegacyContractClass = load_contract_class("../../build/v0/account_contract.json").expect("Failed to load ContractAccount contract class");
    pub static ref UNINITIALIZED_ACCOUNT_CLASS: LegacyContractClass = load_contract_class("../../build/v0/uninitialized_account.json").expect("Failed to load uninitialized account c contract class");

    // Main class hashes
    pub static ref KAKAROT_CLASS_HASH: ClassHash = ClassHash(KAKAROT_CLASS.class_hash().unwrap().into());
    pub static ref ACCOUNT_CONTRACT_CLASS_HASH: ClassHash = ClassHash(ACCOUNT_CONTRACT_CLASS.class_hash().unwrap().into());
    pub static ref UNINITIALIZED_ACCOUNT_CLASS_HASH: ClassHash = ClassHash(UNINITIALIZED_ACCOUNT_CLASS.class_hash().unwrap().into());
}

#[cfg(feature = "v1")]
lazy_static! {
    // Main contract classes v1
    pub static ref KAKAROT_CLASS: CompiledClass = load_contract_class("../../build/v1/contracts_KakarotCore.compiled_contract_class.json").expect("Failed to load Kakarot contract class");
    pub static ref ACCOUNT_CONTRACT_CLASS: CompiledClass = load_contract_class("../../build/v1/contracts_AccountContract.compiled_contract_class.json").expect("Failed to load ContractAccount contract class");
    pub static ref UNINITIALIZED_ACCOUNT_CLASS: CompiledClass = load_contract_class("../../build/v1/contracts_UninitializedAccount.compiled_contract_class.json").expect("Failed to load uninitialized account c contract class");

    // Main class hashes
    pub static ref KAKAROT_CLASS_HASH: ClassHash = ClassHash(KAKAROT_CLASS.class_hash().unwrap().into());
    pub static ref ACCOUNT_CONTRACT_CLASS_HASH: ClassHash = ClassHash(ACCOUNT_CONTRACT_CLASS.class_hash().unwrap().into());
    pub static ref UNINITIALIZED_ACCOUNT_CLASS_HASH: ClassHash = ClassHash(UNINITIALIZED_ACCOUNT_CLASS.class_hash().unwrap().into());
    pub static ref PROXY_CLASS_HASH: ClassHash = *UNINITIALIZED_ACCOUNT_CLASS_HASH;
}

#[cfg(not(any(feature = "v0", feature = "v1")))]
lazy_static! {
    pub static ref ACCOUNT_CONTRACT_CLASS_HASH: ClassHash =
        panic!("Contract account class hash not defined, use features flag \"v0\" or \"v1\"");
    pub static ref EOA_CLASS_HASH: ClassHash =
        panic!("EOA class hash not defined, use features flag \"v0\" or \"v1\"");
    pub static ref PROXY_CLASS_HASH: ClassHash =
        panic!("Proxy class hash not defined, use features flag \"v0\" or \"v1\"");
    pub static ref UNINITIALIZED_ACCOUNT_CLASS_HASH: ClassHash =
        panic!("Uninitialized account class hash not defined, use features flag \"v0\" or \"v1\"");
}

pub mod storage_variables {
    pub const ACCOUNT_BYTECODE: &str = "Account_bytecode";
    pub const ACCOUNT_BYTECODE_LEN: &str = "Account_bytecode_len";
    pub const ACCOUNT_STORAGE: &str = "Account_storage";
    pub const ACCOUNT_IS_INITIALIZED: &str = "Account_is_initialized";
    pub const ACCOUNT_EVM_ADDRESS: &str = "Account_evm_address";
    pub const ACCOUNT_NONCE: &str = "Account_nonce";
    pub const ACCOUNT_KAKAROT_ADDRESS: &str = "Account_kakarot_address";
    pub const ACCOUNT_IMPLEMENTATION: &str = "Account_implementation";
    pub const ACCOUNT_CAIRO1_HELPERS_CLASS: &str = "Account_cairo1_helpers_class_hash";
    pub const ACCOUNT_VALID_JUMPDESTS : &str = "Account_valid_jumpdests";

    pub const KAKAROT_COINBASE: &str = "Kakarot_coinbase";
    pub const KAKAROT_BASE_FEE: &str = "Kakarot_base_fee";
    pub const KAKAROT_BLOCK_GAS_LIMIT: &str = "Kakarot_block_gas_limit";
    pub const KAKAROT_EVM_TO_STARKNET_ADDRESS: &str = "Kakarot_evm_to_starknet_address";
    pub const KAKAROT_NATIVE_TOKEN_ADDRESS: &str = "Kakarot_native_token_address";
    pub const KAKAROT_UNINITIALIZED_ACCOUNT_CLASS_HASH: &str =
        "Kakarot_uninitialized_account_class_hash";
    pub const KAKAROT_CAIRO1_HELPERS_CLASS_HASH: &str = "Kakarot_cairo1_helpers_class_hash";
    pub const KAKAROT_ACCOUNT_CONTRACT_CLASS_HASH: &str = "Kakarot_account_contract_class_hash";
    pub const KAKAROT_PREV_RANDAO: &str = "Kakarot_prev_randao";

    pub const OWNABLE_OWNER: &str = "Ownable_owner";
}

#[cfg(test)]
pub mod tests {
    use std::str::FromStr;

    use lazy_static::lazy_static;
    use reth_primitives::{Address, B256};

    lazy_static! {
        pub static ref PRIVATE_KEY: B256 =
            B256::from_str("0x6ae82d865482a203603ecbf25c865e082396d7705a6bbce92c1ff1d6ab9b503c")
                .unwrap();
        pub static ref PUBLIC_KEY: Address =
            Address::from_str("0x7513A12F74fFF533ee12F20EE524e4883CBd1945").unwrap();
        pub static ref TEST_CONTRACT_ADDRESS: Address =
            Address::left_padding_from(&0xdeadbeefu64.to_be_bytes());
    }
}
