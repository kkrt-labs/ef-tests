pub mod account;
pub mod constants;
pub mod contract_class;
pub mod evm_state;
pub mod types;
pub mod utils;

use blockifier::abi::abi_utils::get_storage_var_address;
use blockifier::execution::contract_class::{ContractClass, ContractClassV0, ContractClassV1};
use blockifier::state::errors::StateError;
use blockifier::state::state_api::State as BlockifierState;
use blockifier::transaction::errors::TransactionExecutionError;
use blockifier::transaction::transaction_execution::Transaction;
use cairo_lang_starknet::casm_contract_class::CasmContractClass;
use cairo_vm::types::errors::program_errors::ProgramError;
use sequencer::execution::Execution;
use sequencer::sequencer::Sequencer;
use sequencer::state::State;

use self::constants::kkrt_constants::{
    CONTRACT_ACCOUNT_CLASS, CONTRACT_ACCOUNT_CLASS_HASH, EOA_CLASS, EOA_CLASS_HASH,
    KAKAROT_ADDRESS, KAKAROT_CLASS, KAKAROT_CLASS_HASH, PROXY_CLASS, PROXY_CLASS_HASH,
};
use self::constants::{
    BLOCK_CONTEXT, ETH_FEE_TOKEN_ADDRESS, FEE_TOKEN_CLASS, FEE_TOKEN_CLASS_HASH,
    KAKAROT_ADDRESS_V1, KAKAROT_OWNER_ADDRESS,
};
use self::contract_class::{CasmContractClassWrapper, ContractClassConversionError};
use thiserror::Error;

/// Kakarot wrapper around a sequencer.
pub(crate) struct KakarotSequencer(Sequencer<State>);

type InitializationResult<T> = Result<T, InitializationError>;

#[derive(Debug, Error)]
pub enum InitializationError {
    #[error(transparent)]
    ClassConversionError(#[from] ContractClassConversionError),
    #[error(transparent)]
    ProgramError(#[from] ProgramError),
    #[error(transparent)]
    StateError(#[from] StateError),
}

impl KakarotSequencer {
    pub fn new(state: State) -> Self {
        let sequencer = Sequencer::new(BLOCK_CONTEXT.clone(), state);
        Self(sequencer)
    }

    /// Initializes the sequencer state with the Kakarot contract, its storage,
    /// declares all necessary classes and deploys the fee token contract.
    pub fn initialize(mut self) -> InitializationResult<Self> {
        let storage = vec![
            ("Ownable_owner", *KAKAROT_OWNER_ADDRESS.0.key()),
            ("native_token_address", *ETH_FEE_TOKEN_ADDRESS.0.key()),
            ("contract_account_class_hash", CONTRACT_ACCOUNT_CLASS_HASH.0),
            ("externally_owned_account_class_hash", EOA_CLASS_HASH.0),
            ("account_proxy_class_hash", PROXY_CLASS_HASH.0),
        ];

        // Write all the storage vars to the sequencer state.
        for (k, v) in storage {
            (&mut self.0.state).set_storage_at(
                *KAKAROT_ADDRESS,
                get_storage_var_address(k, &[]),
                v,
            );
        }

        // // Write the v1 class hash
        // (&mut self.0.state).set_class_hash_at(*KAKAROT_ADDRESS_V1, *KAKAROT_CLASS_HASH_V1)?;
        // (&mut self.0.state).set_contract_class(
        //     &KAKAROT_CLASS_HASH_V1,
        //     ContractClass::V1(ContractClassV1::try_from(Into::<CasmContractClass>::into(
        //         CasmContractClassWrapper::try_from(&*KAKAROT_CLASS_V1)?,
        //     ))?),
        // )?;

        // Write the kakarot class and class hash.
        (&mut self.0.state).set_class_hash_at(*KAKAROT_ADDRESS, *KAKAROT_CLASS_HASH)?;
        (&mut self.0.state).set_contract_class(
            &KAKAROT_CLASS_HASH,
            ContractClass::V0(ContractClassV0::try_from_json_string(
                &serde_json::to_string(&*KAKAROT_CLASS).map_err(ProgramError::Parse)?,
            )?),
        )?;

        // Write proxy, eoa, contract account and erc20 classes and class hashes.
        (&mut self.0.state).set_contract_class(
            &PROXY_CLASS_HASH,
            ContractClass::V0(ContractClassV0::try_from_json_string(
                &serde_json::to_string(&*PROXY_CLASS).map_err(ProgramError::Parse)?,
            )?),
        )?;
        (&mut self.0.state).set_contract_class(
            &CONTRACT_ACCOUNT_CLASS_HASH,
            ContractClass::V0(ContractClassV0::try_from_json_string(
                &serde_json::to_string(&*CONTRACT_ACCOUNT_CLASS).map_err(ProgramError::Parse)?,
            )?),
        )?;
        (&mut self.0.state).set_contract_class(
            &EOA_CLASS_HASH,
            ContractClass::V0(ContractClassV0::try_from_json_string(
                &serde_json::to_string(&*EOA_CLASS).map_err(ProgramError::Parse)?,
            )?),
        )?;
        (&mut self.0.state).set_contract_class(
            &FEE_TOKEN_CLASS_HASH,
            ContractClass::V0(ContractClassV0::try_from_json_string(
                &serde_json::to_string(&*FEE_TOKEN_CLASS).map_err(ProgramError::Parse)?,
            )?),
        )?;
        (&mut self.0.state).set_class_hash_at(*ETH_FEE_TOKEN_ADDRESS, *FEE_TOKEN_CLASS_HASH)?;

        Ok(self)
    }
}

impl Execution for KakarotSequencer {
    fn execute(
        &mut self,
        transaction: Transaction,
    ) -> TransactionExecutionResult<TransactionExecutionInfo> {
        self.0.execute(transaction)
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
