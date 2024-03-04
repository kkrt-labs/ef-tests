// Inspired by https://github.com/paradigmxyz/reth/tree/main/testing/ef-tests
use super::error::RunnerError;
use super::result::{extract_execution_retdata, log_execution_result};
use crate::evm_sequencer::constants::{
    CONTRACT_ACCOUNT_CLASS_HASH, EOA_CLASS_HASH, KAKAROT_ADDRESS, PROXY_CLASS_HASH,
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

use ethers_signers::{LocalWallet, Signer};
use reth_primitives::{sign_message, Address, SealedBlock, B256, U256};

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
        let wallet = LocalWallet::from_bytes(&self.secret_key.0)
            .map_err(|err| RunnerError::Other(vec![err.to_string()].into()))?;
        let sender_address = wallet.address().to_fixed_bytes();

        for (address, account) in self.pre.iter() {
            let is_eoa = address.0 == sender_address;
            let kakarot_account = KakarotAccount::new(
                address,
                &account.code,
                account.nonce,
                &account.storage.clone().into_iter().collect::<Vec<_>>()[..],
                is_eoa,
            )?;
            sequencer.setup_account(kakarot_account)?;
            sequencer.fund(address, account.balance)?;
        }

        Ok(())
    }

    fn handle_transaction(
        &self,
        sequencer: &mut KakarotSequencer,
    ) -> Result<Option<String>, RunnerError> {
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
        log_execution_result(&execution_result, &self.case_name, &self.case_category);

        let retdata = execution_result
            .map(extract_execution_retdata)
            .unwrap_or_default();

        Ok(retdata)
    }

    fn handle_post_state(
        &self,
        sequencer: &mut KakarotSequencer,
        retdata: Option<String>,
    ) -> Result<(), RunnerError> {
        let wallet = LocalWallet::from_bytes(&self.secret_key.0)
            .map_err(|err| RunnerError::Other(vec![err.to_string()].into()))?;
        let sender_address = wallet.address().to_fixed_bytes();

        let eth_validation_failed = retdata
            .map(|retdata| retdata == "Kakarot: eth validation failed")
            .unwrap_or_default();

        let maybe_block_header = self.block.block_header.as_ref();
        // Get gas used from block header
        let gas_used = maybe_block_header
            .map(|block_header| block_header.gas_used)
            .unwrap_or_default();

        // Get coinbase address
        let coinbase = maybe_block_header
            .map(|block_header| block_header.coinbase)
            .unwrap_or_default();

        // Get baseFeePerGas
        let base_fee_per_gas = maybe_block_header
            .and_then(|block_header| block_header.base_fee_per_gas)
            .unwrap_or_default();

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
        let gas_price = gas_price | effective_gas_price;
        let transaction_cost = gas_price * gas_used;

        let post_state = self.post.clone().expect("Post state not found");
        let post_state = update_post_state(post_state, self.pre.clone());

        let mut diff: Vec<String> = vec![];
        for (address, expected_state) in post_state.iter() {
            // Storage
            for (k, v) in expected_state.storage.iter() {
                let actual = sequencer.storage_at(address, *k)?;
                if actual != *v {
                    let storage_diff = format!(
                        "storage mismatch for {:#20x} at {:#32x}: expected {:#32x}, got {:#32x}",
                        address, k, v, actual
                    );
                    diff.push(storage_diff);
                }
            }

            // Nonce
            let mut actual = sequencer.nonce_at(address)?;
            // If the transaction failed during ethereum validation, performed in __execute__, the nonce is incremented but should not.
            // Substract 1 to the actual nonce.
            if eth_validation_failed && address.0 == sender_address {
                actual -= U256::from(1);
            }

            if actual != expected_state.nonce {
                let nonce_diff = format!(
                    "nonce mismatch for {:#20x}: expected {:#32x}, got {:#32x}",
                    address, expected_state.nonce, actual
                );
                diff.push(nonce_diff);
            }

            // Bytecode
            let actual = sequencer.code_at(address)?;
            if actual != expected_state.code {
                let bytecode_diff = format!(
                    "code mismatch for {:#20x}: expected {:#x}, got {:#x}",
                    address, expected_state.code, actual
                );
                diff.push(bytecode_diff);
            }

            // Balance
            let mut actual = sequencer.balance_at(address)?;
            // Subtract transaction cost to sender balance
            if address.0 == sender_address {
                actual -= transaction_cost;
            }
            // Add priority fee to coinbase balance
            if *address == coinbase {
                actual += (gas_price - base_fee_per_gas) * gas_used;
            }
            if actual != expected_state.balance {
                let balance_diff = format!(
                    "balance mismatch for {:#20x}: expected {:#32x}, got {:#32x}",
                    address, expected_state.balance, actual
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
        let maybe_block_header = self.block.block_header.as_ref();

        let coinbase_address = maybe_block_header.map(|b| b.coinbase).unwrap_or_default();

        let base_fee = maybe_block_header
            .and_then(|block_header| block_header.base_fee_per_gas)
            .unwrap_or_default();
        let prev_randao = maybe_block_header
            .map(|block_header| block_header.mix_hash)
            .unwrap_or_default();
        let block_gaslimit = maybe_block_header
            .map(|block_header| block_header.gas_limit)
            .unwrap_or_default();

        let block_number = maybe_block_header.map(|b| b.number).unwrap_or_default();
        let block_number = TryInto::<u64>::try_into(block_number).unwrap_or_default();

        let block_timestamp = maybe_block_header.map(|b| b.timestamp).unwrap_or_default();
        let block_timestamp = TryInto::<u64>::try_into(block_timestamp).unwrap_or_default();

        let kakarot_environment = KakarotEnvironment::new(
            *KAKAROT_ADDRESS,
            *PROXY_CLASS_HASH,
            *EOA_CLASS_HASH,
            *CONTRACT_ACCOUNT_CLASS_HASH,
        );
        let mut sequencer = KakarotSequencer::new(
            INITIAL_SEQUENCER_STATE.clone(),
            kakarot_environment,
            coinbase_address,
            *CHAIN_ID,
            block_number,
            block_timestamp,
        );

        sequencer.setup_state(base_fee, prev_randao, block_gaslimit)?;

        self.handle_pre_state(&mut sequencer)?;

        let retdata = self.handle_transaction(&mut sequencer)?;

        self.handle_post_state(&mut sequencer, retdata)?;
        Ok(())
    }
}
