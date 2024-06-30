#[cfg(feature = "v0")]
pub mod v0;
#[cfg(feature = "v1")]
pub mod v1;

use blockifier::bouncer::BouncerConfig;
#[cfg(feature = "v0")]
pub use v0::INITIAL_SEQUENCER_STATE;

#[cfg(feature = "v1")]
pub use v1::INITIAL_SEQUENCER_STATE;

#[cfg(not(any(feature = "v0", feature = "v1")))]
use lazy_static::lazy_static;
#[cfg(not(any(feature = "v0", feature = "v1")))]
lazy_static! {
    pub static ref INITIAL_SEQUENCER_STATE: State = State::default();
}

use std::ops::{Deref, DerefMut};

use blockifier::blockifier::block::{BlockInfo, GasPrices};
use blockifier::context::ChainInfo;
use blockifier::context::{BlockContext, FeeTokenAddresses};
use blockifier::versioned_constants::VersionedConstants;
use blockifier::{
    execution::contract_class::{ContractClass, ContractClassV0, ContractClassV1},
    state::state_api::StateResult,
};
use cairo_lang_starknet_classes::casm_contract_class::CasmContractClass;
use cairo_vm::types::errors::program_errors::ProgramError;
use reth_primitives::Address;
use sequencer::{sequencer::Sequencer, state::State};
use starknet::core::types::contract::{legacy::LegacyContractClass, CompiledClass};
use starknet_api::{
    block::{BlockNumber, BlockTimestamp},
    core::{ChainId, ClassHash, ContractAddress},
};
use std::num::NonZeroU128;

use super::{
    constants::{ETH_FEE_TOKEN_ADDRESS, STRK_FEE_TOKEN_ADDRESS},
    types::contract_class::CasmContractClassWrapper,
    utils::compute_starknet_address,
};

/// Kakarot wrapper around a sequencer.
#[derive(Clone)]
pub struct KakarotSequencer {
    sequencer: Sequencer<State, Address>,
    pub(crate) environment: KakarotEnvironment,
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct KakarotEnvironment {
    /// The address of the Kakarot contract.
    pub(crate) kakarot_address: ContractAddress,
    /// The class hash of the base account contract.
    /// This is the uninitialized account class in v1.
    pub(crate) base_account_class_hash: ClassHash,
    /// The class hash of the cairo1 helpers class.
    pub(crate) cairo1_helpers_class_hash: ClassHash,
    pub(crate) account_contract_class_hash: ClassHash,
}

impl KakarotEnvironment {
    pub const fn new(
        kakarot_address: ContractAddress,
        base_account_class_hash: ClassHash,
        cairo1_helpers_class_hash: ClassHash,
        account_contract_class_hash: ClassHash,
    ) -> Self {
        Self {
            kakarot_address,
            base_account_class_hash,
            cairo1_helpers_class_hash,
            account_contract_class_hash,
        }
    }
}

impl KakarotSequencer {
    pub fn new(
        initial_state: State,
        environment: KakarotEnvironment,
        coinbase_address: Address,
        chain_id: u64,
        block_number: u64,
        block_timestamp: u64,
    ) -> Self {
        let kakarot_address = (*environment.kakarot_address.0.key()).into();
        let coinbase_constructor_args = {
            use crate::evm_sequencer::types::felt::FeltSequencer;
            let evm_address: FeltSequencer = coinbase_address.try_into().unwrap(); // infallible
            vec![kakarot_address, evm_address.into()]
        };

        let block_info = BlockInfo {
            block_number: BlockNumber(block_number),
            block_timestamp: BlockTimestamp(block_timestamp),
            sequencer_address: compute_starknet_address(
                &coinbase_address,
                environment.base_account_class_hash.0.into(),
                &coinbase_constructor_args,
            )
            .try_into()
            .expect("Failed to convert to ContractAddress"),
            gas_prices: GasPrices {
                eth_l1_gas_price: NonZeroU128::new(1).unwrap(),
                strk_l1_gas_price: NonZeroU128::new(1).unwrap(),
                eth_l1_data_gas_price: NonZeroU128::new(1).unwrap(),
                strk_l1_data_gas_price: NonZeroU128::new(1).unwrap(),
            },
            use_kzg_da: false,
        };

        let chain_info = ChainInfo {
            chain_id: ChainId(String::from_utf8(chain_id.to_be_bytes().to_vec()).unwrap()),
            fee_token_addresses: FeeTokenAddresses {
                eth_fee_token_address: *ETH_FEE_TOKEN_ADDRESS,
                strk_fee_token_address: *STRK_FEE_TOKEN_ADDRESS,
            },
        };

        // let versioned_constants = VersionedConstants {
        //     tx_event_limits: Default::default(),
        //     invoke_tx_max_n_steps: 50_000_000,
        //     l2_resource_gas_costs: Default::default(),
        //     max_recursion_depth: 8192,
        //     validate_max_n_steps: 50_000_000,
        //     os_constants: Arc::new(Default::default()),
        //     os_resources: Arc::new(Default::default()),
        //     vm_resource_fee_cost: Arc::new(VM_RESOURCES.clone()),
        // };

        // let versioned_constants = VersionedConstants::latest_constants().clone();

        let versioned_constants: VersionedConstants =
            serde_json::from_str(include_str!("./resources/versioned_constants.json"))
                .expect("failed to parse versioned constants");

        let bouncer_config = BouncerConfig::max();

        let concurrency_mode = Default::default();

        let block_context = BlockContext::new(
            block_info,
            chain_info,
            versioned_constants,
            bouncer_config,
            concurrency_mode,
        );

        // let block_context = BlockContext {
        //     block_info: BlockInfo {
        //         block_number: BlockNumber(block_number),
        //         block_timestamp: BlockTimestamp(block_timestamp),
        //         sequencer_address: compute_starknet_address(
        //             &coinbase_address,
        //             environment.base_account_class_hash.0.into(),
        //             &coinbase_constructor_args,
        //         )
        //         .try_into()
        //         .expect("Failed to convert to ContractAddress"),
        //         vm_resource_fee_cost: Arc::new(VM_RESOURCES.clone()),
        //         gas_prices: GasPrices {
        //             eth_l1_gas_price: 1,
        //             strk_l1_gas_price: 1,
        //             eth_l1_data_gas_price: 1,
        //             strk_l1_data_gas_price: 1,
        //         },
        //         use_kzg_da: false,
        //         invoke_tx_max_n_steps: 50_000_000,
        //         validate_max_n_steps: 50_000_000,
        //         max_recursion_depth: 8192,
        //     },
        //     chain_info: ChainInfo {
        //         chain_id: ChainId(String::from_utf8(chain_id.to_be_bytes().to_vec()).unwrap()),
        //         fee_token_addresses: FeeTokenAddresses {
        //             eth_fee_token_address: *ETH_FEE_TOKEN_ADDRESS,
        //             strk_fee_token_address: *STRK_FEE_TOKEN_ADDRESS,
        //         },
        //     },
        //     versioned_constants: VersionedConstants {
        //         tx_event_limits: Default::default(),
        //         invoke_tx_max_n_steps: 50_000_000,
        //         l2_resource_gas_costs: Default::default(),
        //         max_recursion_depth: 8192,
        //         validate_max_n_steps: 50_000_000,
        //     },
        //     bouncer_config: Default::default(),
        //     concurrency_mode: Default::default(),
        // };

        let sequencer = Sequencer::new(block_context, initial_state, coinbase_address);
        Self {
            sequencer,
            environment,
        }
    }

    pub const fn sequencer(&self) -> &Sequencer<State, Address> {
        &self.sequencer
    }

    pub const fn environment(&self) -> &KakarotEnvironment {
        &self.environment
    }

    pub fn chain_id(&self) -> u64 {
        // Safety: chain_id is always 8 bytes.
        let chain_id = &self.block_context().chain_info().chain_id.0.as_bytes()[..8];
        u64::from_be_bytes(chain_id.try_into().unwrap())
    }

    pub fn compute_starknet_address(&self, evm_address: &Address) -> StateResult<ContractAddress> {
        let kakarot_address = (*self.environment.kakarot_address.0.key()).into();
        let base_class_hash = self.environment.base_account_class_hash.0.into();

        let constructor_args = {
            use crate::evm_sequencer::types::felt::FeltSequencer;
            let evm_address: FeltSequencer = (*evm_address).try_into().unwrap(); // infallible
            vec![kakarot_address, evm_address.into()]
        };

        Ok(compute_starknet_address(evm_address, base_class_hash, &constructor_args).try_into()?)
    }
}

impl Deref for KakarotSequencer {
    type Target = Sequencer<State, Address>;

    fn deref(&self) -> &Self::Target {
        &self.sequencer
    }
}

impl DerefMut for KakarotSequencer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.sequencer
    }
}

pub fn convert_contract_class_v0(
    class: &LegacyContractClass,
) -> Result<ContractClass, eyre::Error> {
    Result::<ContractClass, eyre::Error>::Ok(ContractClass::V0(
        ContractClassV0::try_from_json_string(
            &serde_json::to_string(class).map_err(ProgramError::Parse)?,
        )?,
    ))
}

pub fn convert_contract_class_v1(class: &CompiledClass) -> Result<ContractClass, eyre::Error> {
    let casm_contract_class = CasmContractClassWrapper::try_from(class)?;
    let casm_contract_class: CasmContractClass = casm_contract_class.into();
    Ok(ContractClass::V1(ContractClassV1::try_from(
        casm_contract_class,
    )?))
}
