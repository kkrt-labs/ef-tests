use blockifier::abi::abi_utils::get_storage_var_address;
#[allow(unused_imports)]
use blockifier::state::state_api::{
    State as BlockifierState, StateReader as BlockifierStateReader, StateResult,
};
use lazy_static::lazy_static;
use starknet_api::hash::StarkFelt;

use crate::evm_sequencer::{
    constants::{
        BLOCK_GAS_LIMIT, CONTRACT_ACCOUNT_CLASS, CONTRACT_ACCOUNT_CLASS_HASH,
        ETH_FEE_TOKEN_ADDRESS, FEE_TOKEN_CLASS, FEE_TOKEN_CLASS_HASH, KAKAROT_ADDRESS,
        KAKAROT_CLASS, KAKAROT_CLASS_HASH, KAKAROT_OWNER_ADDRESS, PRECOMPILES_CLASS,
        PRECOMPILES_CLASS_HASH, UNINITIALIZED_ACCOUNT_CLASS, UNINITIALIZED_ACCOUNT_CLASS_HASH,
    },
    sequencer::{convert_contract_class_v0, convert_contract_class_v1},
};
use sequencer::state::State as SequencerState;

lazy_static! {
    pub static ref INITIAL_SEQUENCER_STATE: SequencerState = {
        let mut state = SequencerState::default();

        let storage = [
            ("Ownable_owner", *KAKAROT_OWNER_ADDRESS.0.key()),
            ("Kakarot_native_token_address", *ETH_FEE_TOKEN_ADDRESS.0.key()),
            ("Kakarot_contract_account_class_hash", CONTRACT_ACCOUNT_CLASS_HASH.0),
            ("Kakarot_precompiles_class_hash", PRECOMPILES_CLASS_HASH.0),
            ("Kakarot_block_gas_limit", StarkFelt::from(BLOCK_GAS_LIMIT)),
            ("Kakarot_uninitialized_account_class_hash", UNINITIALIZED_ACCOUNT_CLASS_HASH.0),
        ];

        // Write all the storage vars to the sequencer state.
        for (k, v) in storage {
            (&mut state).set_storage_at(*KAKAROT_ADDRESS, get_storage_var_address(k, &[]), v).expect("failed to set storage");
        }

        // Write the kakarot class and class hash.
        (&mut state).set_class_hash_at(*KAKAROT_ADDRESS, *KAKAROT_CLASS_HASH).expect("failed to set kakarot class hash");
        (&mut state)
            .set_contract_class(*KAKAROT_CLASS_HASH, convert_contract_class_v0(&KAKAROT_CLASS).expect("failed to convert KAKAROT CLASS to contract class")).expect("failed to set kakarot contract class");

        // Write contract account, uninitialized_account and erc20 classes and class hashes.
        (&mut state).set_contract_class(
            *CONTRACT_ACCOUNT_CLASS_HASH,
            convert_contract_class_v0(&CONTRACT_ACCOUNT_CLASS).expect("failed to convert CONTRACT ACCOUNT CLASS to contract class"),
        ).expect("failed to set contract account class");
        (&mut state)
            .set_contract_class(*UNINITIALIZED_ACCOUNT_CLASS_HASH, convert_contract_class_v0(&UNINITIALIZED_ACCOUNT_CLASS).expect("failed to convert EOA CLASS to contract class")).expect("failed to set eoa contract class");

        (&mut state).set_contract_class(
            *FEE_TOKEN_CLASS_HASH,
            convert_contract_class_v0(&FEE_TOKEN_CLASS).expect("failed to convert FEE TOKEN CLASS to contract class"),
        ).expect("failed to set sequencer contract class");
        (&mut state).set_class_hash_at(*ETH_FEE_TOKEN_ADDRESS, *FEE_TOKEN_CLASS_HASH).expect("failed to set fee token class hash");
        (&mut state).set_contract_class(*PRECOMPILES_CLASS_HASH, convert_contract_class_v1(&PRECOMPILES_CLASS).expect("failed to convert PRECOMPILES Class to contract class")).expect("failed to set precompiles contract class");

        state
    };
}
