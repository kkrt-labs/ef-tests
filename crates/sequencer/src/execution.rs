use starknet_in_rust::{
    execution::TransactionExecutionInfo,
    transaction::{error::TransactionError, Transaction},
};

pub type TransactionExecutionResult<T> = Result<T, TransactionError>;

pub trait Execution {
    fn execute(
        &mut self,
        transaction: &Transaction,
    ) -> TransactionExecutionResult<TransactionExecutionInfo>;
}
