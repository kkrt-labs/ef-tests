use blockifier::{
    execution::{call_info::CallInfo, errors::EntryPointExecutionError},
    transaction::{
        errors::TransactionExecutionError,
        objects::{TransactionExecutionInfo, TransactionExecutionResult},
    },
};
use starknet::macros::selector;
use starknet_api::transaction::{EventContent, EventData};
use tracing::{error, info, warn};

use std::convert::From;

#[derive(Default, Debug)]
pub struct EVMOutput {
    pub return_data: String,
    pub gas_used: u64,
    pub success: bool,
}

impl From<&EventData> for EVMOutput {
    fn from(input: &EventData) -> Self {
        let return_data_len: usize = input.0[0].try_into().unwrap();
        let return_data_bytes = input
            .0
            .iter()
            .skip(1)
            .take(return_data_len)
            .flat_map(|felt| felt.bytes().last().cloned())
            .collect();
        let return_data = String::from_utf8(return_data_bytes).unwrap();

        let success: u64 = input.0[1 + return_data_len].try_into().unwrap();
        let gas_used: u64 = input.0[input.0.len() - 1].try_into().unwrap();

        EVMOutput {
            return_data,
            gas_used,
            success: success == 1,
        }
    }
}

#[allow(clippy::cognitive_complexity)]
pub(crate) fn extract_output_and_log_execution_result(
    result: &TransactionExecutionResult<TransactionExecutionInfo>,
    case_name: &str,
    case_category: &str,
) -> Option<EVMOutput> {
    let case = format!("{}::{}", case_category, case_name);
    match result {
        TransactionExecutionResult::Ok(info) => {
            /* trunk-ignore(clippy/option_if_let_else) */
            if let Some(err) = info.revert_error.as_ref() {
                warn!("{} reverted:\n{}", case, err.replace("\\n", "\n"));
                return None;
            }

            info!("{} passed: {:?}", case, info.actual_resources);
            #[cfg(feature = "v0")]
            if let Some(call) = info.execute_call_info.as_ref() {
                use starknet_api::hash::StarkFelt;
                let events = kakarot_execution_events(call);
                // Check only one execution event.
                if events.len() != 1 {
                    warn!(
                        "{} failed to find the single execution event: {:?}",
                        case, events
                    );
                    return None;
                }
                let output = EVMOutput::from(&events[0].data);
                if events[0].data.0.last() == Some(&StarkFelt::ZERO) {
                    warn!("{} returned: {}", case, output.return_data);
                }
                return Some(output);
            }
            None
        }
        TransactionExecutionResult::Err(TransactionExecutionError::ValidateTransactionError(
            EntryPointExecutionError::VirtualMachineExecutionErrorWithTrace { trace, .. },
        )) => {
            // There are specific test cases where validation failed because the sender account has code.
            // They're caught by EOA validation, and rejected with this specific error message.
            if trace.contains("EOAs cannot have code") {
                return None;
            }
            let re = regex::Regex::new(
                r#"Error in the called contract \((0x[0-9a-zA-Z]+)\)[\s\S]*?EntryPointSelector\(StarkFelt\("(0x[0-9a-zA-Z]+)"\)\)"#,
            ).unwrap();
            let matches: Vec<_> = re.captures_iter(trace).map(|c| c.extract::<2>()).collect();
            let last_match = matches.last().cloned().unwrap_or_default();
            warn!(
                "Failed to find entrypoint {} for contract {}",
                last_match.1[1], last_match.1[0]
            );
            None
        }
        TransactionExecutionResult::Err(err) => {
            error!("{} failed with:\n{:?}", case, err);
            None
        }
    }
}

#[allow(dead_code)]
fn kakarot_execution_events(call_info: &CallInfo) -> Vec<EventContent> {
    let mut events = Vec::new();
    for c in call_info.into_iter() {
        let mut filtered_events = c
            .execution
            .events
            .iter()
            .filter(|e| {
                e.event.keys.first().map(|e| e.0) == Some(selector!("transaction_executed").into())
            })
            .map(|e| e.event.clone())
            .collect();
        events.append(&mut filtered_events);
    }
    events
}
