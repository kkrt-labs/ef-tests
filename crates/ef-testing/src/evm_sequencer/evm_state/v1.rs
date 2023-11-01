use blockifier::state::state_api::StateResult;
use reth_primitives::{Address, Bytes};
use revm_primitives::U256;
use starknet::core::utils::get_contract_address;
use starknet_api::{core::ClassHash, hash::StarkFelt, StarknetApiError, state::StorageKey};

use super::EvmState;
use crate::evm_sequencer::{
    constants::KAKAROT_ADDRESS, sequencer::KakarotSequencer, types::felt::FeltSequencer,
};

impl EvmState for KakarotSequencer {
    fn setup_account(
        &mut self,
        evm_address: &Address,
        bytecode: &Bytes,
        nonce: U256,
        storage: Vec<(U256, U256)>,
    ) -> StateResult<()> {
        let nonce = StarkFelt::from(TryInto::<u128>::try_into(nonce).map_err(|err| {
            StarknetApiError::OutOfRange {
                string: err.to_string(),
            }
        })?);
        let evm_address = TryInto::<FeltSequencer>::try_into(*evm_address)
            .unwrap()
            .into(); // infallible

        let mut storage = vec![
            (("kakarot_core_address", []), KAKAROT_ADDRESS.0.key()),
            (("evm_address", []), evm_address),
        ];

        // Set the rest of the storage based on the content of the bytecode or the storage.
        let class_hash = if bytecode.is_empty() && storage.is_empty() {
            
        }

        Ok(())

        // be careful to use poseidon hash here
        // run uninitialized constructor
        // set kakarot core address and evm address
        // also set the address registry
        // call initialize
        // - set correct class hash (contract or eoa)
        // - if contract, set code and nonce = 1
        // - bytecode:
    }
}

/// Computes the Starknet address of a contract given its EVM address.
 fn compute_starknet_address(evm_address: &Address, class_hash: ClassHash) -> FeltSequencer {
    let evm_address: FeltSequencer = (*evm_address).try_into().unwrap(); // infallible
    let starknet_address = get_contract_address(
        evm_address.into(),
        class_hash.0.into(),
        &[KAKAROT_ADDRESS.0.key(), evm_address.into()],
        (*KAKAROT_ADDRESS.0.key()).into(),
    );
    starknet_address.into()
}

// fn compute_storage_base_address()

fn compute_poseidon_hash_on_elements() -> StarkFelt {

}
