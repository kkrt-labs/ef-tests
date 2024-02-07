use blockifier::{
    execution::{call_info::CallInfo, errors::EntryPointExecutionError},
    transaction::{
        errors::TransactionExecutionError,
        objects::{TransactionExecutionInfo, TransactionExecutionResult},
    },
};
use starknet::macros::selector;
use starknet_api::transaction::EventContent;
use tracing::{error, info, warn};

#[allow(clippy::cognitive_complexity)]
pub(crate) fn log_execution_result(
    result: &TransactionExecutionResult<TransactionExecutionInfo>,
    case_name: &str,
    case_category: &str,
) {
    let case = format!("{}::{}", case_category, case_name);
    match result {
        TransactionExecutionResult::Ok(info) => {
            if let Some(err) = info.revert_error.as_ref() {
                warn!("{} reverted:\n{}", case, err.replace("\\n", "\n"));
            } else {
                info!("{} passed: {:?}", case, info.actual_resources);
                #[cfg(feature = "v0")]
                if let Some(call) = info.execute_call_info.as_ref() {
                    use starknet::core::types::FieldElement;
                    use starknet_api::hash::StarkFelt;
                    let events = kakarot_execution_events(&call);
                    // Check only one execution event.
                    if events.len() != 1 {
                        warn!(
                            "{} failed to find the single execution event: {:?}",
                            case, events
                        );
                        return;
                    }
                    if events[0].data.0.last() == Some(&StarkFelt::ZERO) {
                        let return_data = call.execution.retdata.0.clone();

                        let revert_message_len = return_data.first().cloned().unwrap_or_default();
                        let revert_message_len =
                            usize::try_from(revert_message_len).unwrap_or_default();

                        let revert_message: String = return_data
                            .into_iter()
                            .skip(1)
                            .filter_map(|d| u8::try_from(FieldElement::from(d)).ok())
                            .map(|d| d as char)
                            .collect();

                        // Check that the length of the revert message matches the first element
                        // in the return data
                        // (https://github.com/kkrt-labs/kakarot/blob/main/src/kakarot/accounts/eoa/externally_owned_account.cairo#L67)
                        if revert_message_len != revert_message.len() {
                            warn!(
                                "{} produced incorrect revert message length: expected {}, got {}",
                                case,
                                revert_message.len(),
                                revert_message_len
                            );
                            return;
                        }
                        warn!("{} returned: {}", case, revert_message);
                    }
                }
            }
        }
        TransactionExecutionResult::Err(TransactionExecutionError::ValidateTransactionError(
            EntryPointExecutionError::VirtualMachineExecutionErrorWithTrace { trace, .. },
        )) => {
            let re = regex::Regex::new(
                r#"Error in the called contract \((0x[0-9a-zA-Z]+)\)[\s\S]*?EntryPointSelector\(StarkFelt\("(0x[0-9a-zA-Z]+)"\)\)"#,
            ).unwrap();
            let matches: Vec<_> = re.captures_iter(trace).map(|c| c.extract::<2>()).collect();
            let last_match = matches.last().cloned().unwrap_or_default();
            warn!(
                "Failed to find entrypoint {} for contract {}",
                last_match.1[1], last_match.1[0]
            );
        }
        TransactionExecutionResult::Err(err) => error!("{} failed with:\n{:?}", case, err),
    }
}

pub(crate) fn extract_execution_retdata(
    result: TransactionExecutionResult<TransactionExecutionInfo>,
) -> Option<String> {
    let info = match result {
        TransactionExecutionResult::Ok(info) => info,
        TransactionExecutionResult::Err(_) => return None,
    };
    if let Some(call) = info.execute_call_info {
        let call_exec = &call.execution;
        let retdata = &call_exec.retdata;

        // Skip the first byte which is the length of the return data
        let retdata_bytes: Vec<u8> = retdata
            .0
            .iter()
            .skip(1)
            .map(|felt| felt.bytes()[31])
            .collect();

        let retdata_str: String = retdata_bytes.iter().map(|&c| c as char).collect();
        return Some(retdata_str);
    }
    None
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
