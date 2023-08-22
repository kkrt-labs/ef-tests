pub mod case;
pub mod result;

use reth_primitives::{Bytes, U64};
use revm_primitives::{B160, B256};
use serde::{self, Deserialize};

#[derive(Debug, PartialEq, Eq, Deserialize)]
pub struct BlockchainTestTransaction {
    pub transaction: Transaction,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transaction {
    pub data: Vec<Bytes>,
    pub gas_limit: Vec<U64>,
    pub gas_price: U64,
    pub nonce: U64,
    pub secret_key: B256,
    pub to: B160,
    pub sender: B160,
    pub value: Vec<U64>,
}
