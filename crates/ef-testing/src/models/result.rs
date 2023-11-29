use blockifier::{
    execution::call_info::CallInfo,
    transaction::objects::{TransactionExecutionInfo, TransactionExecutionResult},
};
use starknet::{core::types::FieldElement, macros::selector};
use starknet_api::{hash::StarkFelt, transaction::EventContent};
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
                    let events = get_kakarot_execution_events(&call);
                    if events.len() != 1 {
                        warn!(
                            "{} failed to find the single execution event: {:?}",
                            case_name, events
                        );
                        return;
                    }
                    if events[0].data.0.last() == Some(&StarkFelt::ZERO) {
                        let return_data = call.execution.retdata.0;

                        let revert_message_len = return_data.first().cloned().unwrap_or_default();
                        let revert_message_len =
                            usize::try_from(revert_message_len).unwrap_or_default();

                        let revert_message: String = return_data
                            .into_iter()
                            .skip(1)
                            .filter_map(|d| u8::try_from(FieldElement::from(d)).ok())
                            .map(|d| d as char)
                            .collect();
                        if revert_message_len != revert_message.len() {
                            warn!(
                                "{} produced incorrect revert message length: expected {}, got {}",
                                case_name,
                                revert_message.len(),
                                revert_message_len
                            );
                        } else {
                            warn!("{} returned: {}", case_name, revert_message);
                        }
                    }
                }
            }
        }
        TransactionExecutionResult::Err(err) => {
            error!("{} tx failed with:\n{:?}", case_name, err);
        }
    }
}

fn get_kakarot_execution_events(call_info: &CallInfo) -> Vec<EventContent> {
    let mut events = Vec::new();
    for c in call_info.into_iter() {
        let mut filtered_events = c
            .execution
            .events
            .iter()
            .filter(|e| {
                e.event.keys.get(0).map(|e| e.0) == Some(selector!("transaction_executed").into())
            })
            .map(|e| e.event.clone())
            .collect();
        events.append(&mut filtered_events);
    }
    events
}
