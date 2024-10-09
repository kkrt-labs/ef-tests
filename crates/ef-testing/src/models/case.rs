// Inspired by https://github.com/paradigmxyz/reth/tree/main/testing/ef-tests
use super::error::RunnerError;
use super::result::{extract_output_and_log_execution_result, EVMOutput};
use crate::evm_sequencer::constants::{
    ACCOUNT_CONTRACT_CLASS_HASH, BEACON_ROOT_ADDRESS, CAIRO1_HELPERS_CLASS_HASH, KAKAROT_ADDRESS,
    UNINITIALIZED_ACCOUNT_CLASS_HASH,
};
use crate::evm_sequencer::evm_state::Evm;
use crate::evm_sequencer::sequencer::{
    KakarotEnvironment, KakarotSequencer, INITIAL_SEQUENCER_STATE,
};
use crate::{
    evm_sequencer::{account::KakarotAccount, constants::CHAIN_ID},
    traits::Case,
    utils::update_post_state,
};
use alloy_rlp::Decodable as _;
use async_trait::async_trait;
use ef_tests::models::Account;
use ef_tests::models::Block;
use ef_tests::models::State;
use std::collections::BTreeMap;

use alloy_primitives::{Address, B256, U256};
use reth_primitives::{sign_message, SealedBlock};

#[derive(Debug)]
pub struct BlockchainTestCase {
    case_name: String,
    case_category: String,
    block: Block,
    pre: State,
    post: Option<BTreeMap<Address, Account>>,
    secret_key: B256,
}

// Division of logic:
// 'handle' methods attempt to abstract the data coming from BlockChainTestCase
// from more general logic that can be used across tests
impl BlockchainTestCase {
    pub const fn new(
        case_name: String,
        case_category: String,
        block: Block,
        pre: State,
        post: Option<BTreeMap<Address, Account>>,
        secret_key: B256,
    ) -> Self {
        Self {
            case_name,
            case_category,
            block,
            pre,
            post,
            secret_key,
        }
    }

    fn handle_pre_state(&self, sequencer: &mut KakarotSequencer) -> Result<(), RunnerError> {
        for (address, account) in self.pre.iter() {
            let kakarot_account = KakarotAccount::new(address, account.clone())?;
            sequencer.setup_account(kakarot_account)?;
            sequencer.fund(address, account.balance)?;
        }

        Ok(())
    }

    fn handle_transaction(
        &self,
        sequencer: &mut KakarotSequencer,
    ) -> Result<EVMOutput, RunnerError> {
        // we extract the transactions from the block
        let block = SealedBlock::decode(&mut self.block.rlp.as_ref())
            .map_err(RunnerError::RlpDecodeError)?;

        let mut output = EVMOutput::default();

        // Iterate over all transactions in the block
        for tx in block.body.transactions.iter() {
            // Encode body as transaction
            let mut tx_signed = tx.clone();
            tx_signed.transaction.set_chain_id(CHAIN_ID);
            // TODO: this will not support blocks with transactions from different senders (different secret key)
            let signature = sign_message(self.secret_key, tx_signed.signature_hash())
                .map_err(|err| RunnerError::Other(vec![err.to_string()].into()))?;
            tx_signed.signature = signature;

            let execution_result = sequencer.execute_transaction(tx_signed);

            // Update the output with the execution result of the current transaction
            let tx_output = extract_output_and_log_execution_result(
                &execution_result,
                &self.case_name,
                &self.case_category,
            )
            .unwrap_or_default();

            output.merge(&tx_output);
        }

        Ok(output)
    }

    fn handle_post_state(
        &self,
        sequencer: &mut KakarotSequencer,
        output: EVMOutput,
    ) -> Result<(), RunnerError> {
        let maybe_revert_reason = String::from_utf8(output.return_data.as_slice().to_vec());

        // Get gas_used and base_fee from RLP block - as in some cases, the block header is not present in the test data.
        let sealed_block = SealedBlock::decode(&mut self.block.rlp.as_ref())
            .map_err(RunnerError::RlpDecodeError)?;
        let sealed_header = sealed_block.header.unseal();

        let base_fee_per_gas: U256 = U256::from(sealed_header.base_fee_per_gas.unwrap_or_default());

        let expected_gas_used = U256::from(sealed_header.gas_used);

        // Get gas price from transaction
        let maybe_transaction = self
            .block
            .transactions
            .as_ref()
            .and_then(|transactions| transactions.first());
        let gas_price = maybe_transaction
            .and_then(|transaction| transaction.gas_price)
            .unwrap_or_default();
        let max_priority_fee_per_gas = maybe_transaction
            .and_then(|transaction| transaction.max_priority_fee_per_gas)
            .unwrap_or_default();
        let effective_gas_price = maybe_transaction
            .and_then(|transaction| transaction.max_fee_per_gas)
            .map(|max_fee_per_gas| {
                max_priority_fee_per_gas.min(max_fee_per_gas - base_fee_per_gas) + base_fee_per_gas
            })
            .unwrap_or_default();
        // <https://eips.ethereum.org/EIPS/eip-1559>: priority fee is capped because the base fee is filled first
        if gas_price != U256::ZERO && effective_gas_price != U256::ZERO {
            return Err(RunnerError::Other(
                vec!["max_fee_per_gas and gas_price are both set".to_string()].into(),
            ));
        }
        let post_state = self.post.clone().expect("Post state not found");
        let post_state = update_post_state(post_state, self.pre.clone());

        let mut errors = Vec::new();

        let actual_gas_used = output.gas_used;
        let expected_gas_u64: u64 = expected_gas_used.try_into().unwrap();
        if expected_gas_u64 != actual_gas_used {
            errors.push(format!(
                "gas used mismatch: expected {expected_gas_u64}, got {actual_gas_used}"
            ));
        }

        for (address, expected_state) in post_state.iter() {
            //TODO: this should not be a part of the post-state of EF-Tests and can
            // be removed once we base ourself on the next EF-Tests release, which fixes this issue
            // Beacon-related features are not supported in Kakarot
            if *address == BEACON_ROOT_ADDRESS {
                continue;
            }

            // Storage
            for (k, v) in expected_state.storage.iter() {
                let actual = sequencer.storage_at(address, *k)?;
                if actual != *v {
                    let storage_diff = format!(
                        "storage mismatch for {:#20x} at {:#32x}: expected {:#32x}, got {:#32x}",
                        address, k, v, actual
                    );
                    errors.push(storage_diff);
                }
            }

            // Nonce
            let actual = sequencer.nonce_at(address)?;
            if actual != expected_state.nonce {
                let nonce_diff = format!(
                    "nonce mismatch for {:#20x}: expected {:#32x}, got {:#32x}",
                    address, expected_state.nonce, actual
                );
                errors.push(nonce_diff);
            }

            // Bytecode
            let actual = sequencer.code_at(address)?;
            if actual != expected_state.code {
                let bytecode_diff = format!(
                    "code mismatch for {:#20x}: expected {:#x}, got {:#x}",
                    address, expected_state.code, actual
                );
                errors.push(bytecode_diff);
            }

            // Balance

            #[cfg(feature = "v0")]
            //TODO Charging fees is not enabled yet for SSJ
            {
                let coinbase = sealed_header.beneficiary;
                let mut actual = sequencer.balance_at(address)?;
                // Our coinbase should receive all of the txs fees, not only the priority fee.
                if *address == coinbase {
                    actual -= base_fee_per_gas * expected_gas_used;
                }
                if actual != expected_state.balance {
                    let balance_diff = format!(
                        "balance mismatch for {:#20x}: expected {:#32x}, got {:#32x}",
                        address, expected_state.balance, actual
                    );
                    errors.push(balance_diff);
                }
            }
        }

        if !errors.is_empty() {
            if let Ok(revert_reason) = maybe_revert_reason {
                errors.push(format!("revert reason: {}", revert_reason));
            }
            return Err(RunnerError::Other(errors.into()));
        }

        Ok(())
    }
}

#[async_trait]
impl Case for BlockchainTestCase {
    fn run(&self) -> Result<(), RunnerError> {
        // Get gas_used and base_fee from RLP block - as in some cases, the block header is not present in the test data.
        let sealed_block = SealedBlock::decode(&mut self.block.rlp.as_ref())
            .map_err(RunnerError::RlpDecodeError)?;
        let sealed_header = sealed_block.header.clone().unseal();

        let coinbase_address = sealed_header.beneficiary;

        let prev_randao: U256 = sealed_header.mix_hash.into();
        let base_fee = U256::from(sealed_header.base_fee_per_gas.unwrap_or_default());
        let block_gas_limit = U256::from(sealed_header.gas_limit);
        let block_number = U256::from(sealed_header.number);
        let block_timestamp = U256::from(sealed_block.timestamp);

        let kakarot_environment = KakarotEnvironment::new(
            *KAKAROT_ADDRESS,
            *UNINITIALIZED_ACCOUNT_CLASS_HASH,
            *CAIRO1_HELPERS_CLASS_HASH,
            *ACCOUNT_CONTRACT_CLASS_HASH,
        );
        let mut sequencer = KakarotSequencer::new(
            INITIAL_SEQUENCER_STATE.clone(),
            kakarot_environment,
            coinbase_address,
            CHAIN_ID,
            block_number.try_into().unwrap_or_default(),
            block_timestamp.try_into().unwrap_or_default(),
        );

        sequencer.setup_state(base_fee, prev_randao, block_gas_limit)?;

        self.handle_pre_state(&mut sequencer)?;

        let output = self.handle_transaction(&mut sequencer)?;

        self.handle_post_state(&mut sequencer, output)?;
        Ok(())
    }
}
