//! Correlated platform outcomes crossing the ordinary admitted kernel boundary.
use super::*;

pub(in crate::form_runner) fn complete_host_effect(
    scheduler: &mut TourScheduler,
    pending: &PendingHostEffect,
) -> Result<(), String> {
    if matches!(pending.effect, BrowserHostEffect::ClockObservation) {
        return Err("clock observation requires an exact timestamp".into());
    }
    if matches!(pending.effect, BrowserHostEffect::Snapshot { .. }) {
        return resource_effect::complete(scheduler, pending, Ok(None));
    }
    scheduler
        .complete_host_operation(
            pending.request.node,
            pending.request.request,
            HostOperationOutcome {
                disposition: HostOperationDisposition::Completed,
                output: None,
                failure: None,
            },
        )
        .map_err(debug_error)
}

pub(in crate::form_runner) fn complete_host_effect_with_output(
    scheduler: &mut TourScheduler,
    pending: &PendingHostEffect,
    output: &[u8],
) -> Result<(), String> {
    if matches!(pending.effect, BrowserHostEffect::Snapshot { .. }) {
        return resource_effect::complete(scheduler, pending, Ok(Some(output)));
    }
    let maximum_output_bytes = match &pending.effect {
        BrowserHostEffect::ClockObservation => {
            return attempt::complete_clock(scheduler, pending, output)
        }
        BrowserHostEffect::PointerEvent => {
            let value = conduit_core::StructuredInfoValue::from_canonical_bytes(output)
                .map_err(|error| format!("decode pointer input: {error:?}"))?;
            if value.value_type() != &conduit_semantic_catalog::pointer_event_type() {
                return Err("pointer input has the wrong exact type".into());
            }
            MAXIMUM_BROWSER_VALUE_BYTES as u32
        }
        BrowserHostEffect::KeyEvent => {
            conduit_human::KeyEvent::decode(output)
                .map(|_| ())
                .map_err(|error| format!("decode browser key event: {error:?}"))?;
            conduit_human::KEY_EVENT_ENCODED_LEN as u32
        }
        BrowserHostEffect::ButtonTransition => {
            conduit_semantic_catalog::map_button_transition_to_indicator(output)
                .map_err(|error| format!("decode browser button transition: {error:?}"))?;
            conduit_semantic_catalog::BUTTON_TRANSITION_MAXIMUM_BYTES
        }
        _ => return Err("browser Host effect does not accept completion output".into()),
    };
    let value = scheduler.store_host_value(output).map_err(debug_error)?;
    let result = scheduler
        .complete_host_operation(
            pending.request.node,
            pending.request.request,
            HostOperationOutcome {
                disposition: HostOperationDisposition::Completed,
                output: Some(
                    BoundedValueRef::new(value, maximum_output_bytes)
                        .map_err(|_| "browser input exceeded its planned bound")?,
                ),
                failure: None,
            },
        )
        .map_err(debug_error);
    if result.is_err() {
        scheduler.discard_host_value(value).map_err(debug_error)?;
    }
    result
}
