#[cfg(feature = "v0")]
pub mod v0;
#[cfg(feature = "v1")]
pub mod v1;

use std::{
    ops::{Deref, DerefMut},
    sync::Arc,
};

use blockifier::block_context::{BlockContext, FeeTokenAddresses, GasPrices};
use reth_primitives::Address;
use sequencer::{sequencer::Sequencer, state::State};
use starknet_api::{
    block::{BlockNumber, BlockTimestamp},
    core::ChainId,
};

use super::{
    constants::{CHAIN_ID, ETH_FEE_TOKEN_ADDRESS, STRK_FEE_TOKEN_ADDRESS, VM_RESOURCES},
    utils::compute_starknet_address,
};

/// Kakarot wrapper around a sequencer.
#[derive(Clone)]
pub(crate) struct KakarotSequencer(Sequencer<State, Address>);

impl KakarotSequencer {
    pub fn new(coinbase_address: Address, block_number: u64, block_timestamp: u64) -> Self {
        let initial_state = {
            #[cfg(feature = "v0")]
            {
                v0::INITIAL_SEQUENCER_STATE.clone()
            }
            #[cfg(feature = "v1")]
            {
                v1::INITIAL_SEQUENCER_STATE.clone()
            }
            #[cfg(not(any(feature = "v0", feature = "v1")))]
            {
                State::default()
            }
        };

        let block_context = BlockContext {
            chain_id: ChainId(String::from_utf8(CHAIN_ID.to_be_bytes().to_vec()).unwrap()),
            block_number: BlockNumber(block_number),
            block_timestamp: BlockTimestamp(block_timestamp),
            sequencer_address: compute_starknet_address(&coinbase_address)
                .try_into()
                .expect("Failed to convert coinbase address to contract address"),
            fee_token_addresses: FeeTokenAddresses {
                eth_fee_token_address: *ETH_FEE_TOKEN_ADDRESS,
                strk_fee_token_address: *STRK_FEE_TOKEN_ADDRESS,
            },
            vm_resource_fee_cost: Arc::new(VM_RESOURCES.clone()),
            gas_prices: GasPrices {
                eth_l1_gas_price: 1,
                strk_l1_gas_price: 1,
            },
            invoke_tx_max_n_steps: 50_000_000,
            validate_max_n_steps: 50_000_000,
            max_recursion_depth: 8192,
        };
        let sequencer = Sequencer::new(block_context, initial_state, coinbase_address);
        Self(sequencer)
    }
}

impl Deref for KakarotSequencer {
    type Target = Sequencer<State, Address>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for KakarotSequencer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
