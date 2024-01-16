use std::{cell::RefCell, rc::Rc};

use cairo_native::cache::ProgramCache;
use starknet_in_rust::{
    execution::TransactionExecutionInfo,
    transaction::{error::TransactionError, Transaction},
    utils::ClassHash,
};

pub type TransactionExecutionResult<T> = Result<T, TransactionError>;

pub trait Execution {
    fn execute(
        &mut self,
        transaction: &Transaction,
        cache: Option<Rc<RefCell<ProgramCache<'_, ClassHash>>>>,
    ) -> TransactionExecutionResult<TransactionExecutionInfo>;
}
