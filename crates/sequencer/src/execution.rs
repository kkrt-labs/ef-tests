use blockifier::transaction::{
    objects::{TransactionExecutionInfo, TransactionExecutionResult},
    transaction_execution::Transaction,
};

pub trait Execution {
    fn execute(
        &mut self,
        transaction: Transaction,
    ) -> TransactionExecutionResult<TransactionExecutionInfo>;
}
