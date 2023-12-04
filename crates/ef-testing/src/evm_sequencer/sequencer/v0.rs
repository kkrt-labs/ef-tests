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
    InitializationError,
};
use sequencer::state::State as SequencerState;

use super::{InitializationResult, InitializeSequencer, KakarotSequencer};

lazy_static! {
    static ref KAKAROT_SEQUENCER: KakarotSequencer = {
        let mut sequencer = KakarotSequencer::new(SequencerState::default());

        let storage = [
            ("Ownable_owner", *KAKAROT_OWNER_ADDRESS.0.key()),
            ("native_token_address", *ETH_FEE_TOKEN_ADDRESS.0.key()),
            ("contract_account_class_hash", CONTRACT_ACCOUNT_CLASS_HASH.0),
            ("externally_owned_account_class_hash", EOA_CLASS_HASH.0),
            ("account_proxy_class_hash", PROXY_CLASS_HASH.0),
        ];

        // Write all the storage vars to the sequencer state.
        for (k, v) in storage {
            (&mut sequencer.state).set_storage_at(*KAKAROT_ADDRESS, get_storage_var_address(k, &[]), v);
        }

        // Write the kakarot class and class hash.
        (&mut sequencer.state).set_class_hash_at(*KAKAROT_ADDRESS, *KAKAROT_CLASS_HASH).expect("Failed to set sequencer class hash");
        (&mut sequencer.state)
            .set_contract_class(&KAKAROT_CLASS_HASH, convert_contract_class(&KAKAROT_CLASS).expect("Failed to convert KAKAROT CLASS to contract class")).expect("Failed to set sequencer contract class");

        // Write proxy, eoa, contract account and erc20 classes and class hashes.
        (&mut sequencer.state)
            .set_contract_class(&PROXY_CLASS_HASH, convert_contract_class(&PROXY_CLASS).expect("Failed to convert PROXY CLASS to contract class")).expect("Failed to set sequencer contract class");
        (&mut sequencer.state).set_contract_class(
            &CONTRACT_ACCOUNT_CLASS_HASH,
            convert_contract_class(&CONTRACT_ACCOUNT_CLASS).expect("Failed to convert CONTRACT ACCOUNT CLASS to contract class"),
        ).expect("Failed to set sequencer contract class");
        (&mut sequencer.state)
            .set_contract_class(&EOA_CLASS_HASH, convert_contract_class(&EOA_CLASS).expect("Failed to convert EOA CLASS to contract class")).expect("Failed to set sequencer contract class");
        (&mut sequencer.state).set_contract_class(
            &FEE_TOKEN_CLASS_HASH,
            convert_contract_class(&FEE_TOKEN_CLASS).expect("Failed to set FEE TOKEN CLASS to contract class"),
        ).expect("Failed to set sequencer contract class");
        (&mut sequencer.state).set_class_hash_at(*ETH_FEE_TOKEN_ADDRESS, *FEE_TOKEN_CLASS_HASH).expect("Failed to set fee token class hash");

        sequencer
    };
}

impl InitializeSequencer for KakarotSequencer {
    fn initialize(mut self) -> InitializationResult<Self> {
        Ok(KAKAROT_SEQUENCER.clone())
    }
}

fn convert_contract_class(
    class: &LegacyContractClass,
) -> Result<ContractClass, InitializationError> {
    Result::<ContractClass, InitializationError>::Ok(ContractClass::V0(
        ContractClassV0::try_from_json_string(
            &serde_json::to_string(class).map_err(ProgramError::Parse)?,
        )?,
    ))
}
