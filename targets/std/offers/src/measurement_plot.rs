//! Reviewed native measurement projection descriptor and finite CPU workspace.
use conduit_core::{
    ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, ExecutionProfileId,
    HostCallRequirement, ImplementationId,
};
use conduit_data::MeasurementPlotSeries;
pub const MEASUREMENT_PLOT_IMPLEMENTATION: &str = "std/measurement-plot@1";
pub const MEASUREMENT_PLOT_PROFILE: &str = "std/measurement-plot-hosted@1";
pub const MEASUREMENT_PLOT_HOST_CALL: &str = "conduit.host/measurement-plot@1";

/// Canonical structured values include the exact intrinsic leaf contract/header.
/// These bounds are derived from that contract, not a renderer's packet estimate.
pub fn measurement_plot_value_bounds() -> (u32, u32) {
    let bound = |value_type: conduit_core::StructuredInfoType, payload: usize| {
        u32::try_from(
            value_type
                .canonical_bytes()
                .expect("reviewed measurement type")
                .len()
                + 5
                + payload,
        )
        .expect("finite measurement value bound")
    };
    (
        bound(
            conduit_data::measurement_window_type(),
            conduit_data::MAXIMUM_MEASUREMENT_WINDOW_BYTES,
        ),
        bound(
            conduit_data::measurement_plot_series_type(),
            conduit_data::MAXIMUM_MEASUREMENT_PLOT_SERIES_BYTES,
        ),
    )
}

/// Conservative requested Rust backing, excluding allocator overhead and stacks.
/// The installation owner must separately admit this requirement and bind the
/// provider to its actual Host Call; this number is not an ownership witness.
pub fn measurement_plot_workspace_bytes() -> u32 {
    let (input, output) = measurement_plot_value_bounds();
    let decoded = core::mem::size_of::<conduit_data::BoundedMeasurementWindow>()
        // The decoder and validated restored window can coexist. All decoded
        // clock text together is bounded by the incoming wire payload.
        + 2 * conduit_data::MAXIMUM_MEASUREMENT_WINDOW_SAMPLES
            * core::mem::size_of::<conduit_data::MeasurementSample>()
        + conduit_data::MAXIMUM_MEASUREMENT_WINDOW_BYTES;
    let projected = 2
        * conduit_data::MAXIMUM_MEASUREMENT_PLOT_POINTS
        * core::mem::size_of::<conduit_data::MeasurementPlotPoint>()
        + core::mem::size_of::<MeasurementPlotSeries>();
    input + output * 2 + u32::try_from(decoded + projected).expect("finite projection workspace")
}

pub fn measurement_plot_std_offer() -> CapabilityOffer {
    let contract = conduit_data::measurement_plot_semantic_contract();
    let kind = contract.kind_id.clone();
    let digest = conduit_core::semantic_digest(
        "std/measurement-plot-provider@1",
        concat!(
            include_str!("measurement_plot.rs"),
            include_str!("../../src/hosted_measurement.rs"),
            include_str!("../../../../semantics/data/src/measurement_plot.rs"),
            include_str!("../../../../semantics/data/src/measurement_wire.rs"),
            include_str!("../../../../semantics/data/src/measurement_window.rs"),
            include_str!("../../../../semantics/data/types.conduit"),
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
    let (input, output) = measurement_plot_value_bounds();
    let mut offered = BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from(MEASUREMENT_PLOT_IMPLEMENTATION),
            execution_profile_id: ExecutionProfileId::from(MEASUREMENT_PLOT_PROFILE),
            implementation_id: ImplementationId::from(MEASUREMENT_PLOT_IMPLEMENTATION),
            artifact_id: ArtifactId::from(artifact),
            host_calls: vec![HostCallRequirement {
                contract_id: MEASUREMENT_PLOT_HOST_CALL.into(),
                target_kind: Some(kind),
                maximum_in_flight: 1,
                maximum_input_bytes: input,
                maximum_output_bytes: output,
            }],
            resource_requirements: vec![conduit_core::resource_requirement(
                conduit_core::RUNTIME_MEMORY_RESOURCE_CLASS,
                measurement_plot_workspace_bytes(),
            )],
            authority_requirements: vec![],
        },
    )
    .build();
    // Refine the generic semantic ceiling to this implementation's actual
    // finite wire envelope, avoiding 256KiB queue reservations per port.
    offered.limits.max_queue_bytes = input.max(output);
    offered
}
