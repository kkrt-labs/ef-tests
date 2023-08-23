use kakarot_rpc_core::client::helpers::split_u256_into_field_elements;
use reth_primitives::U128;
use revm_primitives::U256;
use starknet::core::types::FieldElement;

pub fn split_u256_maybe_low_high(value: U256) -> Vec<FieldElement> {
    let split = split_u256_into_field_elements(value);
    let range = if value < U256::from(U128::MAX) { 1 } else { 2 };
    split[..range].to_vec()
}
