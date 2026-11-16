//! Exact admitted browser timer request decoding.
pub(super) fn decode_timer_duration(
    operation: &conduit_core::HostCallRequirement,
    input: &[u8],
) -> Result<u64, String> {
    let expected_target =
        if operation.contract_id.as_str() == conduit_core::MONOTONIC_TIMER_HOST_CALL_CONTRACT {
            Some(conduit_semantic_catalog::TIMED_BUTTON_ATTEMPT_KIND.into())
        } else {
            None
        };
    if operation.target_kind != expected_target
        || operation.maximum_in_flight != 1
        || operation.maximum_input_bytes != conduit_time::TICK_ENCODED_LEN
        || operation.maximum_output_bytes != 0
    {
        return Err("planned browser timer operation has the wrong exact contract".into());
    }
    let duration_millis = conduit_core::decode_monotonic_duration(input)
        .map_err(|error| format!("decode browser timer duration: {error:?}"))?;
    if duration_millis > crate::installed_browser::BROWSER_TIMER_MAXIMUM_MILLIS {
        return Err("browser timer duration exceeds its admitted implementation bound".into());
    }
    Ok(duration_millis)
}
