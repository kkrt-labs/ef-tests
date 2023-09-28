pub mod constants;
pub mod setup;
pub mod types;
pub mod utils;

use blockifier::execution::contract_class::{ContractClass, ContractClassV0};
use blockifier::state::errors::StateError;
use blockifier::state::state_api::{State as BlockifierState, StateResult};
use cairo_vm::types::errors::program_errors::ProgramError;
use reth_primitives::Address;
use sequencer::sequencer::Sequencer;
use sequencer::state::State;
use starknet::core::utils::get_contract_address;

use self::constants::{
    BLOCK_CONTEXT, CONTRACT_ACCOUNT_CLASS, CONTRACT_ACCOUNT_CLASS_HASH, EOA_CLASS, EOA_CLASS_HASH,
    FEE_TOKEN_ADDRESS, KAKAROT_ADDRESS, KAKAROT_CLASS, KAKAROT_CLASS_HASH, KAKAROT_OWNER_ADDRESS,
    PROXY_CLASS, PROXY_CLASS_HASH,
};
use self::types::FeltSequencer;
use self::utils::{
    class_hash_to_starkfelt, contract_address_to_starkfelt, get_storage_var_address,
};

pub(crate) struct KakarotSequencer(Sequencer<State>);

#[allow(dead_code)]
impl KakarotSequencer {
    pub fn new(state: State) -> Self {
        let sequencer = Sequencer::new(BLOCK_CONTEXT.clone(), state);
        Self(sequencer)
    }

    pub fn initialize(mut self) -> StateResult<Self> {
        let mut storage = vec![];

        // Initialize the kakarot owner.
        let kakarot_owner_storage = (
            get_storage_var_address("Ownable_owner", &[]).unwrap(), // safe unwrap: var is ASCII
            contract_address_to_starkfelt(&KAKAROT_OWNER_ADDRESS),
        );
        storage.push(kakarot_owner_storage);

        // Initialize the kakarot fee token address.
        let kakarot_fee_token_storage = (
            get_storage_var_address("native_token_address", &[]).unwrap(), // safe unwrap: var is ASCII
            contract_address_to_starkfelt(&FEE_TOKEN_ADDRESS),
        );
        storage.push(kakarot_fee_token_storage);

        // Initialize the kakarot various class hashes.
        let kakarot_class_hashes = &mut vec![
            (
                get_storage_var_address("contract_account_class_hash", &[]).unwrap(),
                class_hash_to_starkfelt(&CONTRACT_ACCOUNT_CLASS_HASH),
            ),
            (
                get_storage_var_address("externally_owned_account_class_hash", &[]).unwrap(),
                class_hash_to_starkfelt(&EOA_CLASS_HASH),
            ),
            (
                get_storage_var_address("account_proxy_class_hash", &[]).unwrap(),
                class_hash_to_starkfelt(&PROXY_CLASS_HASH),
            ),
        ];
        storage.append(kakarot_class_hashes);

        // Write all the storage vars to the sequencer state.
        for (k, v) in storage {
            (&mut self.0.state).set_storage_at(*KAKAROT_ADDRESS, k, v);
        }

        // Write the kakarot class and class hash.
        (&mut self.0.state).set_class_hash_at(*KAKAROT_ADDRESS, *KAKAROT_CLASS_HASH)?;
        (&mut self.0.state).set_contract_class(
            &KAKAROT_CLASS_HASH,
            ContractClass::V0(ContractClassV0::try_from_json_string(
                &serde_json::to_string(&*KAKAROT_CLASS)
                    .map_err(|err| StateError::ProgramError(ProgramError::Parse(err)))?,
            )?),
        )?;

        // Write proxy, eoa and contract account classes and class hashes.
        (&mut self.0.state).set_contract_class(
            &PROXY_CLASS_HASH,
            ContractClass::V0(ContractClassV0::try_from_json_string(
                &serde_json::to_string(&*PROXY_CLASS)
                    .map_err(|err| StateError::ProgramError(ProgramError::Parse(err)))?,
            )?),
        )?;
        (&mut self.0.state).set_contract_class(
            &CONTRACT_ACCOUNT_CLASS_HASH,
            ContractClass::V0(ContractClassV0::try_from_json_string(
                &serde_json::to_string(&*CONTRACT_ACCOUNT_CLASS)
                    .map_err(|err| StateError::ProgramError(ProgramError::Parse(err)))?,
            )?),
        )?;
        (&mut self.0.state).set_contract_class(
            &EOA_CLASS_HASH,
            ContractClass::V0(ContractClassV0::try_from_json_string(
                &serde_json::to_string(&*EOA_CLASS)
                    .map_err(|err| StateError::ProgramError(ProgramError::Parse(err)))?,
            )?),
        )?;

        Ok(self)
    }

    pub fn compute_starknet_address(&self, evm_address: &Address) -> FeltSequencer {
        let evm_address: FeltSequencer = (*evm_address).into();
        let starknet_address = get_contract_address(
            evm_address.into(),
            PROXY_CLASS_HASH.0.into(),
            &[],
            (*KAKAROT_ADDRESS.0.key()).into(),
        );
        starknet_address.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initialize() {
        // Given
        let state = State::default();
        let sequencer = KakarotSequencer::new(state);

        // When
        let result = sequencer.initialize();

        // Then
        assert!(result.is_ok());
    }
}
