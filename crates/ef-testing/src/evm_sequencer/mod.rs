pub mod constants;
pub mod setup;
pub mod types;
pub mod utils;

use blockifier::abi::abi_utils::get_storage_var_address;
use blockifier::execution::contract_class::{ContractClass, ContractClassV0};
use blockifier::state::errors::StateError;
use blockifier::state::state_api::{State as BlockifierState, StateResult};
use cairo_vm::types::errors::program_errors::ProgramError;
use sequencer::sequencer::Sequencer;
use sequencer::state::State;

use self::constants::{
    BLOCK_CONTEXT, CONTRACT_ACCOUNT_CLASS, CONTRACT_ACCOUNT_CLASS_HASH, EOA_CLASS, EOA_CLASS_HASH,
    FEE_TOKEN_ADDRESS, KAKAROT_ADDRESS, KAKAROT_CLASS, KAKAROT_CLASS_HASH, KAKAROT_OWNER_ADDRESS,
    PROXY_CLASS, PROXY_CLASS_HASH,
};

pub(crate) struct KakarotSequencer(Sequencer<State>);

#[allow(dead_code)]
impl KakarotSequencer {
    pub fn new(state: State) -> Self {
        let sequencer = Sequencer::new(BLOCK_CONTEXT.clone(), state);
        Self(sequencer)
    }

    pub fn initialize(mut self) -> StateResult<Self> {
        let storage = vec![
            ("Ownable_owner", *KAKAROT_OWNER_ADDRESS.0.key()),
            ("native_token_address", *FEE_TOKEN_ADDRESS.0.key()),
            ("contract_account_class_hash", CONTRACT_ACCOUNT_CLASS_HASH.0),
            ("externally_owned_account_class_hash", EOA_CLASS_HASH.0),
            ("account_proxy_class_hash", PROXY_CLASS_HASH.0),
        ];

        // Write all the storage vars to the sequencer state.
        for (k, v) in storage {
            (&mut self.0.state).set_storage_at(
                *KAKAROT_ADDRESS,
                get_storage_var_address(k, &[]).unwrap(), // safe unwrap: all vars are ASCII
                v,
            );
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
