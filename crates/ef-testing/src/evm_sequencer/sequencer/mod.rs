use std::ops::{Deref, DerefMut};

use blockifier::transaction::{
    errors::TransactionExecutionError, transaction_execution::Transaction,
};
use sequencer::{execution::Execution, sequencer::Sequencer, state::State};

use super::{constants::BLOCK_CONTEXT, InitializationResult};

#[cfg(feature = "v0")]
pub mod v0;
#[cfg(feature = "v1")]
pub mod v1;

/// Sequencer initialization interface. Initializes the sequencer state
/// by setting contract, its storage, declares all necessary classes and
/// deploys the fee token contract.
/// Default implementation is used when no feature flag is enabled.
pub trait InitializeSequencer {
    fn initialize(self) -> InitializationResult<Self>
    where
        Self: Sized,
    {
        panic!("Not implemented, use features flag \"v0\" or \"v1\"")
    }
}

#[cfg(not(any(feature = "v0", feature = "v1")))]
impl InitializeSequencer for KakarotSequencer {}

/// Kakarot wrapper around a sequencer.
pub(crate) struct KakarotSequencer(Sequencer<State>);

impl KakarotSequencer {
    pub fn new(state: State) -> Self {
        let sequencer = Sequencer::new(BLOCK_CONTEXT.clone(), state);
        Self(sequencer)
    }
}

impl Deref for KakarotSequencer {
    type Target = Sequencer<State>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for KakarotSequencer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Execution for KakarotSequencer {
    fn execute(&mut self, transaction: Transaction) -> Result<(), TransactionExecutionError> {
        let _ = self.0.execute(transaction);
        Ok(())
    }
}
