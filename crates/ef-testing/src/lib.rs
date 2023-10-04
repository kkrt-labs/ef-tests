pub mod evm_sequencer;
pub mod models;
pub mod traits;
pub mod utils;

use bytes::BytesMut;
use evm_sequencer::constants::CHAIN_ID;
use models::error::RunnerError;
use reth_primitives::{sign_message, Bytes, SealedBlock, Signature, Transaction};
use reth_rlp::Decodable;
use revm_primitives::B256;

/// Sign a transaction given a private key.
pub fn sign_tx(tx: &Transaction, private_key: &B256) -> Result<Signature, eyre::Error> {
    let signature = sign_message(*private_key, tx.signature_hash())?;
    Ok(signature)
}

pub fn get_signed_rlp_encoded_transaction(block: &Bytes, pk: B256) -> Result<Bytes, RunnerError> {
    // Parse it as a sealed block
    let block = SealedBlock::decode(&mut block.as_ref()).map_err(RunnerError::RlpDecodeError)?;

    // Encode body as transaction
    let mut out = BytesMut::new();
    let mut tx_signed = block
        .body
        .first()
        .cloned()
        .ok_or_else(|| RunnerError::Other("No transaction in pre state block".to_string()))?;

    tx_signed.transaction.set_chain_id(*CHAIN_ID);
    let signature = sign_tx(&tx_signed.transaction, &pk)?;
    tx_signed.encode_with_signature(&signature, &mut out, true);

    Ok(out.to_vec().into())
}
