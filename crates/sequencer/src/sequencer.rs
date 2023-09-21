use blockifier::block_context::BlockContext;
use blockifier::state::state_api::{State, StateReader};

/// Sequencer is the main struct of the sequencer crate.
/// Using a trait bound for the state allows for better
/// speed, as the type of the state is known at compile time.
/// We bound S such that a mutable reference to S (&'a mut S)
/// must implement State and StateReader. The `for` keyword
/// indicates that the bound must hold for any lifetime 'a.
/// For more details, check out https://doc.rust-lang.org/nomicon/hrtb.html
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
