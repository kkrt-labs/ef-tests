use std::sync::Arc;

use blockifier::transaction::{
    account_transaction::AccountTransaction,
    transaction_execution::Transaction as ExecutionTransaction,
};
use starknet::core::crypto::compute_hash_on_elements;
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

    pub fn try_into_execution_transaction(
        self,
        chain_id: FieldElement,
    ) -> Result<ExecutionTransaction, eyre::Error> {
        match self.0 {
            BroadcastedTransaction::Invoke(invoke) => Ok(ExecutionTransaction::AccountTransaction(
                AccountTransaction::Invoke(InvokeTransaction::V1(InvokeTransactionV1 {
                    transaction_hash: TransactionHash(Into::<StarkHash>::into(
                        Into::<StarkFelt>::into(compute_transaction_hash(
                            invoke.sender_address,
                            &invoke.calldata,
                            invoke.max_fee,
                            chain_id,
                            invoke.nonce,
                        )),
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

const PREFIX_INVOKE: FieldElement = FieldElement::from_mont([
    18443034532770911073,
    18446744073709551615,
    18446744073709551615,
    513398556346534256,
]);

fn compute_transaction_hash(
    sender_address: FieldElement,
    calldata: &[FieldElement],
    max_fee: FieldElement,
    chain_id: FieldElement,
    nonce: FieldElement,
) -> FieldElement {
    compute_hash_on_elements(&[
        PREFIX_INVOKE,
        FieldElement::ONE,
        sender_address,
        FieldElement::ZERO, // entry_point_selector
        compute_hash_on_elements(calldata),
        max_fee,
        chain_id,
        nonce,
    ])
}
