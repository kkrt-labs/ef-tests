// Inspired by https://github.com/paradigmxyz/reth/tree/main/testing/ef-tests
// Modified to use async running

use crate::traits::Suite;

use super::case::BlockchainTestCase;

pub struct BlockchainTestSuite {
    pub name: String,
}

impl BlockchainTestSuite {
    #[must_use]
    pub const fn new(name: String) -> Self {
        Self { name }
    }
}

impl Suite for BlockchainTestSuite {
    type Case = BlockchainTestCase;

    fn suite_name(&self) -> String {
        format!("BlockchainTests/{}", self.name)
    }
}
