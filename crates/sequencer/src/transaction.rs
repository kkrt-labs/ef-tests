use std::sync::Arc;

use blockifier::transaction::{
    account_transaction::AccountTransaction,
    transaction_execution::Transaction as ExecutionTransaction,
};
use starknet::core::types::{BroadcastedTransaction, FieldElement};
use starknet_api::core::{ContractAddress, Nonce, PatriciaKey};
use starknet_api::hash::{StarkFelt, StarkHash};
use starknet_api::transaction::InvokeTransaction;
use starknet_api::transaction::{
    Calldata, Fee, InvokeTransactionV1, TransactionHash, TransactionSignature,
};

#[derive(Debug)]
pub struct StarknetTransaction(BroadcastedTransaction);

impl StarknetTransaction {
    pub fn new(transaction: BroadcastedTransaction) -> Self {
        Self(transaction)
    }
}

impl TryFrom<StarknetTransaction> for ExecutionTransaction {
    type Error = eyre::Error;

    fn try_from(transaction: StarknetTransaction) -> Result<Self, Self::Error> {
        match transaction.0 {
            BroadcastedTransaction::Invoke(invoke) => Ok(ExecutionTransaction::AccountTransaction(
                AccountTransaction::Invoke(InvokeTransaction::V1(InvokeTransactionV1 {
                    transaction_hash: TransactionHash(Into::<StarkHash>::into(
                        Into::<StarkFelt>::into(FieldElement::ONE), // TODO: Replace with actual computed hash.
                    )),
                    max_fee: Fee(invoke.max_fee.try_into()?),
                    signature: TransactionSignature(
                        invoke
                            .signature
                            .into_iter()
                            .map(Into::<StarkFelt>::into)
                            .collect(),
                    ),
                    nonce: Nonce(invoke.nonce.try_into()?),
                    sender_address: ContractAddress(TryInto::<PatriciaKey>::try_into(Into::<
                        StarkHash,
                    >::into(
                        Into::<StarkFelt>::into(invoke.sender_address),
                    ))?),
                    calldata: Calldata(Arc::new(
                        invoke
                            .calldata
                            .into_iter()
                            .map(Into::<StarkFelt>::into)
                            .collect(),
                    )),
                })),
            )),
            // TODO: Add support for other transaction types.
            _ => Err(eyre::eyre!("Unsupported transaction type")),
        }
    }
}
