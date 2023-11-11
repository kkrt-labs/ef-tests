pub mod evm_sequencer;
pub mod models;
pub mod test_utils;
pub mod traits;
pub mod utils;

use bytes::BytesMut;
use evm_sequencer::constants::CHAIN_ID;
use models::error::RunnerError;
use reth_primitives::{
    sign_message, AccessList, Bytes, SealedBlock, Signature, Transaction, TxEip2930,
};
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
    let tx_signed = block.body.first().cloned().ok_or_else(|| {
        RunnerError::Other(vec!["No transaction in pre state block".into()].into())
    })?;

    let mut tx = match &tx_signed.transaction {
        Transaction::Legacy(tx) => Transaction::Eip2930(TxEip2930 {
            chain_id: *CHAIN_ID,
            nonce: tx.nonce,
            gas_limit: tx.gas_limit,
            gas_price: tx.gas_price,
            to: tx.to,
            value: tx.value,
            access_list: AccessList::default(),
            input: tx.input.clone(),
        }),
        _ => tx_signed.transaction,
    };

    tx.set_chain_id(*CHAIN_ID);
    let signature = sign_tx(&tx, &pk)?;
    tx.encode_with_signature(&signature, &mut out, false);

    Ok(out.to_vec().into())
}
