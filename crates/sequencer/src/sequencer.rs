use crate::config::SequencerConfig;
use blockifier::state::state_api::{State, StateReader};

/// Sequencer is the main struct of the sequencer crate.
/// Using a trait bound for the state allows for better
/// speed, as the type of the state is known at compile time.
pub struct Sequencer<S: State + StateReader> {
    config: SequencerConfig,
    state: S,
}

impl<S: State + StateReader> Sequencer<S> {
    /// Creates a new Sequencer instance.
    pub fn new(config: SequencerConfig, state: S) -> Self {
        Self { config, state }
    }
}
