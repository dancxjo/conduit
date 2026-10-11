//! Canonical Data envelopes include the bounded intrinsic leaf/type header.
const HEADER: usize = 256;
pub(crate) const PROFILE: usize = conduit_data::MAXIMUM_MEASUREMENT_WINDOW_PROFILE_BYTES + HEADER;
pub(crate) const SAMPLE: usize = conduit_data::MAXIMUM_MEASUREMENT_SAMPLE_BYTES + HEADER;
pub(crate) const WINDOW: usize = conduit_data::MAXIMUM_MEASUREMENT_WINDOW_BYTES + HEADER;
pub(crate) const SUMMARY: usize = conduit_data::MAXIMUM_MEASUREMENT_SUMMARY_BYTES + HEADER;
pub(crate) const HYSTERESIS: usize =
    conduit_data::MAXIMUM_MEASUREMENT_HYSTERESIS_PROFILE_BYTES + HEADER;
pub(crate) const DECISION: usize =
    conduit_data::MAXIMUM_MEASUREMENT_THRESHOLD_DECISION_BYTES + HEADER;
pub(crate) const SERIES: usize = conduit_data::MAXIMUM_MEASUREMENT_PLOT_SERIES_BYTES + HEADER;
pub(crate) const MEMORY_POOL_BYTES: u32 = 8 * 1024 * 1024;
pub(crate) fn queue_bound(kind: &str) -> Option<u32> {
    Some(
        match kind {
            conduit_data::MEASUREMENT_COUNT_WINDOW_KIND
            | conduit_data::MEASUREMENT_SUMMARY_KIND
            | conduit_data::MEASUREMENT_PLOT_KIND => WINDOW,
            conduit_data::MEASUREMENT_HYSTERESIS_KIND => SUMMARY.max(HYSTERESIS),
            conduit_data::MEASUREMENT_OBSERVATION_KIND => SAMPLE,
            _ => return None,
        }
        .max(super::MAXIMUM_BROWSER_VALUE_BYTES) as u32,
    )
}

/// Reserve output queues, host-call copies, codec scratch and retained window
/// samples from this installed profile's finite envelope before planning.
pub(crate) fn finish_offer(
    mut offer: conduit_core::CapabilityOffer,
) -> conduit_core::CapabilityOffer {
    if let Some(bound) = queue_bound(offer.kind_id.as_str()) {
        offer.limits.max_queue_items = offer.limits.max_queue_items.min(4);
        offer.limits.max_queue_bytes = bound;
        let queue_slots = u32::from(offer.limits.max_queue_items)
            * (offer.inputs.len() + offer.outputs.len()) as u32;
        let transient_slots = (offer.outputs.len() + offer.host_calls.len()) as u32;
        let retained = if offer.kind_id.as_str() == conduit_data::MEASUREMENT_COUNT_WINDOW_KIND {
            core::mem::size_of::<conduit_data::BoundedMeasurementWindow>()
                + conduit_data::MAXIMUM_MEASUREMENT_WINDOW_SAMPLES
                    * core::mem::size_of::<conduit_data::MeasurementSample>()
                + PROFILE
        } else {
            0
        };
        let bytes = bound * (queue_slots + transient_slots + 4) + retained as u32 + 4096;
        offer
            .resource_requirements
            .push(conduit_core::resource_requirement(
                conduit_core::RUNTIME_MEMORY_RESOURCE_CLASS,
                bytes,
            ));
    }
    offer
}

/// Resolve only reviewed finite target wire profiles; other values keep the
/// existing generic browser queue envelope.
pub(crate) fn value_queue_bound(kind: &conduit_core::KindId) -> Option<u32> {
    for (value_type, bound) in [
        (conduit_data::measurement_window_profile_type(), PROFILE),
        (conduit_data::measurement_sample_type(), SAMPLE),
        (conduit_data::measurement_window_type(), WINDOW),
        (conduit_data::measurement_summary_type(), SUMMARY),
        (
            conduit_data::measurement_hysteresis_profile_type(),
            HYSTERESIS,
        ),
        (
            conduit_data::measurement_threshold_decision_type(),
            DECISION,
        ),
        (conduit_data::measurement_plot_series_type(), SERIES),
    ] {
        if value_type.profile().ok()?.value_kind() == kind {
            return Some(bound.max(super::MAXIMUM_BROWSER_VALUE_BYTES) as u32);
        }
    }
    let pointer = conduit_semantic_catalog::pointer_source_semantic_contract();
    if pointer.outputs[0].value_kind == *kind {
        return Some(super::NORMALIZED_POINTER_VALUE_BYTES as u32);
    }
    let stroke = conduit_presentation::capture_bounded_stroke_kind_projection();
    (stroke.outputs[0].value_kind == *kind)
        .then_some(super::stroke_capture::MAXIMUM_PATH_BYTES as u32)
}
