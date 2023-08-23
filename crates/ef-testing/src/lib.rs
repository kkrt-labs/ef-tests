pub mod constants;
pub mod models;
pub mod storage;
pub mod traits;
pub mod utils;

use bytes::BytesMut;
use kakarot_rpc_core::client::constants::CHAIN_ID;
use reth_primitives::{sign_message, Bytes, SealedBlock, Signature, Transaction};
use reth_rlp::Decodable;
use revm_primitives::B256;

/// Sign a transaction given a private key and a chain id.
pub fn sign_tx_with_chain_id(
    tx: &mut Transaction,
    private_key: &B256,
    chain_id: u64,
) -> Result<Signature, eyre::Error> {
    tx.set_chain_id(chain_id);
    let signature = sign_message(*private_key, tx.signature_hash())?;
    Ok(signature)
}

pub fn get_signed_rlp_encoded_transaction(
    block: Bytes,
    pk: B256,
) -> Result<Bytes, ef_tests::Error> {
    // parse it as a sealed block
    let mut block =
        SealedBlock::decode(&mut block.as_ref()).map_err(ef_tests::Error::RlpDecodeError)?;

    // encode body as transaction
    let mut out = BytesMut::new();
    let tx_signed = block.body.get_mut(0).unwrap();

    let tx = &mut tx_signed.transaction;
    tx.set_chain_id(CHAIN_ID);
    let signature = sign_tx_with_chain_id(tx, &pk, CHAIN_ID).unwrap();
    tx_signed.encode_with_signature(&signature, &mut out, true);

    Ok(out.to_vec().into())
}
