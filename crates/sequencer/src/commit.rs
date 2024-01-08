use starknet_in_rust::state::{
    cached_state::CachedState,
    contract_class_cache::ContractClassCache,
    state_api::{State, StateReader},
    StateDiff,
};

use crate::state::StateResult;

/// Generic trait for committing changes from a cached state to a state.
/// The default implementation allows committing changes from a starknet
/// in rust cached state to a starknet in rust state.
pub trait Committer: State {
    fn commit<S: StateReader, C: ContractClassCache>(
        &mut self,
        cached_state: &mut CachedState<S, C>,
    ) -> StateResult<()> {
        let cache = cached_state.cache();
        let state_diff = StateDiff::from_cached_state(cache)?;
        self.apply_state_update(&state_diff)?;
        Ok(())
    }
}
