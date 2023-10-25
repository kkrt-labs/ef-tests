pub mod account;
pub mod constants;
pub mod evm_state;
pub mod sequencer;
pub mod types;
pub mod utils;

use blockifier::state::errors::StateError;
use cairo_vm::types::errors::program_errors::ProgramError;

use self::types::contract_class::ContractClassConversionError;
use thiserror::Error;

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

#[cfg(test)]
mod tests {
    use crate::evm_sequencer::sequencer::{InitializeSequencer, KakarotSequencer};
    use ::sequencer::state::State;

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
