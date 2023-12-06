// Inspired by https://github.com/paradigmxyz/reth/tree/main/testing/ef-tests
use super::error::RunnerError;
use super::result::log_execution_result;
use crate::evm_sequencer::evm_state::Evm;
use crate::evm_sequencer::sequencer::KakarotSequencer;
use crate::{
    evm_sequencer::{account::KakarotAccount, constants::CHAIN_ID},
    traits::Case,
    utils::update_post_state,
};
use async_trait::async_trait;
use ef_tests::models::Block;
use ef_tests::models::{RootOrState, State};

use ethers_signers::{LocalWallet, Signer};
use reth_primitives::{sign_message, SealedBlock};
use reth_rlp::Decodable as _;
use revm_primitives::B256;

#[derive(Debug)]
pub struct BlockchainTestCase {
    case_name: String,
    parent_dir: String,
    block: Block,
    pre: State,
    post: RootOrState,
    secret_key: B256,
}

// Division of logic:
// 'handle' methods attempt to abstract the data coming from BlockChainTestCase
// from more general logic that can be used across tests
impl BlockchainTestCase {
    pub const fn new(
        case_name: String,
        parent_dir: String,
        block: Block,
        pre: State,
        post: RootOrState,
        secret_key: B256,
    ) -> Self {
        Self {
            case_name,
            parent_dir,
            block,
            pre,
            post,
            secret_key,
        }
    }

    fn handle_pre_state(&self, sequencer: &mut KakarotSequencer) -> Result<(), RunnerError> {
        for (address, account) in self.pre.iter() {
            let kakarot_account = KakarotAccount::new(
                address,
                &account.code,
                account.nonce.0,
                account.storage.iter().map(|(k, v)| (k.0, v.0)).collect(),
            )?;
            sequencer.setup_account(kakarot_account)?;
            sequencer.fund(address, account.balance.0)?;
        }

        Ok(())
    }

    fn handle_transaction(&self, sequencer: &mut KakarotSequencer) -> Result<(), RunnerError> {
        // we extract the transaction from the block
        let block = &self.block;
        let block =
            SealedBlock::decode(&mut block.rlp.as_ref()).map_err(RunnerError::RlpDecodeError)?;

        // Encode body as transaction
        let mut tx_signed = block.body.first().cloned().ok_or_else(|| {
            RunnerError::Other(vec!["No transaction in pre state block".into()].into())
        })?;

        tx_signed.transaction.set_chain_id(*CHAIN_ID);
        let signature = sign_message(self.secret_key, tx_signed.signature_hash())
            .map_err(|err| RunnerError::Other(vec![err.to_string()].into()))?;

        tx_signed.signature = signature;

        let execution_result = sequencer.execute_transaction(tx_signed);
        log_execution_result(execution_result, &self.case_name, &self.parent_dir);

        Ok(())
    }

    fn handle_post_state(&self, sequencer: &mut KakarotSequencer) -> Result<(), RunnerError> {
        let wallet = LocalWallet::from_bytes(&self.secret_key.0)
            .map_err(|err| RunnerError::Other(vec![err.to_string()].into()))?;
        let sender_address = wallet.address().to_fixed_bytes();

        let maybe_block_header = self.block.block_header.as_ref();
        // Get gas used from block header
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
        let maybe_transaction = self
            .block
            .transactions
            .as_ref()
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

        let post_state = match self.post.clone() {
            RootOrState::Root(_) => {
                panic!("RootOrState::Root(_) not supported")
            }
            RootOrState::State(state) => state,
        };
        let post_state = update_post_state(post_state, self.pre.clone());

        let mut diff: Vec<String> = vec![];
        for (address, expected_state) in post_state.iter() {
            // Storage
            for (k, v) in expected_state.storage.iter() {
                let actual = sequencer.get_storage_at(address, k.0)?;
                if actual != v.0 {
                    let storage_diff = format!(
                        "storage mismatch for {:#20x} at {:#32x}: expected {:#32x}, got {:#32x}",
                        address, k.0, v.0, actual
                    );
                    diff.push(storage_diff);
                }
            }
            // Nonce
            let actual = sequencer.get_nonce_at(address)?;
            if actual != expected_state.nonce.0 {
                let nonce_diff = format!(
                    "nonce mismatch for {:#20x}: expected {:#32x}, got {:#32x}",
                    address, expected_state.nonce.0, actual
                );
                diff.push(nonce_diff);
            }
            // Bytecode
            let actual = sequencer.get_code_at(address)?;
            if actual != expected_state.code {
                let bytecode_diff = format!(
                    "code mismatch for {:#20x}: expected {:#x}, got {:#x}",
                    address, expected_state.code, actual
                );
                diff.push(bytecode_diff);
            }
            // Balance
            let mut actual = sequencer.get_balance_at(address)?;
            // Subtract transaction cost to sender balance
            if address.0 == sender_address {
                actual -= transaction_cost;
            }
            if actual != expected_state.balance.0 {
                let balance_diff = format!(
                    "balance mismatch for {:#20x}: expected {:#32x}, got {:#32x}",
                    address, expected_state.balance.0, actual
                );
                diff.push(balance_diff);
            }
        }

        if !diff.is_empty() {
            return Err(RunnerError::Other(diff.into()));
        }

        Ok(())
    }
}

#[async_trait]
impl Case for BlockchainTestCase {
    fn run(&self) -> Result<(), RunnerError> {
        let mut sequencer = KakarotSequencer::new();

        self.handle_pre_state(&mut sequencer)?;

        self.handle_transaction(&mut sequencer)?;

        self.handle_post_state(&mut sequencer)?;
        Ok(())
    }
}
