#[cfg(feature = "v0")]
pub mod v0;
#[cfg(feature = "v1")]
pub mod v1;

use std::ops::{Deref, DerefMut};

use blockifier::transaction::{
    objects::{TransactionExecutionInfo, TransactionExecutionResult},
    transaction_execution::Transaction,
};
use sequencer::{execution::Execution, sequencer::Sequencer, state::State};

use super::constants::BLOCK_CONTEXT;

/// Kakarot wrapper around a sequencer.
#[derive(Clone)]
pub(crate) struct KakarotSequencer(Sequencer<State>);

impl KakarotSequencer {
    pub fn new() -> Self {
        let initial_state = {
            #[cfg(feature = "v0")]
            {
                v0::INITIAL_SEQUENCER_STATE.clone()
            }
            #[cfg(feature = "v1")]
            {
                v1::INITIAL_SEQUENCER_STATE.clone()
            }
            #[cfg(not(any(feature = "v0", feature = "v1")))]
            panic!("No sequencer version enabled")
        };
        let sequencer = Sequencer::new(BLOCK_CONTEXT.clone(), initial_state);
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
    fn execute(
        &mut self,
        transaction: Transaction,
    ) -> TransactionExecutionResult<TransactionExecutionInfo> {
        self.0.execute(transaction)
    }
}
