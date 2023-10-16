// Inspired by https://github.com/paradigmxyz/reth/tree/main/testing/ef-tests

use super::error::RunnerError;
use crate::{
    evm_sequencer::{
        constants::CHAIN_ID, evm_state::EvmState, utils::to_broadcasted_starknet_transaction,
        KakarotSequencer,
    },
    get_signed_rlp_encoded_transaction,
    traits::Case,
    utils::{load_file, update_post_state},
};
use async_trait::async_trait;
use ef_tests::models::BlockchainTest;
use ef_tests::models::RootOrState;

use ethers_signers::{LocalWallet, Signer};
use regex::Regex;
use revm_primitives::B256;
use sequencer::{
    execution::Execution, state::State as SequencerState, transaction::StarknetTransaction,
};
use serde::Deserialize;
use starknet::core::types::{BroadcastedTransaction, FieldElement};
use std::{collections::BTreeMap, path::Path};

#[derive(Debug)]
pub struct BlockchainTestCase {
    pub name: String,
    pub test: BlockchainTest,
    pub secret_key: B256,
}

#[derive(Deserialize)]
pub struct BlockchainTestsSkip {
    pub filename: BTreeMap<String, Vec<String>>,
    pub regex: BTreeMap<String, Vec<String>>,
}

lazy_static::lazy_static! {
    pub static ref SKIP: BlockchainTestsSkip = {
        let skip_file = Path::new("../../blockchain-tests-skip.yml");
        let skip_str = load_file(skip_file).unwrap();

        serde_yaml::from_str(&skip_str).unwrap()
    };
}

// Division of logic:
// 'handle' methods attempt to abstract the data coming from BlockChainTestCase
// from more general logic that can be used across tests
impl BlockchainTestCase {
    /// Returns whether a given test should be skipped
    /// # Panics
    ///
    /// Will panic if the file name cannot be stringified.
    #[must_use]
    pub fn should_skip(path: &Path) -> bool {
        let dir = path
            .parent()
            .unwrap()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap();
        let name = path.file_name().unwrap().to_str().unwrap();

        let mut should_skip = if SKIP.filename.contains_key(dir) {
            SKIP.filename
                .get(dir)
                .unwrap()
                .iter()
                .any(|filename| filename == name)
        } else {
            false
        };

        if !should_skip && SKIP.regex.contains_key(dir) {
            should_skip = SKIP
                .regex
                .get(dir)
                .unwrap()
                .iter()
                .any(|regex| Regex::new(regex.as_str()).unwrap().is_match(name));
        }

        should_skip
    }

    async fn handle_pre_state(&self, sequencer: &mut KakarotSequencer) -> Result<(), RunnerError> {
        for (address, account) in self.test.pre.iter() {
            sequencer.setup_account(
                address,
                &account.code,
                account.nonce.0,
                account.storage.iter().map(|(k, v)| (k.0, v.0)).collect(),
            )?;
            sequencer.fund(address, account.balance.0)?;
        }

        Ok(())
    }

    async fn handle_transaction(
        &self,
        sequencer: &mut KakarotSequencer,
    ) -> Result<(), RunnerError> {
        // we extract the transaction from the block
        let block = self
            .test
            .blocks
            .first()
            .ok_or_else(|| RunnerError::Other("test has no blocks".to_string()))?;
        // we adjust the rlp to correspond with our currently hardcoded CHAIN_ID
        let tx_encoded = get_signed_rlp_encoded_transaction(&block.rlp, self.secret_key)?;

        let starknet_transaction = StarknetTransaction::new(BroadcastedTransaction::Invoke(
            to_broadcasted_starknet_transaction(&tx_encoded)?,
        ));
        sequencer.execute(
            starknet_transaction
                .try_into_execution_transaction(FieldElement::from(*CHAIN_ID))
                .unwrap(),
        )?;

        Ok(())
    }

    async fn handle_post_state(&self, sequencer: &mut KakarotSequencer) -> Result<(), RunnerError> {
        let wallet = LocalWallet::from_bytes(&self.secret_key.0)
            .map_err(|err| RunnerError::Other(err.to_string()))?;
        let sender_address = wallet.address().to_fixed_bytes();

        // Get gas used from block header
        let maybe_block = self.test.blocks.first();
        let maybe_block_header = maybe_block.and_then(|block| block.block_header.as_ref());
        let gas_used = maybe_block_header
            .map(|block_header| block_header.gas_used.0)
            .unwrap_or_default();

        // Get coinbase address
        let coinbase = maybe_block_header
            .map(|block_header| block_header.coinbase)
            .unwrap_or_default();

        // Get baseFeePerGas
        let base_fee_per_gas = maybe_block_header
            .and_then(|block_header| block_header.base_fee_per_gas)
            .map(|base_fee| base_fee.0)
            .unwrap_or_default();

        // Get gas price from transaction
        let maybe_transaction = maybe_block
            .and_then(|block| block.transactions.as_ref())
            .and_then(|transactions| transactions.first());
        let gas_price = maybe_transaction
            .and_then(|transaction| transaction.gas_price)
            .map(|gas_price| gas_price.0)
            .unwrap_or_default();
        let transaction_cost = if coinbase.0 != sender_address {
            gas_price
        } else {
            base_fee_per_gas
        } * gas_used;

        let post_state =
            match self.test.post_state.clone().ok_or_else(|| {
                RunnerError::Other(format!("missing post state for {}", self.name))
            })? {
                RootOrState::Root(_) => {
                    panic!("RootOrState::Root(_) not supported, for {}", self.name)
                }
                RootOrState::State(state) => state,
            };
        let post_state = update_post_state(post_state, self.test.pre.clone());

        for (address, expected_state) in post_state.iter() {
            // Storage
            for (k, v) in expected_state.storage.iter() {
                let actual = sequencer.get_storage_at(address, k.0)?;
                if actual != v.0 {
                    return Err(RunnerError::Other(format!(
                        "{} storage mismatch for {:#20x} at {:#32x}: expected {:#32x}, got {:#32x}",
                        self.name, address, k.0, v.0, actual
                    )));
                }
            }
            // Nonce
            let actual = sequencer.get_nonce_at(address)?;
            if actual != expected_state.nonce.0 {
                return Err(RunnerError::Other(format!(
                    "{} nonce mismatch for {:#20x}: expected {:#32x}, got {:#32x}",
                    self.name, address, expected_state.nonce.0, actual
                )));
            }
            // Bytecode
            let actual = sequencer.get_code_at(address)?;
            if actual != expected_state.code {
                return Err(RunnerError::Other(format!(
                    "{} code mismatch for {:#20x}: expected {:#x}, got {:#x}",
                    self.name, address, expected_state.code, actual
                )));
            }
            // Balance
            let mut actual = sequencer.get_balance_at(address)?;
            // Subtract transaction cost to sender balance
            if address.0 == sender_address {
                actual -= transaction_cost;
            }
            if actual != expected_state.balance.0 {
                return Err(RunnerError::Other(format!(
                    "{} balance mismatch for {:#20x}: expected {:#32x}, got {:#32x}",
                    self.name, address, expected_state.balance.0, actual
                )));
            }
        }

        Ok(())
    }
}

#[async_trait]
impl Case for BlockchainTestCase {
    /// A description of the test.
    fn description(&self) -> String {
        self.name.clone()
    }

    async fn run(&self) -> Result<(), RunnerError> {
        let sequencer = KakarotSequencer::new(SequencerState::default());
        let mut sequencer = sequencer.initialize()?;

        tracing::info!("Running test {}", self.name);

        self.handle_pre_state(&mut sequencer).await?;

        // handle transaction
        self.handle_transaction(&mut sequencer).await?;

        // handle post state
        self.handle_post_state(&mut sequencer).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ctor::ctor;
    use tracing_subscriber::{filter, FmtSubscriber};

    #[ctor]
    fn setup() {
        // Change this to "error" to see less output.
        let filter = filter::EnvFilter::new("ef_testing=info,sequencer=warn");
        let subscriber = FmtSubscriber::builder().with_env_filter(filter).finish();
        tracing::subscriber::set_global_default(subscriber)
            .expect("setting tracing default failed");
    }

    #[test]
    fn test_should_skip() {
        // Given
        let path = Path::new(
            "ethereum-tests/BlockchainTests/GeneralStateTests/VMTests/vmArithmeticTest/mulmod.json",
        );

        // When
        let should_skip = BlockchainTestCase::should_skip(path);

        // Then
        assert!(should_skip);
    }
}
