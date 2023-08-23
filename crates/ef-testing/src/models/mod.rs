pub mod case;
pub mod result;
pub mod suite;

use ef_tests::models::{ForkSpec, Header, RootOrState, State};
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

/// The definition of a blockchain test.
#[derive(Debug, PartialEq, Eq, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BlockchainTest {
    /// Block data.
    pub blocks: Vec<Block>,
    /// The expected post state.
    pub post_state: Option<RootOrState>,
    /// The test pre-state.
    pub pre: State,
    /// Network spec.
    pub network: ForkSpec,
}

/// A block in an Ethereum blockchain test.
#[derive(Debug, PartialEq, Eq, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Block {
    /// Block header.
    pub block_header: Option<Header>,
    /// RLP encoded block bytes
    pub rlp: Bytes,
    /// Uncle/ommer headers
    pub uncle_headers: Option<Vec<Header>>,
}
