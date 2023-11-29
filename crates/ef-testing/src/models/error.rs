use std::{
    fmt::{Debug, Display},
    path::PathBuf,
};

use blockifier::{state::errors::StateError, transaction::errors::TransactionExecutionError};
use starknet::{
    core::{types::FromByteArrayError, utils::NonAsciiNameError},
    providers::ProviderError,
};
use starknet_api::StarknetApiError;

use crate::evm_sequencer::InitializationError;

/// Error type based off <https://github.com/paradigmxyz/reth/blob/main/testing/ef-tests/src/result.rs>
#[derive(Debug, thiserror::Error)]
pub enum RunnerError {
    /// Assertion error
    #[error("{0}")]
    Assertion(String),
    /// An error occurred while decoding RLP.
    #[error("An error occurred deserializing RLP")]
    RlpDecodeError(#[from] reth_rlp::DecodeError),
    /// An IO error occurred
    #[error("An error occurred interacting with the file system at {path}: {error}")]
    Io {
        /// The path to the file or directory
        path: PathBuf,
        /// The specific error
        error: String,
    },
    /// Sequencer initialization error
    #[error("An error occurred while initializing the sequencer: {0}")]
    InitializationError(#[from] InitializationError),
    /// Sequencer error
    #[error("An error occurred while running the sequencer: {0}")]
    SequencerError(#[from] StateError),
    /// Execution error
    #[error("An error occurred while executing the transaction: {0}")]
    ExecutionError(#[from] TransactionExecutionError),
    /// Skipped test
    #[error("test skipped")]
    Skipped,
    /// Other
    #[error(transparent)]
    Other(Messages),
}

pub struct Messages(Vec<String>);

impl std::error::Error for Messages {}

impl From<Vec<String>> for Messages {
    fn from(messages: Vec<String>) -> Self {
        Self(messages)
    }
}

impl Debug for Messages {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "\n{}\n", self.0.join("\n"))
    }
}

impl Display for Messages {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "\n{}\n", self.0.join("\n"))
    }
}

impl From<eyre::Error> for RunnerError {
    fn from(err: eyre::Error) -> Self {
        Self::Other(vec![err.to_string()].into())
    }
}

impl<E: std::error::Error> From<ProviderError<E>> for RunnerError {
    fn from(err: ProviderError<E>) -> Self {
        Self::Other(vec![err.to_string()].into())
    }
}

impl From<regex::Error> for RunnerError {
    fn from(err: regex::Error) -> Self {
        Self::Other(vec![err.to_string()].into())
    }
}

impl From<FromByteArrayError> for RunnerError {
    fn from(err: FromByteArrayError) -> Self {
        Self::Other(vec![err.to_string()].into())
    }
}

impl From<StarknetApiError> for RunnerError {
    fn from(err: StarknetApiError) -> Self {
        Self::Other(vec![err.to_string()].into())
    }
}

impl From<NonAsciiNameError> for RunnerError {
    fn from(err: NonAsciiNameError) -> Self {
        Self::Other(vec![err.to_string()].into())
    }
}
