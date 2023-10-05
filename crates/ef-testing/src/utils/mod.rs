use std::{fs, path::Path};

use reth_primitives::{keccak256, Address};
use revm_primitives::B256;
use secp256k1::{PublicKey, SecretKey};
use serde::Deserialize;

use crate::models::error::RunnerError;

pub(crate) fn load_file(path: &Path) -> Result<String, RunnerError> {
    fs::read_to_string(path).map_err(|error| RunnerError::Io {
        path: path.into(),
        error: error.to_string(),
    })
}

pub(crate) fn deserialize_into<T: for<'a> Deserialize<'a>>(
    val: &str,
    path: &Path,
) -> Result<T, RunnerError> {
    serde_json::from_str(val).map_err(|error| RunnerError::Io {
        path: path.into(),
        error: error.to_string(),
    })
}

pub(crate) fn address_from_private_key(sk: B256) -> Result<Address, RunnerError> {
    let sk =
        SecretKey::from_slice(sk.as_bytes()).map_err(|err| RunnerError::Other(err.to_string()))?;
    let pk = PublicKey::from_secret_key(&secp256k1::Secp256k1::new(), &sk);
    Ok(Address::from_slice(
        keccak256(&pk.serialize_uncompressed()[1..]).as_bytes()[12..].as_ref(),
    ))
}
