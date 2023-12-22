use cairo_vm::felt::Felt252;
use lazy_static::lazy_static;
use starknet_in_rust::{state::state_api::State as _, utils::get_storage_var_address};

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
            ("Ownable_owner", KAKAROT_OWNER_ADDRESS.0.clone()),
            ("native_token_address", ETH_FEE_TOKEN_ADDRESS.0.clone()),
            ("contract_account_class_hash", Felt252::from_bytes_be(CONTRACT_ACCOUNT_CLASS_HASH.to_bytes_be())),
            ("externally_owned_account_class_hash", Felt252::from_bytes_be(EOA_CLASS_HASH.to_bytes_be()) ),
            ("account_proxy_class_hash", Felt252::from_bytes_be(PROXY_CLASS_HASH.to_bytes_be()) ),
        ];

        // Write all the storage vars to the sequencer state.
        for (k, v) in storage {
            state.set_storage_at(&(KAKAROT_ADDRESS.clone(), get_storage_var_address(k, &[]).expect("Failed to compute storage address").to_be_bytes()), v);
        }

        // Write the kakarot class and class hash.
        state.set_class_hash_at(KAKAROT_ADDRESS.clone(), *KAKAROT_CLASS_HASH).expect("Failed to set kakarot class hash");
        state
            .set_contract_class(&KAKAROT_CLASS_HASH, &KAKAROT_CLASS).expect("Failed to set kakarot contract class");

        // Write proxy, eoa, contract account and erc20 classes and class hashes.
        state
            .set_contract_class(&PROXY_CLASS_HASH, &PROXY_CLASS).expect("Failed to set proxy contract class");
        state.set_contract_class(&CONTRACT_ACCOUNT_CLASS_HASH, &CONTRACT_ACCOUNT_CLASS).expect("Failed to set contract account class");
        state.set_contract_class(&EOA_CLASS_HASH, &EOA_CLASS).expect("Failed to set eoa contract class");
        state.set_contract_class(&FEE_TOKEN_CLASS_HASH, &FEE_TOKEN_CLASS).expect("Failed to set sequencer contract class");
        state.set_class_hash_at(ETH_FEE_TOKEN_ADDRESS.clone(), *FEE_TOKEN_CLASS_HASH).expect("Failed to set fee token class hash");

        state
    };
}
