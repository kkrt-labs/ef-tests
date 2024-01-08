use std::collections::HashMap;
use std::sync::Arc;

use crate::evm_sequencer::utils::compute_starknet_address;
use cairo_vm::felt::Felt252;
use lazy_static::lazy_static;
use num_traits::One;
use reth_primitives::Address;
use starknet::core::types::contract::legacy::LegacyContractClass;
use starknet_in_rust::definitions::block_context::{BlockContext, StarknetOsConfig};
use starknet_in_rust::felt::felt_str;
use starknet_in_rust::services::api::contract_classes::compiled_class::CompiledClass;
use starknet_in_rust::services::api::contract_classes::deprecated_contract_class::ContractClass;
use starknet_in_rust::state::BlockInfo;
use starknet_in_rust::utils::{Address as StarknetAddress, ClassHash};

fn read_contract_class_v0(path: &str) -> CompiledClass {
    let s = std::fs::read_to_string(path).expect("Failed to read v0 contract class");
    let legacy_contract_class = serde_json::from_str::<LegacyContractClass>(&s).unwrap();
    let class_hash = legacy_contract_class
        .class_hash()
        .expect("Failed to get class hash");

    let contract_class = ContractClass::from_program_json_and_class_hash(
        &s,
        Felt252::from_bytes_be(&class_hash.to_bytes_be()[..]),
    )
    .expect("Failed to get contract class");
    CompiledClass::Deprecated(Arc::new(contract_class))
}

lazy_static! {
    // Chain params
    pub static ref CHAIN_ID: u64 = 0x4b4b5254;
    pub static ref COINBASE_ADDRESS: Address = Address::from_low_u64_be(0xDEAD2BAD);

    // Vm resources: maps resource name to fee cost.
    pub static ref VM_RESOURCES: HashMap<String, f64> = [
        ("n_steps".to_string(), 1_f64),
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

    // StarknetOsConfig
    pub static ref STARKNET_OS_CONFIG: StarknetOsConfig = StarknetOsConfig::new(Felt252::from(*CHAIN_ID), ETH_FEE_TOKEN_ADDRESS.clone(), 1);

    // BlockInfo
    pub static ref BLOCK_INFO: BlockInfo = BlockInfo {
        block_number:0,
        block_timestamp:0,
        gas_price:1,
        sequencer_address: SEQUENCER_ADDRESS.clone(),
    };

    // Block context
    pub static ref BLOCK_CONTEXT: BlockContext = BlockContext::new(STARKNET_OS_CONFIG.clone(), 10, 10,VM_RESOURCES.clone(), 20_000_000, 20_000_000, BLOCK_INFO.clone(), HashMap::new(), false);

    // Main addresses
    pub static ref SEQUENCER_ADDRESS: StarknetAddress = compute_starknet_address(&COINBASE_ADDRESS);
    pub static ref ETH_FEE_TOKEN_ADDRESS: StarknetAddress = StarknetAddress(felt_str!("049D36570D4e46f48e99674bd3fcc84644DdD6b96F7C741B1562B82f9e004dC7", 16));
    pub static ref KAKAROT_ADDRESS: StarknetAddress = StarknetAddress(Felt252::one());
    pub static ref KAKAROT_OWNER_ADDRESS: StarknetAddress = StarknetAddress(Felt252::from(2_u8));

    pub static ref FEE_TOKEN_CLASS: CompiledClass = read_contract_class_v0("../../build/common/ERC20.json");
    pub static ref FEE_TOKEN_CLASS_HASH: ClassHash = TryInto::<ContractClass>::try_into(FEE_TOKEN_CLASS.clone()).expect("Failed to convert token class hash").hinted_class_hash().clone().into();

}

#[cfg(feature = "v0")]
pub mod kkrt_constants_v0 {
    use super::*;
    use starknet_in_rust::services::api::contract_classes::compiled_class::CompiledClass;

    lazy_static! {
        // Main contract classes v0
        pub static ref KAKAROT_CLASS: CompiledClass = read_contract_class_v0("../../build/v0/kakarot.json");
        pub static ref CONTRACT_ACCOUNT_CLASS: CompiledClass = read_contract_class_v0("../../build/v0/contract_account.json");
        pub static ref EOA_CLASS: CompiledClass = read_contract_class_v0("../../build/v0/externally_owned_account.json");
        pub static ref PROXY_CLASS: CompiledClass = read_contract_class_v0("../../build/v0/proxy.json");

        // Main class hashes
        pub static ref KAKAROT_CLASS_HASH: ClassHash = TryInto::<ContractClass>::try_into(KAKAROT_CLASS.clone()).expect("Failed to convert Kakarot class hash").hinted_class_hash().clone().into();
        pub static ref CONTRACT_ACCOUNT_CLASS_HASH: ClassHash = TryInto::<ContractClass>::try_into(CONTRACT_ACCOUNT_CLASS.clone()).expect("Failed to convert contract account class hash").hinted_class_hash().clone().into();
        pub static ref EOA_CLASS_HASH: ClassHash = TryInto::<ContractClass>::try_into(EOA_CLASS.clone()).expect("Failed to convert eoa class hash").hinted_class_hash().clone().into();
        pub static ref PROXY_CLASS_HASH: ClassHash = TryInto::<ContractClass>::try_into(PROXY_CLASS.clone()).expect("Failed to convert proxy class hash").hinted_class_hash().clone().into();
    }
}

#[cfg(feature = "v1")]
pub mod kkrt_constants_v1 {
    use super::*;
    use cairo_lang_starknet::casm_contract_class::CasmContractEntryPoint;
    use cairo_lang_starknet::contract_class::ContractClass as SierraContractClass;
    use starknet_crypto::{poseidon_hash_many, FieldElement};
    use starknet_in_rust::{
        services::api::contract_classes::compiled_class::CompiledClass, CasmContractClass,
    };
    use std::sync::Arc;

    fn load_contract_class(path: &str) -> Result<(CompiledClass, ClassHash), eyre::Error> {
        let s = std::fs::read_to_string(path).expect("Failed to read native contract class");
        let contract_class = serde_json::from_str::<SierraContractClass>(&s)
            .expect("Failed to parse contract class");

        let casm_contract_class = CasmContractClass::from_contract_class(contract_class, true)?;
        let class_hash = calculate_class_hash(&casm_contract_class);
        {
            #[cfg(not(feature = "native"))]
            {
                Ok((
                    CompiledClass::Casm(Arc::new(casm_contract_class)),
                    class_hash.into(),
                ))
            }
            #[cfg(feature = "native")]
            {
                let sierra_program = contract_class.extract_sierra_program()?;
                let entrypoints = contract_class.entry_points_by_type;

                Ok((
                    CompiledClass::Sierra(Arc::new((sierra_program, entrypoints))),
                    class_hash.into(),
                ))
            }
        }
    }

    fn calculate_class_hash(casm_contract_class: &CasmContractClass) -> Felt252 {
        // Compute hashes on each component separately.
        let external_funcs_hash =
            entry_points_hash(&casm_contract_class.entry_points_by_type.external);
        let l1_handlers_hash =
            entry_points_hash(&casm_contract_class.entry_points_by_type.l1_handler);
        let constructors_hash =
            entry_points_hash(&casm_contract_class.entry_points_by_type.constructor);
        let bytecode_hash = poseidon_hash_many(
            &casm_contract_class
                .bytecode
                .iter()
                .map(|big_uint| {
                    FieldElement::from_byte_slice_be(&big_uint.value.to_bytes_be()).unwrap()
                })
                .collect::<Vec<_>>(),
        );

        // Compute total hash by hashing each component on top of the previous one.
        Felt252::from_bytes_be(
            &poseidon_hash_many(&[
                FieldElement::from_byte_slice_be(b"COMPILED_CLASS_V1").unwrap(),
                external_funcs_hash,
                l1_handlers_hash,
                constructors_hash,
                bytecode_hash,
            ])
            .to_bytes_be(),
        )
    }

    fn entry_points_hash(entry_points: &[CasmContractEntryPoint]) -> FieldElement {
        let mut entry_point_hash_elements = vec![];
        for entry_point in entry_points {
            entry_point_hash_elements.push(
                FieldElement::from_byte_slice_be(&entry_point.selector.to_bytes_be()).unwrap(),
            );
            entry_point_hash_elements.push(FieldElement::from(entry_point.offset));
            entry_point_hash_elements.push(poseidon_hash_many(
                &entry_point
                    .builtins
                    .iter()
                    .map(|builtin| FieldElement::from_byte_slice_be(builtin.as_bytes()).unwrap())
                    .collect::<Vec<_>>(),
            ));
        }
        poseidon_hash_many(&entry_point_hash_elements)
    }

    lazy_static! {
        static ref KAKAROT_CLASS_AND_HASH: (CompiledClass, ClassHash) = load_contract_class("../../build/v1/kakarot.json").expect("Failed to load Kakarot contract class");
        static ref CONTRACT_ACCOUNT_CLASS_AND_HASH: (CompiledClass, ClassHash) = load_contract_class("../../build/v1/contract_account.json").expect("Failed to load ContractAccount contract class");
        static ref EOA_CLASS_AND_HASH: (CompiledClass, ClassHash) = load_contract_class("../../build/v1/externally_owned_account.json").expect("Failed to load EOA contract class");
        static ref UNINITIALIZED_ACCOUNT_CLASS_AND_HASH: (CompiledClass, ClassHash) = load_contract_class("../../build/v1/uninitialized_account.json").expect("Failed to load uninitialized account contract class");

        // Main contract classes v1
        pub static ref KAKAROT_CLASS: CompiledClass = KAKAROT_CLASS_AND_HASH.0.clone();
        pub static ref CONTRACT_ACCOUNT_CLASS: CompiledClass = CONTRACT_ACCOUNT_CLASS_AND_HASH.0.clone();
        pub static ref EOA_CLASS: CompiledClass = EOA_CLASS_AND_HASH.0.clone();
        pub static ref UNINITIALIZED_ACCOUNT_CLASS: CompiledClass = UNINITIALIZED_ACCOUNT_CLASS_AND_HASH.0.clone();

        // Main class hashes
        pub static ref KAKAROT_CLASS_HASH: ClassHash = KAKAROT_CLASS_AND_HASH.1;
        pub static ref CONTRACT_ACCOUNT_CLASS_HASH: ClassHash = CONTRACT_ACCOUNT_CLASS_AND_HASH.1;
        pub static ref EOA_CLASS_HASH: ClassHash = EOA_CLASS_AND_HASH.1;
        pub static ref UNINITIALIZED_ACCOUNT_CLASS_HASH: ClassHash = UNINITIALIZED_ACCOUNT_CLASS_AND_HASH.1;

        // v1 constants
        pub static ref DEPLOY_FEE: Felt252 = Felt252::from(0xabde1_u128);
    }
}

#[cfg(test)]
pub mod tests {
    use std::str::FromStr;

    use lazy_static::lazy_static;
    use reth_primitives::Address;
    use revm_primitives::B256;

    lazy_static! {
        pub static ref PRIVATE_KEY: B256 =
            B256::from_str("0x6ae82d865482a203603ecbf25c865e082396d7705a6bbce92c1ff1d6ab9b503c")
                .unwrap();
        pub static ref PUBLIC_KEY: Address =
            Address::from_str("0x7513A12F74fFF533ee12F20EE524e4883CBd1945").unwrap();
        pub static ref TEST_CONTRACT_ADDRESS: Address = Address::from_low_u64_be(10);
    }
}
