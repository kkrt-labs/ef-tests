use blockifier::transaction::objects::{TransactionExecutionInfo, TransactionExecutionResult};
use tracing::{error, info, warn};

pub(crate) fn log_execution_result(
    result: TransactionExecutionResult<TransactionExecutionInfo>,
    case_name: &str,
) {
    match result {
        TransactionExecutionResult::Ok(info) => {
            if let Some(err) = info.revert_error {
                warn!("{} reverted:\n{}", case_name, err.replace("\\n", "\n"));
            } else {
                info!("{} passed: {:?}", case_name, info.actual_resources);
            }
        }
        TransactionExecutionResult::Err(err) => {
            error!("{} tx failed with:\n{:?}", case_name, err);
        }
    }
}
