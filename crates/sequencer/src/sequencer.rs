use crate::{commit::Committer, execution::Execution};
use blockifier::{
    block_context::BlockContext,
    state::{
        cached_state::CachedState,
        state_api::{State, StateReader},
    },
    transaction::{
        errors::TransactionExecutionError, transaction_execution::Transaction,
        transactions::ExecutableTransaction,
    },
};
use tracing::{trace, warn};

/// Sequencer is the main struct of the sequencer crate.
/// Using a trait bound for the state allows for better
/// speed, as the type of the state is known at compile time.
/// We bound S such that a mutable reference to S (&'a mut S)
/// must implement State and StateReader. The `for` keyword
/// indicates that the bound must hold for any lifetime 'a.
/// For more details, check out https://doc.rust-lang.org/nomicon/hrtb.html
pub struct Sequencer<S>
where
    for<'a> &'a mut S: State + StateReader + Committer,
{
    pub context: BlockContext,
    pub state: S,
}

impl<S> Sequencer<S>
where
    for<'a> &'a mut S: State + StateReader + Committer,
{
    /// Creates a new Sequencer instance.
    pub fn new(context: BlockContext, state: S) -> Self {
        Self { context, state }
    }
}

impl<S> Execution for Sequencer<S>
where
    for<'a> &'a mut S: State + StateReader + Committer,
{
    fn execute(&mut self, transaction: Transaction) -> Result<(), TransactionExecutionError> {
        let mut cached_state = CachedState::new(&mut self.state);
        let res = transaction.execute(&mut cached_state, &self.context, false);

        match res {
            Err(err) => {
                warn!("Transaction execution failed: {:?}", err)
            }
            Ok(execution_information) => {
                <&mut S>::commit(&mut cached_state);
                match execution_information.revert_error {
                    Some(err) => {
                        warn!("Transaction execution failed: {:?}", err)
                    }
                    None => {
                        trace!("Transaction execution succeeded {execution_information:?}")
                    }
                }
            }
        }

        Ok(())
    }
}
