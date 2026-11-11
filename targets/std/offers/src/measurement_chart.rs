//! Immutable chart Face realization facts; workspace is supplied by its actual
//! installed provider. This descriptor grants no memory, producer trust or Play.
use conduit_core::*;
pub const MEASUREMENT_CHART_FACE_IMPLEMENTATION: &str = "std/measurement-chart-face@1";
pub const MEASUREMENT_CHART_FACE_PROFILE: &str = "std/measurement-chart-face-hosted@1";
pub const MEASUREMENT_CHART_FACE_HOST_CALL: &str = "conduit.host/measurement-chart-face@1";
/// Input is canonical StructuredInfoValue wrapping the bounded intrinsic chart;
/// the Host Call emits no kernel value. Face publication has separate commit custody.
pub fn measurement_chart_face_std_offer(
    workspace_bytes: u32,
) -> Result<CapabilityOffer, &'static str> {
    if workspace_bytes == 0 {
        return Err("measurement chart Face requires explicit finite workspace");
    }
    let kind = conduit_data::measurement_chart_face_semantic_contract();
    let digest = semantic_digest(
        "std/measurement-chart-face-provider@1",
        concat!(
            include_str!("measurement_chart.rs"),
            include_str!("../../src/measurement_chart_offer.rs"),
            include_str!("../../src/measurement_face.rs"),
            include_str!("../../src/installed_std/measurement_chart_face_back.rs"),
            include_str!("../../../../semantics/data/src/measurement_chart.rs"),
            include_str!("../../../../semantics/data/src/measurement_chart/wire.rs"),
            include_str!("../../../../semantics/data/src/measurement_chart_catalog.rs"),
        )
        .as_bytes(),
    );
    let artifact = format!(
        "sha256:{}",
        digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
    Ok(BackOfferBuilder::new(
        kind.clone(),
        Back {
            capability_id: MEASUREMENT_CHART_FACE_IMPLEMENTATION.into(),
            execution_profile_id: MEASUREMENT_CHART_FACE_PROFILE.into(),
            implementation_id: MEASUREMENT_CHART_FACE_IMPLEMENTATION.into(),
            artifact_id: artifact.into(),
            host_calls: vec![HostCallRequirement {
                contract_id: MEASUREMENT_CHART_FACE_HOST_CALL.into(),
                target_kind: Some(kind.kind_id),
                maximum_in_flight: 1,
                maximum_input_bytes: conduit_data::measurement_chart_canonical_bytes() as u32,
                maximum_output_bytes: 0,
            }],
            resource_requirements: vec![resource_requirement(
                RUNTIME_MEMORY_RESOURCE_CLASS,
                workspace_bytes,
            )],
            authority_requirements: vec![],
        },
    )
    .build())
}
/// Requested bounded Rust backing, excluding allocator overhead and stacks.
/// Scope and escaped commit ownership have separate lifetime accounting.
pub fn measurement_chart_face_fragment_backing_bytes() -> usize {
    use conduit_data as d;
    use conduit_face::{FaceFragment, FaceProperty, FaceSubject, FaceText, MAX_FACE_ID_BYTES};
    let properties = d::MAXIMUM_MEASUREMENT_CHART_SERIES
        * (core::mem::size_of::<FaceProperty>()
            + MAX_FACE_ID_BYTES
            + d::MAXIMUM_MEASUREMENT_CHART_LABEL_BYTES
            + d::MAXIMUM_MEASUREMENT_PLOT_SERIES_BYTES
            + d::MEASUREMENT_PLOT_SERIES_INFO_ID.len());
    core::mem::size_of::<FaceFragment>()
        + 3 * MAX_FACE_ID_BYTES
        + core::mem::size_of::<FaceSubject>()
        + 2 * MAX_FACE_ID_BYTES
        + d::MEASUREMENT_PLOT_FACE_KIND.len()
        + core::mem::size_of::<FaceText>()
        + MAX_FACE_ID_BYTES
        + 128
        + properties
}
pub fn measurement_chart_face_workspace_bytes() -> u32 {
    use conduit_data as d;
    u32::try_from(
        d::measurement_chart_decode_workspace_bytes()
            + 2 * d::MAXIMUM_MEASUREMENT_PLOT_SERIES_BYTES
            + d::MAXIMUM_MEASUREMENT_CHART_SERIES
                * core::mem::size_of::<(&'static str, &'static d::MeasurementPlotSeries)>()
            + 2 * measurement_chart_face_fragment_backing_bytes(),
    )
    .expect("finite chart Face workspace")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_chart_sink_offer_requires_explicit_workspace_and_no_return_value() {
        assert!(measurement_chart_face_std_offer(0).is_err());
        let offer = measurement_chart_face_std_offer(12345).unwrap();
        assert_eq!(offer.inputs.len(), 1);
        assert!(offer.outputs.is_empty());
        assert_eq!(
            offer.inputs[0].temporal,
            PortTemporal::Flow { closes: true }
        );
        assert_eq!(
            offer.host_calls[0].maximum_input_bytes as usize,
            conduit_data::measurement_chart_canonical_bytes()
        );
        assert_eq!(offer.host_calls[0].maximum_output_bytes, 0);
        assert_eq!(
            offer.resource_requirements,
            vec![resource_requirement(RUNTIME_MEMORY_RESOURCE_CLASS, 12345)]
        );
        assert!(offer.authority_requirements.is_empty());
    }
}
