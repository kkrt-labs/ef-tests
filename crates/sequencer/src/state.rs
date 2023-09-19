use blockifier::{
    execution::contract_class::ContractClass, state::cached_state::ContractStorageKey,
};
use rustc_hash::FxHashMap;
use starknet_api::{
    core::{ClassHash, ContractAddress, Nonce},
    hash::StarkFelt,
};

#[derive(Default)]
/// Generic state structure for the sequencer.
pub struct State {
    classes: FxHashMap<ClassHash, ContractClass>,
    contracts: FxHashMap<ContractAddress, ClassHash>,
    storage: FxHashMap<ContractStorageKey, StarkFelt>,
    nonces: FxHashMap<ContractAddress, Nonce>,
}
