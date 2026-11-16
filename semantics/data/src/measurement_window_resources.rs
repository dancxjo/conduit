//! Finite requested backing definitions; not an admission or possession.
fn bound(ty: conduit_core::StructuredInfoType, payload: usize) -> u32 {
    (ty.canonical_bytes().expect("finite measurement Type").len() + 5 + payload)
        .try_into()
        .expect("finite measurement bound")
}
pub fn measurement_window_value_bounds() -> [u32; 3] {
    [
        bound(
            crate::measurement_window_profile_type(),
            crate::MAXIMUM_MEASUREMENT_WINDOW_PROFILE_BYTES,
        ),
        bound(
            crate::measurement_sample_type(),
            crate::MAXIMUM_MEASUREMENT_SAMPLE_BYTES,
        ),
        bound(
            crate::measurement_window_type(),
            crate::MAXIMUM_MEASUREMENT_WINDOW_BYTES,
        ),
    ]
}
/// Full dynamic requested-backing highwater, retained for the entire Play.
/// Covers committed+candidate Vec capacity, clone realloc old+new, bounded
/// clock text, codec payload realloc and an independently copied Host reply.
/// Provider's already prepared type/input/output buffers are charged separately.
pub fn measurement_window_dynamic_workspace_bytes() -> usize {
    let n = crate::MAXIMUM_MEASUREMENT_WINDOW_SAMPLES;
    5 * n * core::mem::size_of::<crate::MeasurementSample>()
        + 3 * (n + 1) * 128
        + 3 * crate::MAXIMUM_MEASUREMENT_WINDOW_BYTES
        + crate::MAXIMUM_MEASUREMENT_SAMPLE_BYTES
        + crate::MAXIMUM_MEASUREMENT_WINDOW_PROFILE_BYTES
}
