//! Traits definition
//! Inspired by <https://github.com/paradigmxyz/reth/tree/main/testing/ef-tests>
use crate::models::error::RunnerError;
use async_trait::async_trait;
use std::fmt::Debug;

/// A single test case, capable of loading a JSON description of itself and running it.
#[async_trait]
pub trait Case: Debug + Sync + Send + Sized {
    /// Run the test on the Kakarot test sequencer.
    fn run(&self) -> Result<(), RunnerError>;
}
