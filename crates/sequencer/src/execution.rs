use blockifier::transaction::{
    errors::TransactionExecutionError, transaction_execution::Transaction,
};

pub trait Execution {
    fn execute(&mut self, transaction: Transaction) -> Result<(), TransactionExecutionError>;
}
