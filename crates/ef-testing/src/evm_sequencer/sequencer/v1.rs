use blockifier::{
    abi::abi_utils::get_storage_var_address,
    execution::contract_class::{ContractClass, ContractClassV0, ContractClassV1},
    state::state_api::State as BlockifierState,
};
use cairo_lang_starknet::casm_contract_class::CasmContractClass;
use cairo_vm::types::errors::program_errors::ProgramError;
use starknet::core::types::contract::CompiledClass;
use starknet_api::hash::StarkFelt;

use crate::evm_sequencer::{
    constants::{
        kkrt_constants_v1::{
            CONTRACT_ACCOUNT_CLASS, CONTRACT_ACCOUNT_CLASS_HASH, DEPLOY_FEE, EOA_CLASS,
            EOA_CLASS_HASH, KAKAROT_CLASS, KAKAROT_CLASS_HASH, UNINITIALIZED_ACCOUNT_CLASS,
            UNINITIALIZED_ACCOUNT_CLASS_HASH,
        },
        CHAIN_ID, ETH_FEE_TOKEN_ADDRESS, FEE_TOKEN_CLASS, FEE_TOKEN_CLASS_HASH, KAKAROT_ADDRESS,
        KAKAROT_OWNER_ADDRESS,
    },
    types::contract_class::CasmContractClassWrapper,
    InitializationError, InitializationResult,
};

use super::{InitializeSequencer, KakarotSequencer};

impl InitializeSequencer for KakarotSequencer {
    fn initialize(mut self) -> InitializationResult<Self>
    where
        Self: Sized,
    {
        let storage = [
            ("owner", *KAKAROT_OWNER_ADDRESS.0.key()),
            ("chain_id", StarkFelt::from(*CHAIN_ID)),
            ("native_token", *ETH_FEE_TOKEN_ADDRESS.0.key()),
            ("deploy_fee", *DEPLOY_FEE),
            ("ca_class_hash", CONTRACT_ACCOUNT_CLASS_HASH.0),
            ("eoa_class_hash", EOA_CLASS_HASH.0),
            ("account_class_hash", UNINITIALIZED_ACCOUNT_CLASS_HASH.0),
        ];

        // Write all the storage vars to the sequencer state.
        for (k, v) in storage {
            (&mut self.0.state).set_storage_at(
                *KAKAROT_ADDRESS,
                get_storage_var_address(k, &[]),
                v,
            );
        }

        // Write the kakarot class and class hash.
        (&mut self.0.state).set_class_hash_at(*KAKAROT_ADDRESS, *KAKAROT_CLASS_HASH)?;
        (&mut self.0.state)
            .set_contract_class(&KAKAROT_CLASS_HASH, convert_contract_class(&KAKAROT_CLASS)?)?;

        // Write eoa, contract account and uninitialized account.
        (&mut self.0.state).set_contract_class(
            &CONTRACT_ACCOUNT_CLASS_HASH,
            convert_contract_class(&CONTRACT_ACCOUNT_CLASS)?,
        )?;
        (&mut self.0.state)
            .set_contract_class(&EOA_CLASS_HASH, convert_contract_class(&EOA_CLASS)?)?;
        (&mut self.0.state).set_contract_class(
            &UNINITIALIZED_ACCOUNT_CLASS_HASH,
            convert_contract_class(&UNINITIALIZED_ACCOUNT_CLASS)?,
        )?;

        (&mut self.0.state).set_contract_class(
            &FEE_TOKEN_CLASS_HASH,
            ContractClass::V0(ContractClassV0::try_from_json_string(
                &serde_json::to_string(&*FEE_TOKEN_CLASS).map_err(ProgramError::Parse)?,
            )?),
        )?;
        (&mut self.0.state).set_class_hash_at(*ETH_FEE_TOKEN_ADDRESS, *FEE_TOKEN_CLASS_HASH)?;

        Ok(self)
    }
}

fn convert_contract_class(class: &CompiledClass) -> Result<ContractClass, InitializationError> {
    let casm_contract_class = CasmContractClassWrapper::try_from(class)?;
    let casm_contract_class: CasmContractClass = casm_contract_class.into();
    Result::<ContractClass, InitializationError>::Ok(ContractClass::V1(ContractClassV1::try_from(
        casm_contract_class,
    )?))
}
