#[cfg(feature = "v0")]
pub mod v0;
#[cfg(feature = "v1")]
pub mod v1;

use std::{cell::RefCell, rc::Rc};

use cairo_native::cache::ProgramCache;
use reth_primitives::{Address, Bytes, TransactionSigned};
use revm_primitives::U256;
use sequencer::{execution::TransactionExecutionResult, state::StateResult};
use starknet_in_rust::{execution::TransactionExecutionInfo, transaction::ClassHash};

use super::account::KakarotAccount;

/// EVM state interface. Used to setup the evm state, EOA and contract accounts,
/// fund them and get their state (balance, nonce, code, storage).
/// Default implementation is used when no feature flag is enabled.
pub trait Evm {
    // TODO enforce using a marker type that you can only proceed
    // with execution if the state is initialized.
    fn setup_state(&mut self) -> StateResult<()> {
        panic!("Not implemented, use features flag \"v0\" or \"v1\"")
    }

    fn setup_account(&mut self, _account: KakarotAccount) -> StateResult<()> {
        panic!("Not implemented, use features flag \"v0\" or \"v1\"")
    }

    fn fund(&mut self, _evm_address: &Address, _balance: U256) -> StateResult<()> {
        panic!("Not implemented, use features flag \"v0\" or \"v1\"")
    }

    fn storage_at(&mut self, _evm_address: &Address, _key: U256) -> StateResult<U256> {
        panic!("Not implemented, use features flag \"v0\" or \"v1\"")
    }

    fn nonce_at(&mut self, _evm_address: &Address) -> StateResult<U256> {
        panic!("Not implemented, use features flag \"v0\" or \"v1\"")
    }

    fn code_at(&mut self, _evm_address: &Address) -> StateResult<Bytes> {
        panic!("Not implemented, use features flag \"v0\" or \"v1\"")
    }

    fn balance_at(&mut self, _evm_address: &Address) -> StateResult<U256> {
        panic!("Not implemented, use features flag \"v0\" or \"v1\"")
    }

    fn execute_transaction(
        &mut self,
        _transaction: TransactionSigned,
        _cache: Option<Rc<RefCell<ProgramCache<'_, ClassHash>>>>,
    ) -> TransactionExecutionResult<TransactionExecutionInfo> {
        panic!("Not implemented, use features flag \"v0\" or \"v1\"")
    }
}

#[cfg(not(any(feature = "v0", feature = "v1")))]
impl Evm for super::sequencer::KakarotSequencer {}
