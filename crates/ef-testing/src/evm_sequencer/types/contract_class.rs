use cairo_lang_casm::hints::Hint;
use cairo_lang_starknet::casm_contract_class::{
    CasmContractClass, CasmContractEntryPoint, CasmContractEntryPoints,
};
use cairo_lang_utils::bigint::BigUintAsHex;
use num_bigint::{BigUint, ParseBigIntError};
use num_traits::Num;
use starknet::core::types::contract::{CompiledClass, CompiledClassEntrypoint};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PrimeError {
    #[error("Failed to parse prime: {0}")]
    ParsingError(#[from] ParseBigIntError),
    #[error("Missing 0x prefix")]
    MissingPrefixError,
}

#[derive(Error, Debug)]
pub enum ContractClassConversionError {
    #[error("Failed to convert prime: {0}")]
    PrimeConversionError(#[from] PrimeError),
}

pub(crate) struct CasmContractClassWrapper(CasmContractClass);

impl From<CasmContractClassWrapper> for CasmContractClass {
    fn from(value: CasmContractClassWrapper) -> Self {
        value.0
    }
}

impl TryFrom<&CompiledClass> for CasmContractClassWrapper {
    type Error = ContractClassConversionError;

    fn try_from(value: &CompiledClass) -> Result<Self, Self::Error> {
        let prime = match &value.prime.strip_prefix("0x") {
            Some(prime) => BigUint::from_str_radix(prime, 16).map_err(PrimeError::ParsingError)?,
            None => {
                return Err(ContractClassConversionError::PrimeConversionError(
                    PrimeError::MissingPrefixError,
                ))
            }
        };

        let bytecode = value
            .bytecode
            .iter()
            .map(|b| BigUintAsHex {
                value: BigUint::from_bytes_be(&b.to_bytes_be()),
            })
            .collect();

        let hints = value
            .hints
            .iter()
            .map(|h| {
                (
                    h.id as usize,
                    Vec::from(&h.code[..])
                        .into_iter()
                        .filter_map(|hint| serde_json::from_value(hint).ok())
                        .collect::<Vec<Hint>>(),
                )
            })
            .collect();

        let entry_points_by_type = CasmContractEntryPoints {
            external: value
                .entry_points_by_type
                .external
                .iter()
                .map(compiled_class_entrypoint_to_casm_entrypoint)
                .collect(),
            l1_handler: value
                .entry_points_by_type
                .l1_handler
                .iter()
                .map(compiled_class_entrypoint_to_casm_entrypoint)
                .collect(),
            constructor: value
                .entry_points_by_type
                .constructor
                .iter()
                .map(compiled_class_entrypoint_to_casm_entrypoint)
                .collect(),
        };

        Ok(Self(CasmContractClass {
            prime,
            compiler_version: value.compiler_version.clone(),
            bytecode,
            hints,
            pythonic_hints: None,
            entry_points_by_type,
        }))
    }
}

fn compiled_class_entrypoint_to_casm_entrypoint(
    ep: &CompiledClassEntrypoint,
) -> CasmContractEntryPoint {
    CasmContractEntryPoint {
        selector: BigUint::from_bytes_be(&ep.selector.to_bytes_be()),
        offset: ep.offset as usize,
        builtins: ep.builtins.clone(),
    }
}
