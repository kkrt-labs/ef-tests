use std::{collections::HashMap, sync::Arc};

use blockifier::block_context::BlockContext;
use lazy_static::lazy_static;
use starknet::core::types::{contract::legacy::LegacyContractClass, FieldElement};
use starknet_api::{
    block::{BlockNumber, BlockTimestamp},
    core::{ChainId, ClassHash, ContractAddress, PatriciaKey},
    hash::StarkFelt,
};

lazy_static! {
    // Chain params
    pub static ref CHAIN_ID: u64 = 0x4b4b5254;

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

    // Block context
    pub static ref BLOCK_CONTEXT: BlockContext = BlockContext {
            chain_id: ChainId(String::from_utf8(CHAIN_ID.to_be_bytes().to_vec()).unwrap()),
            block_number: BlockNumber(0),
            block_timestamp: BlockTimestamp(0),
            sequencer_address: *SEQUENCER_ADDRESS,
            fee_token_address: *FEE_TOKEN_ADDRESS,
            vm_resource_fee_cost: Arc::new(VM_RESOURCES.clone()),
            gas_price: 1,
            invoke_tx_max_n_steps: 2_u32.pow(24),
            validate_max_n_steps: 2_u32.pow(24),
            max_recursion_depth: 1024,
        };

    // Main addresses
    pub static ref SEQUENCER_ADDRESS: ContractAddress = ContractAddress(
        TryInto::<PatriciaKey>::try_into(StarkFelt::from(
            FieldElement::from_hex_be(
                "0x01176a1bd84444c89232ec27754698e5d2e7e1a7f1539f12027f28b23ec9f3d8"
            )
            .unwrap()
        ))
        .unwrap()
    );
    pub static ref FEE_TOKEN_ADDRESS: ContractAddress = ContractAddress(
        TryInto::<PatriciaKey>::try_into(StarkFelt::from(
            FieldElement::from_hex_be(
                "0x049D36570D4e46f48e99674bd3fcc84644DdD6b96F7C741B1562B82f9e004dC7"
            )
            .unwrap()
        ))
        .unwrap()
    );
    pub static ref KAKAROT_ADDRESS: ContractAddress =
        ContractAddress(TryInto::<PatriciaKey>::try_into(StarkFelt::from(1_u8)).unwrap());
    pub static ref KAKAROT_OWNER_ADDRESS: ContractAddress =
        ContractAddress(TryInto::<PatriciaKey>::try_into(StarkFelt::from(2_u8)).unwrap());

    // Main contract classes
    pub static ref KAKAROT_CLASS: LegacyContractClass = serde_json::from_reader::<_, LegacyContractClass>(std::fs::File::open("../../lib/kakarot/build/kakarot.json").unwrap()).unwrap();
    pub static ref CONTRACT_ACCOUNT_CLASS: LegacyContractClass = serde_json::from_reader::<_, LegacyContractClass>(std::fs::File::open("../../lib/kakarot/build/contract_account.json").unwrap()).unwrap();
    pub static ref EOA_CLASS: LegacyContractClass = serde_json::from_reader::<_, LegacyContractClass>(std::fs::File::open("../../lib/kakarot/build/externally_owned_account.json").unwrap()).unwrap();
    pub static ref PROXY_CLASS: LegacyContractClass = serde_json::from_reader::<_, LegacyContractClass>(std::fs::File::open("../../lib/kakarot/build/proxy.json").unwrap()).unwrap();

    // Main class hashes
    pub static ref KAKAROT_CLASS_HASH: ClassHash = ClassHash(KAKAROT_CLASS.class_hash().unwrap().into());
    pub static ref CONTRACT_ACCOUNT_CLASS_HASH: ClassHash = ClassHash(CONTRACT_ACCOUNT_CLASS.class_hash().unwrap().into());
    pub static ref EOA_CLASS_HASH: ClassHash = ClassHash(EOA_CLASS.class_hash().unwrap().into());
    pub static ref PROXY_CLASS_HASH: ClassHash = ClassHash(PROXY_CLASS.class_hash().unwrap().into());

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
