use std::sync::Arc;

use blockifier::transaction::{
    account_transaction::AccountTransaction,
    transaction_execution::Transaction as ExecutionTransaction,
};
use starknet::core::crypto::compute_hash_on_elements;
use starknet::core::types::{BroadcastedInvokeTransaction, BroadcastedTransaction, Felt};
use starknet_api::core::Nonce;
use starknet_api::executable_transaction::{
    AccountTransaction as AccountTransactionEnum, InvokeTransaction,
};
use starknet_api::transaction::fields::{Calldata, Fee, TransactionSignature};
use starknet_api::transaction::{InvokeTransactionV1, TransactionHash};

/// Wrapper around a Starknet-rs transaction.
/// Allows for conversion from a Starknet-rs
/// transaction to a Blockifier-rs transaction.
#[derive(Debug)]
pub struct BroadcastedTransactionWrapper(BroadcastedTransaction);

impl BroadcastedTransactionWrapper {
    #[must_use]
    #[inline]
    pub const fn new(transaction: BroadcastedTransaction) -> Self {
        Self(transaction)
    }

    #[inline]
    pub fn try_into_execution_transaction(
        self,
        chain_id: Felt,
    ) -> Result<ExecutionTransaction, eyre::Error> {
        match self.0 {
            BroadcastedTransaction::Invoke(invoke) => match invoke {
                BroadcastedInvokeTransaction::V1(invoke_v1) => {
                    Ok(ExecutionTransaction::Account(AccountTransaction {
                        tx: AccountTransactionEnum::Invoke(InvokeTransaction {
                            tx: starknet_api::transaction::InvokeTransaction::V1(
                                InvokeTransactionV1 {
                                    max_fee: Fee(invoke_v1.max_fee.to_biguint().try_into()?),
                                    signature: TransactionSignature(
                                        invoke_v1.signature.into_iter().map(Into::into).collect(),
                                    ),
                                    nonce: Nonce(invoke_v1.nonce),
                                    sender_address: invoke_v1.sender_address.try_into()?,
                                    calldata: Calldata(Arc::new(invoke_v1.calldata.to_vec())),
                                },
                            ),
                            tx_hash: TransactionHash(compute_transaction_hash(
                                invoke_v1.sender_address,
                                &invoke_v1.calldata,
                                invoke_v1.max_fee,
                                chain_id,
                                invoke_v1.nonce,
                            )),
                        }),
                        only_query: false,
                    }))
                }
                BroadcastedInvokeTransaction::V3(_) => {
                    Err(eyre::eyre!("Unsupported InvokeTransaction version V3"))
                }
            },
            // TODO: Add support for other transaction types.
            _ => Err(eyre::eyre!("Unsupported transaction type")),
        }
    }
}

fn compute_transaction_hash(
    sender_address: Felt,
    calldata: &[Felt],
    max_fee: Felt,
    chain_id: Felt,
    nonce: Felt,
) -> Felt {
    compute_hash_on_elements(&[
        Felt::from_bytes_be_slice(b"invoke"),
        Felt::ONE,
        sender_address,
        Felt::ZERO, // entry_point_selector
        compute_hash_on_elements(calldata),
        max_fee,
        chain_id,
        nonce,
    ])
}
