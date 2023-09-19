use std::{fmt::Debug, path::PathBuf};

use kakarot_rpc_core::{client::errors::EthApiError, models::ConversionError};
use starknet::{
    core::{types::FromByteArrayError, utils::NonAsciiNameError},
    providers::ProviderError,
};
use starknet_api::StarknetApiError;

/// Error type based off <https://github.com/paradigmxyz/reth/blob/main/testing/ef-tests/src/result.rs>
#[derive(Clone, Debug, thiserror::Error)]
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
    /// Sequencer error
    #[error("An error occurred while running the sequencer: {0}")]
    SequencerError(String),
    /// Skipped test
    #[error("test skipped")]
    Skipped,
    /// Other
    #[error("{0}")]
    Other(String),
}

impl From<eyre::Error> for RunnerError {
    fn from(err: eyre::Error) -> Self {
        Self::Other(err.to_string())
    }
}

impl<E: std::error::Error> From<EthApiError<E>> for RunnerError {
    fn from(err: EthApiError<E>) -> Self {
        Self::Other(err.to_string())
    }
}

impl<E: std::error::Error> From<ProviderError<E>> for RunnerError {
    fn from(err: ProviderError<E>) -> Self {
        Self::Other(err.to_string())
    }
}

impl From<regex::Error> for RunnerError {
    fn from(err: regex::Error) -> Self {
        Self::Other(err.to_string())
    }
}

impl From<ConversionError<()>> for RunnerError {
    fn from(err: ConversionError<()>) -> Self {
        Self::Other(err.to_string())
    }
}

impl From<FromByteArrayError> for RunnerError {
    fn from(err: FromByteArrayError) -> Self {
        Self::Other(err.to_string())
    }
}

impl From<StarknetApiError> for RunnerError {
    fn from(err: StarknetApiError) -> Self {
        Self::Other(err.to_string())
    }
}

impl From<NonAsciiNameError> for RunnerError {
    fn from(err: NonAsciiNameError) -> Self {
        Self::Other(err.to_string())
    }
}
