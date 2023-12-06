#[allow(unused_imports)]
use blockifier::state::state_api::{
    State as BlockifierState, StateReader as BlockifierStateReader, StateResult,
};
use blockifier::{
    abi::abi_utils::get_storage_var_address,
    execution::contract_class::{ContractClass, ContractClassV0},
};
use cairo_vm::types::errors::program_errors::ProgramError;
use lazy_static::lazy_static;
use starknet::core::types::contract::legacy::LegacyContractClass;

use crate::evm_sequencer::{
    constants::kkrt_constants_v0::{
        CONTRACT_ACCOUNT_CLASS, CONTRACT_ACCOUNT_CLASS_HASH, EOA_CLASS, EOA_CLASS_HASH,
        KAKAROT_CLASS, KAKAROT_CLASS_HASH, PROXY_CLASS, PROXY_CLASS_HASH,
    },
    constants::{
        ETH_FEE_TOKEN_ADDRESS, FEE_TOKEN_CLASS, FEE_TOKEN_CLASS_HASH, KAKAROT_ADDRESS,
        KAKAROT_OWNER_ADDRESS,
    },
};
use sequencer::state::State as SequencerState;

lazy_static! {
    pub static ref INITIAL_SEQUENCER_STATE: SequencerState = {
        let mut state = SequencerState::default();

        let storage = [
            ("Ownable_owner", *KAKAROT_OWNER_ADDRESS.0.key()),
            ("native_token_address", *ETH_FEE_TOKEN_ADDRESS.0.key()),
            ("contract_account_class_hash", CONTRACT_ACCOUNT_CLASS_HASH.0),
            ("externally_owned_account_class_hash", EOA_CLASS_HASH.0),
            ("account_proxy_class_hash", PROXY_CLASS_HASH.0),
        ];

        // Write all the storage vars to the sequencer state.
        for (k, v) in storage {
            (&mut state).set_storage_at(*KAKAROT_ADDRESS, get_storage_var_address(k, &[]), v);
        }

        // Write the kakarot class and class hash.
        (&mut state).set_class_hash_at(*KAKAROT_ADDRESS, *KAKAROT_CLASS_HASH).expect("Failed to set kakarot class hash");
        (&mut state)
            .set_contract_class(&KAKAROT_CLASS_HASH, convert_contract_class(&KAKAROT_CLASS).expect("Failed to convert KAKAROT CLASS to contract class")).expect("Failed to set kakarot contract class");

        // Write proxy, eoa, contract account and erc20 classes and class hashes.
        (&mut state)
            .set_contract_class(&PROXY_CLASS_HASH, convert_contract_class(&PROXY_CLASS).expect("Failed to convert PROXY CLASS to contract class")).expect("Failed to set proxy contract class");
        (&mut state).set_contract_class(
            &CONTRACT_ACCOUNT_CLASS_HASH,
            convert_contract_class(&CONTRACT_ACCOUNT_CLASS).expect("Failed to convert CONTRACT ACCOUNT CLASS to contract class"),
        ).expect("Failed to set contract account class");
        (&mut state)
            .set_contract_class(&EOA_CLASS_HASH, convert_contract_class(&EOA_CLASS).expect("Failed to convert EOA CLASS to contract class")).expect("Failed to set eoa contract class");
        (&mut state).set_contract_class(
            &FEE_TOKEN_CLASS_HASH,
            convert_contract_class(&FEE_TOKEN_CLASS).expect("Failed to convert FEE TOKEN CLASS to contract class"),
        ).expect("Failed to set sequencer contract class");
        (&mut state).set_class_hash_at(*ETH_FEE_TOKEN_ADDRESS, *FEE_TOKEN_CLASS_HASH).expect("Failed to set fee token class hash");

        state
    };
}

fn convert_contract_class(class: &LegacyContractClass) -> Result<ContractClass, eyre::Error> {
    Result::<ContractClass, eyre::Error>::Ok(ContractClass::V0(
        ContractClassV0::try_from_json_string(
            &serde_json::to_string(class).map_err(ProgramError::Parse)?,
        )?,
    ))
}
