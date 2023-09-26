// This file has been taken from katana -> https://github.com/dojoengine/dojo/blob/main/crates/katana/core/src/db/serde/utils.rs

use anyhow::Result;
use blockifier::execution::contract_class::{ContractClass, ContractClassV0};
use cairo_lang_starknet::casm_contract_class::CasmContractClass;
use starknet::core::types::{FieldElement, FlattenedSierraClass};

pub fn rpc_to_inner_class(
    contract_class: &FlattenedSierraClass,
) -> Result<(FieldElement, ContractClass)> {
    let class_hash = contract_class.class_hash();
    let contract_class = rpc_to_cairo_contract_class(contract_class)?;
    let casm_contract = CasmContractClass::from_contract_class(contract_class, true)?;
    Ok((class_hash, ContractClass::V1(casm_contract.try_into()?)))
}

/// Converts `starknet-rs` RPC [FlattenedSierraClass] type to Cairo's
/// [ContractClass](cairo_lang_starknet::contract_class::ContractClass) type.
pub fn rpc_to_cairo_contract_class(
    contract_class: &FlattenedSierraClass,
) -> Result<cairo_lang_starknet::contract_class::ContractClass> {
    let value = serde_json::to_value(contract_class)?;

    Ok(cairo_lang_starknet::contract_class::ContractClass {
        abi: serde_json::from_value(value["abi"].clone()).ok(),
        sierra_program: serde_json::from_value(value["sierra_program"].clone())?,
        entry_points_by_type: serde_json::from_value(value["entry_points_by_type"].clone())?,
        contract_class_version: serde_json::from_value(value["contract_class_version"].clone())?,
        sierra_program_debug_info: serde_json::from_value(
            value["sierra_program_debug_info"].clone(),
        )
        .ok(),
    })
}

pub fn get_contract_class(contract_class_str: &str) -> ContractClass {
    let legacy_contract_class: ContractClassV0 = serde_json::from_str(contract_class_str).unwrap();
    ContractClass::V0(legacy_contract_class)
}
