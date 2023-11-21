#[cfg(feature = "v0")]
pub mod v0;
// #[cfg(feature = "v1")]
pub mod v1;

use blockifier::state::state_api::StateResult;
use reth_primitives::{Address, Bytes};
use revm_primitives::U256;

/// EVM state interface. Used to setup EOA and contract accounts,
/// fund them and get their state (balance, nonce, code, storage).
/// Default implementation is used when no feature flag is enabled.
pub trait EvmState {
    fn setup_account(
        &mut self,
        _evm_address: &Address,
        _bytecode: &Bytes,
        _nonce: U256,
        _storage: Vec<(U256, U256)>,
    ) -> StateResult<()> {
        panic!("Not implemented, use features flag \"v0\" or \"v1\"")
    }

    fn fund(&mut self, _evm_address: &Address, _balance: U256) -> StateResult<()> {
        panic!("Not implemented, use features flag \"v0\" or \"v1\"")
    }

    fn get_storage_at(&mut self, _evm_address: &Address, _key: U256) -> StateResult<U256> {
        panic!("Not implemented, use features flag \"v0\" or \"v1\"")
    }

    fn get_nonce_at(&mut self, _evm_address: &Address) -> StateResult<U256> {
        panic!("Not implemented, use features flag \"v0\" or \"v1\"")
    }

    fn get_code_at(&mut self, _evm_address: &Address) -> StateResult<Bytes> {
        panic!("Not implemented, use features flag \"v0\" or \"v1\"")
    }

    fn get_balance_at(&mut self, _evm_address: &Address) -> StateResult<U256> {
        panic!("Not implemented, use features flag \"v0\" or \"v1\"")
    }
}

// #[cfg(not(any(feature = "v0", feature = "v1")))]
// impl EvmState for super::sequencer::KakarotSequencer {}
