use blockifier::transaction::objects::{TransactionExecutionInfo, TransactionExecutionResult};
use starknet::core::types::FieldElement;
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
                if let Some(call) = info.execute_call_info {
                    let revert_message: String = call
                        .execution
                        .retdata
                        .0
                        .into_iter()
                        .filter_map(|d| u8::try_from(FieldElement::from(d)).ok())
                        .map(|d| d as char)
                        .collect();
                    warn!("{} returned: {}", case_name, revert_message);
                }
            }
        }
        TransactionExecutionResult::Err(err) => {
            error!("{} tx failed with:\n{:?}", case_name, err);
        }
    }
}
