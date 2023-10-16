//! Traits definition
//! Inspired by <https://github.com/paradigmxyz/reth/tree/main/testing/ef-tests>
use crate::models::error::RunnerError;
use async_trait::async_trait;
use std::fmt::Debug;

/// A single test case, capable of loading a JSON description of itself and running it.
#[async_trait]
pub trait Case: Debug + Sync + Send + Sized {
    /// A description of the test.
    fn description(&self) -> String {
        "no description".to_string()
    }

    /// Run the test on the Katana test context.
    async fn run(&self) -> Result<(), RunnerError>;
}
