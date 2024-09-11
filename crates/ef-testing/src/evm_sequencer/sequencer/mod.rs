use blockifier::bouncer::BouncerConfig;
use starknet::core::types::Felt;
use std::ops::{Deref, DerefMut};

use crate::evm_sequencer::types::felt::FeltSequencer;
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
use crate::evm_sequencer::{
    constants::{
        storage_variables::{
            ACCOUNT_PUBLIC_KEY, ERC20_BALANCES, KAKAROT_ACCOUNT_CONTRACT_CLASS_HASH,
            KAKAROT_BLOCK_GAS_LIMIT,
            KAKAROT_NATIVE_TOKEN_ADDRESS, KAKAROT_UNINITIALIZED_ACCOUNT_CLASS_HASH, OWNABLE_OWNER,
        },
        ACCOUNT_CONTRACT_CLASS, ACCOUNT_CONTRACT_CLASS_HASH, BLOCK_GAS_LIMIT, ETH_FEE_TOKEN_ADDRESS, FEE_TOKEN_CLASS, FEE_TOKEN_CLASS_HASH,
        KAKAROT_ADDRESS, KAKAROT_CLASS, KAKAROT_CLASS_HASH, KAKAROT_OWNER_ADDRESS,
        OPENZEPPELIN_ACCOUNT_CLASS, OPENZEPPELIN_ACCOUNT_CLASS_HASH, RELAYER_ADDRESS,
        RELAYER_BALANCE, RELAYER_VERIFYING_KEY, UNINITIALIZED_ACCOUNT_CLASS,
        UNINITIALIZED_ACCOUNT_CLASS_HASH, STRK_FEE_TOKEN_ADDRESS,
    },
    types::contract_class::CasmContractClassWrapper,
    utils::compute_starknet_address,
};

#[cfg(feature = "v0")]
use crate::evm_sequencer::constants::{CAIRO1_HELPERS_CLASS, CAIRO1_HELPERS_CLASS_HASH, storage_variables::KAKAROT_CAIRO1_HELPERS_CLASS_HASH};

use blockifier::abi::abi_utils::get_storage_var_address;
#[allow(unused_imports)]
use blockifier::state::state_api::{
    State as BlockifierState, StateReader as BlockifierStateReader,
};
use lazy_static::lazy_static;
use sequencer::state::State as SequencerState;

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
        let coinbase_constructor_args = {
            let evm_address: FeltSequencer = coinbase_address.try_into().unwrap(); // infallible
            vec![Felt::ONE, evm_address.into()]
        };

        let block_info = BlockInfo {
            block_number: BlockNumber(block_number),
            block_timestamp: BlockTimestamp(block_timestamp),
            sequencer_address: compute_starknet_address(
                &coinbase_address,
                environment.base_account_class_hash.0,
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
            chain_id: ChainId::Other(String::from_utf8(chain_id.to_be_bytes().to_vec()).unwrap()),
            fee_token_addresses: FeeTokenAddresses {
                eth_fee_token_address: *ETH_FEE_TOKEN_ADDRESS,
                strk_fee_token_address: *STRK_FEE_TOKEN_ADDRESS,
            },
        };

        let versioned_constants: VersionedConstants =
            serde_json::from_str(include_str!("./resources/versioned_constants.json"))
                .expect("failed to parse versioned constants");

        let bouncer_config = BouncerConfig::max();

        let block_context =
            BlockContext::new(block_info, chain_info, versioned_constants, bouncer_config);

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
        let chain_id = self.block_context().chain_info().chain_id.to_string();
        let chain_id = &chain_id.as_bytes()[..8];
        u64::from_be_bytes(chain_id.try_into().unwrap())
    }

    pub fn compute_starknet_address(&self, evm_address: &Address) -> StateResult<ContractAddress> {
        let base_class_hash = self.environment.base_account_class_hash.0;

        let constructor_args = {
            let evm_address: FeltSequencer = (*evm_address).try_into().unwrap(); // infallible
            vec![Felt::ONE, evm_address.into()]
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

lazy_static! {
    pub static ref INITIAL_SEQUENCER_STATE: SequencerState = {
        let mut state = SequencerState::default();

        // On KakarotZero we rely on the Cairo1 helpers class for unavailable syscalls and precompiles.
        #[cfg(feature = "v0")]
        let storage = vec![
            (OWNABLE_OWNER, *KAKAROT_OWNER_ADDRESS.0.key()),
            (KAKAROT_NATIVE_TOKEN_ADDRESS, *ETH_FEE_TOKEN_ADDRESS.0.key()),
            (KAKAROT_ACCOUNT_CONTRACT_CLASS_HASH, ACCOUNT_CONTRACT_CLASS_HASH.0),
            (KAKAROT_CAIRO1_HELPERS_CLASS_HASH, CAIRO1_HELPERS_CLASS_HASH.0),
            (KAKAROT_BLOCK_GAS_LIMIT, Felt::from(BLOCK_GAS_LIMIT)),
            (KAKAROT_UNINITIALIZED_ACCOUNT_CLASS_HASH, UNINITIALIZED_ACCOUNT_CLASS_HASH.0),
        ];

        #[cfg(not(feature = "v0"))]
        let storage = vec![
            (OWNABLE_OWNER, *KAKAROT_OWNER_ADDRESS.0.key()),
            (KAKAROT_NATIVE_TOKEN_ADDRESS, *ETH_FEE_TOKEN_ADDRESS.0.key()),
            (KAKAROT_ACCOUNT_CONTRACT_CLASS_HASH, ACCOUNT_CONTRACT_CLASS_HASH.0),
            (KAKAROT_BLOCK_GAS_LIMIT, Felt::from(BLOCK_GAS_LIMIT)),
            (KAKAROT_UNINITIALIZED_ACCOUNT_CLASS_HASH, UNINITIALIZED_ACCOUNT_CLASS_HASH.0),
        ];

        // Write all the storage vars to the sequencer state.
        for (k, v) in storage {
            (&mut state).set_storage_at(*KAKAROT_ADDRESS, get_storage_var_address(k, &[]), v).expect("failed to set storage");
        }

        // Convert the contract classes to using the util corresponding to the Cairo version it's compiled with.
        let (converted_kakarot_class, converted_account_class, converted_uninitialized_class) = {
            #[cfg(not(feature = "v1"))]
            {
                // Default to Cairo 0
                (
                    convert_contract_class_v0(&KAKAROT_CLASS).expect("failed to convert kakarot class"),
                    convert_contract_class_v0(&ACCOUNT_CONTRACT_CLASS).expect("failed to convert account class"),
                    convert_contract_class_v0(&UNINITIALIZED_ACCOUNT_CLASS).expect("failed to convert uninitialized class")
                )
            }
            #[cfg(feature = "v1")]
            {
                (
                    convert_contract_class_v1(&KAKAROT_CLASS).expect("failed to convert kakarot class"),
                    convert_contract_class_v1(&ACCOUNT_CONTRACT_CLASS).expect("failed to convert account class"),
                    convert_contract_class_v1(&UNINITIALIZED_ACCOUNT_CLASS).expect("failed to convert uninitialized class")
                )
            }
        };

        // Write the kakarot class and class hash.
        (&mut state).set_class_hash_at(*KAKAROT_ADDRESS, *KAKAROT_CLASS_HASH).expect("failed to set kakarot class hash");
        (&mut state)
            .set_contract_class(*KAKAROT_CLASS_HASH, converted_kakarot_class).expect("failed to set kakarot contract class");

        // Write contract account, uninitialized_account and erc20 classes and class hashes.
        (&mut state).set_contract_class(
            *ACCOUNT_CONTRACT_CLASS_HASH,
            converted_account_class,
        ).expect("failed to set contract account class");
        (&mut state)
            .set_contract_class(*UNINITIALIZED_ACCOUNT_CLASS_HASH, converted_uninitialized_class).expect("failed to set eoa contract class");

        (&mut state).set_contract_class(
            *FEE_TOKEN_CLASS_HASH,
            convert_contract_class_v0(&FEE_TOKEN_CLASS).expect("failed to convert FEE TOKEN CLASS to contract class"),
        ).expect("failed to set sequencer contract class");
        (&mut state).set_class_hash_at(*ETH_FEE_TOKEN_ADDRESS, *FEE_TOKEN_CLASS_HASH).expect("failed to set fee token class hash");

        #[cfg(feature = "v0")]
        (&mut state).set_contract_class(*CAIRO1_HELPERS_CLASS_HASH, convert_contract_class_v1(&CAIRO1_HELPERS_CLASS).expect("failed to convert CAIRO1_HELPERS Class to contract class")).expect("failed to set cairo1_helpers contract class");

        (&mut state).set_contract_class(
            *OPENZEPPELIN_ACCOUNT_CLASS_HASH,
            convert_contract_class_v0(&OPENZEPPELIN_ACCOUNT_CLASS).expect("failed to convert OPENZEPPELIN ACCOUNT CLASS to contract class")
        ).expect("failed to set openzeppelin account contract class");
        (&mut state).set_class_hash_at(*RELAYER_ADDRESS, *OPENZEPPELIN_ACCOUNT_CLASS_HASH).expect("failed to set relayer class hash");
        (&mut state).set_storage_at(*RELAYER_ADDRESS, get_storage_var_address(ACCOUNT_PUBLIC_KEY, &[]), RELAYER_VERIFYING_KEY.scalar()).expect("failed to set relayer public key");
        (&mut state).set_storage_at(*ETH_FEE_TOKEN_ADDRESS, get_storage_var_address(ERC20_BALANCES, &[*RELAYER_ADDRESS.0.key()]), *RELAYER_BALANCE).expect("failed to set relayer balance");

        state
    };
}
