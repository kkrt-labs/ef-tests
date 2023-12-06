use blockifier::{
    abi::abi_utils::get_storage_var_address,
    execution::contract_class::{ContractClass, ContractClassV0, ContractClassV1},
    state::state_api::State as BlockifierState,
};
use cairo_lang_starknet::casm_contract_class::CasmContractClass;
use cairo_vm::types::errors::program_errors::ProgramError;
use lazy_static::lazy_static;
use starknet::core::types::contract::{legacy::LegacyContractClass, CompiledClass};
use starknet_api::hash::StarkFelt;

use crate::evm_sequencer::{
    constants::{
        kkrt_constants_v1::{
            CONTRACT_ACCOUNT_CLASS, CONTRACT_ACCOUNT_CLASS_HASH, DEPLOY_FEE, EOA_CLASS,
            EOA_CLASS_HASH, KAKAROT_CLASS, KAKAROT_CLASS_HASH, UNINITIALIZED_ACCOUNT_CLASS,
            UNINITIALIZED_ACCOUNT_CLASS_HASH,
        },
        CHAIN_ID, ETH_FEE_TOKEN_ADDRESS, FEE_TOKEN_CLASS, FEE_TOKEN_CLASS_HASH, KAKAROT_ADDRESS,
        KAKAROT_OWNER_ADDRESS,
    },
    types::contract_class::CasmContractClassWrapper,
};
use sequencer::state::State as SequencerState;

lazy_static! {
    pub static ref INITIAL_SEQUENCER_STATE: SequencerState = {
        let mut state = SequencerState::default();

        let storage = [
            ("owner", *KAKAROT_OWNER_ADDRESS.0.key()),
            ("chain_id", StarkFelt::from(*CHAIN_ID)),
            ("native_token", *ETH_FEE_TOKEN_ADDRESS.0.key()),
            ("deploy_fee", *DEPLOY_FEE),
            ("ca_class_hash", CONTRACT_ACCOUNT_CLASS_HASH.0),
            ("eoa_class_hash", EOA_CLASS_HASH.0),
            ("account_class_hash", UNINITIALIZED_ACCOUNT_CLASS_HASH.0),
        ];

        // Write all the storage vars to the sequencer state.
        for (k, v) in storage {
            (&mut state).set_storage_at(*KAKAROT_ADDRESS, get_storage_var_address(k, &[]), v);
        }

        // Write the kakarot class and class hash.
        (&mut state).set_class_hash_at(*KAKAROT_ADDRESS, *KAKAROT_CLASS_HASH).expect("Failed to set kakarot class hash");
        (&mut state)
            .set_contract_class(&KAKAROT_CLASS_HASH, convert_contract_class(&KAKAROT_CLASS).expect("Failed to convert KAKAROT CLASS to contract class")).expect("Failed to set kakarot contract class");

        // Write eoa, contract account and uninitialized account.
        (&mut state).set_contract_class(
            &CONTRACT_ACCOUNT_CLASS_HASH,
            convert_contract_class(&CONTRACT_ACCOUNT_CLASS).expect("Failed to convert CONTRACT ACCOUNT CLASS to contract class"),
        ).expect("Failed to set contract account class");
        (&mut state)
            .set_contract_class(&EOA_CLASS_HASH, convert_contract_class(&EOA_CLASS).expect("Failed to convert EOA CLASS to contract class")).expect("Failed to set eoa contract class");
        (&mut state).set_contract_class(
            &UNINITIALIZED_ACCOUNT_CLASS_HASH,
            convert_contract_class(&UNINITIALIZED_ACCOUNT_CLASS).expect("Failed to convert UNINITIALIZED ACCOUNT CLASS to contract class"),
        ).expect("Failed to set uninitialized account class");

        let convert_class = |class: &LegacyContractClass| -> Result<ContractClass, eyre::Error> {
            Ok(ContractClass::V0(ContractClassV0::try_from_json_string(
                &serde_json::to_string(&class).map_err(ProgramError::Parse)?,
            )?))
        };
        (&mut state).set_contract_class(
            &FEE_TOKEN_CLASS_HASH,
            convert_class(&FEE_TOKEN_CLASS).expect("Failed to convert FEE TOKEN CLASS to contract class"),
        ).expect("Failed to set fee token contract class");
        (&mut state).set_class_hash_at(*ETH_FEE_TOKEN_ADDRESS, *FEE_TOKEN_CLASS_HASH).expect("Failed to set fee token class hash");

        state
    };
}

fn convert_contract_class(class: &CompiledClass) -> Result<ContractClass, eyre::Error> {
    let casm_contract_class = CasmContractClassWrapper::try_from(class)?;
    let casm_contract_class: CasmContractClass = casm_contract_class.into();
    Ok(ContractClass::V1(ContractClassV1::try_from(
        casm_contract_class,
    )?))
}
