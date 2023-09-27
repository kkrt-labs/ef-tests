pub mod constants;
pub mod setup;
pub mod types;
pub mod utils;

use std::collections::HashMap;
use std::sync::Arc;

use blockifier::block_context::BlockContext;
use reth_primitives::Address;
use sequencer::sequencer::Sequencer;
use sequencer::state::State;
use starknet::core::utils::get_contract_address;
use starknet_api::block::{BlockNumber, BlockTimestamp};
use starknet_api::core::ChainId;

use self::constants::{FEE_TOKEN_ADDRESS, KAKAROT_ADDRESS, PROXY_CLASS_HASH, SEQUENCER_ADDRESS};
use self::types::FeltSequencer;

pub(crate) struct KakarotSequencer(Sequencer<State>);

#[allow(dead_code)]
impl KakarotSequencer {
    pub fn new(state: State) -> Self {
        let sequencer = Sequencer::new(Self::default_kakarot_block_context(), state);
        Self(sequencer)
    }

    fn default_kakarot_block_context() -> BlockContext {
        BlockContext {
            chain_id: ChainId("KKRT".into()),
            block_number: BlockNumber(0),
            block_timestamp: BlockTimestamp(0),
            sequencer_address: *SEQUENCER_ADDRESS,
            fee_token_address: *FEE_TOKEN_ADDRESS,
            vm_resource_fee_cost: Arc::new(Self::default_vm_resource_fee_cost()),
            gas_price: 1,
            invoke_tx_max_n_steps: 2u32.pow(24),
            validate_max_n_steps: 2u32.pow(24),
            max_recursion_depth: 1024,
        }
    }

    fn default_vm_resource_fee_cost() -> HashMap<String, f64> {
        [
            (String::from("n_steps"), 1_f64),
            ("pedersen_builtin".to_string(), 1_f64),
            ("range_check_builtin".to_string(), 1_f64),
            ("ecdsa_builtin".to_string(), 1_f64),
            ("bitwise_builtin".to_string(), 1_f64),
            ("poseidon_builtin".to_string(), 1_f64),
            ("output_builtin".to_string(), 1_f64),
            ("ec_op_builtin".to_string(), 1_f64),
            ("keccak_builtin".to_string(), 1_f64),
            ("segment_arena_builtin".to_string(), 1_f64),
        ]
        .into_iter()
        .collect()
    }

    pub fn compute_starknet_address(&self, evm_address: &Address) -> FeltSequencer {
        let evm_address: FeltSequencer = (*evm_address).into();
        let starknet_address = get_contract_address(
            evm_address.into(),
            PROXY_CLASS_HASH.0.into(),
            &[],
            (*KAKAROT_ADDRESS.0.key()).into(),
        );
        starknet_address.into()
    }
}
