use blockifier::{
    execution::contract_class::ContractClass, state::cached_state::ContractStorageKey,
};
use rustc_hash::FxHashMap;
use starknet_api::core::CompiledClassHash;
use starknet_api::{
    core::{ClassHash, ContractAddress, Nonce},
    hash::StarkFelt,
};

/// Generic state structure for the sequencer.
/// The use of FxHashMap allows for a better performance.
/// This hash map is used by rustc. It uses a non cryptographic hash function
/// which is faster than the default hash function. Think about changing
/// if the test sequencer is used for tests outside of ef-tests.
/// See [rustc-hash](https://crates.io/crates/rustc-hash) for more information.
#[derive(Default)]
pub struct State {
    pub classes: FxHashMap<ClassHash, ContractClass>,
    pub compiled_classes: FxHashMap<ClassHash, CompiledClassHash>,
    pub contracts: FxHashMap<ContractAddress, ClassHash>,
    pub storage: FxHashMap<ContractStorageKey, StarkFelt>,
    pub nonces: FxHashMap<ContractAddress, Nonce>,
}
