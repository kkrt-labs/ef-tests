use sequencer::execution::TransactionExecutionResult;
use starknet::macros::selector;
use starknet_in_rust::{
    execution::{CallInfo, OrderedEvent, TransactionExecutionInfo},
    utils::field_element_to_felt,
};
use tracing::{error, info, warn};

pub(crate) fn log_execution_result(
    result: TransactionExecutionResult<TransactionExecutionInfo>,
    case_name: &str,
    case_category: &str,
) {
    let case = format!("{}::{}", case_category, case_name);
    match result {
        TransactionExecutionResult::Ok(info) => {
            if let Some(err) = info.revert_error {
                warn!("{} reverted:\n{}", case, err.replace("\\n", "\n"));
            } else {
                info!("{} passed: {:?}", case, info.actual_resources);
                #[cfg(feature = "v0")]
                if let Some(call) = info.call_info {
                    use cairo_vm::Felt252;
                    use num_traits::ToPrimitive;
                    let events = get_kakarot_execution_events(&call);
                    // Check only one execution event.
                    if events.len() != 1 {
                        warn!(
                            "{} failed to find the single execution event: {:?}",
                            case, events
                        );
                        return;
                    }
                    if Some(&Felt252::ZERO) == events[0].data.last() {
                        let return_data = call.retdata;

                        let revert_message_len = return_data.first().cloned().unwrap_or_default();
                        let revert_message_len = revert_message_len.to_usize().unwrap_or_default();

                        let revert_message: String = return_data
                            .into_iter()
                            .skip(1)
                            .filter_map(|d| d.to_u8())
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
        TransactionExecutionResult::Err(err) => {
            error!("{} tx failed with:\n{:?}", case, err);
        }
    }
}

#[allow(dead_code)]
fn get_kakarot_execution_events(call_info: &CallInfo) -> Vec<OrderedEvent> {
    call_info
        .events
        .iter()
        .filter(|e| {
            e.keys.first() == Some(&field_element_to_felt(&selector!("transaction_executed")))
        })
        .cloned()
        .collect()
}
