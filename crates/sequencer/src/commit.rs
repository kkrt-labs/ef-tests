use blockifier::state::{
    cached_state::CachedState,
    state_api::{State as BlockifierState, StateReader as BlockifierStateReader, StateResult},
};

/// Generic trait for committing changes from a cached state to a state.
/// The default implementation allows for any type S for which a mutable reference
/// implements the `BlockifierState` and `BlockifierStateReader` traits to be used.
pub trait Committer<S>
where
    for<'any> &'any mut S: BlockifierState + BlockifierStateReader,
{
    fn commit(cached_state: &mut CachedState<&mut S>) -> StateResult<()> {
        let diff = cached_state.to_state_diff();
        for (address, class_hash) in diff.address_to_class_hash {
            cached_state.state.set_class_hash_at(address, class_hash)?;
        }
        for (address, _) in diff.address_to_nonce {
            cached_state.state.increment_nonce(address)?;
        }
        for (address, storage_updates) in diff.storage_updates {
            for (k, v) in storage_updates {
                cached_state.state.set_storage_at(address, k, v)?;
            }
        }
        for (class_hash, compiled_class_hash) in diff.class_hash_to_compiled_class_hash {
            cached_state
                .state
                .set_compiled_class_hash(class_hash, compiled_class_hash)?;
        }
        Ok(())
    }
}
