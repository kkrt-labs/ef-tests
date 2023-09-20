use crate::config::SequencerConfig;
use blockifier::state::state_api::{State, StateReader};

/// Sequencer is the main struct of the sequencer crate.
/// Using a trait bound for the state allows for better
/// speed, as the type of the state is known at compile time.
pub struct Sequencer<S>
where
    for<'a> &'a mut S: State + StateReader,
{
    pub context: BlockContext,
    pub state: S,
}

impl<S> Sequencer<S>
where
    for<'a> &'a mut S: State + StateReader,
{
    /// Creates a new Sequencer instance.
    pub fn new(context: BlockContext, state: S) -> Self {
        Self { context, state }
    }
}
    }
}
