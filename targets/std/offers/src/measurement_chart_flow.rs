//! Explicit terminal-capable Face realization, with independently funded workspace.
use conduit_core::*;
pub const MEASUREMENT_CHART_FLOW_FACE_IMPLEMENTATION: &str = "std/measurement-chart-face-flow@1";
pub const MEASUREMENT_CHART_FLOW_FACE_PROFILE: &str = "std/measurement-chart-face-flow-hosted@1";
pub const MEASUREMENT_CHART_FLOW_FACE_HOST_CALL: &str =
    "conduit.host/measurement-chart-face-flow@1";
pub fn measurement_chart_flow_face_std_offer(
    workspace: u32,
) -> Result<CapabilityOffer, &'static str> {
    if workspace == 0 {
        return Err("chart Face workspace must be explicitly funded");
    }
    let kind = conduit_data::measurement_chart_flow_semantic_contract();
    let digest = semantic_digest(
        "std/chart-face-flow@1",
        concat!(
            include_str!("measurement_chart_flow.rs"),
            include_str!("../../src/measurement_chart_flow_offer.rs"),
            include_str!("../../src/installed_std/measurement_chart_face_back.rs"),
            include_str!("../../src/installed_std/measurement_chart_face_back/flow.rs"),
            include_str!("../../../../semantics/data/src/measurement_chart_flow_catalog.rs")
        )
        .as_bytes(),
    );
    let artifact = format!(
        "sha256:{}",
        digest
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    );
    Ok(BackOfferBuilder::new(
        kind.clone(),
        Back {
            capability_id: MEASUREMENT_CHART_FLOW_FACE_IMPLEMENTATION.into(),
            execution_profile_id: MEASUREMENT_CHART_FLOW_FACE_PROFILE.into(),
            implementation_id: MEASUREMENT_CHART_FLOW_FACE_IMPLEMENTATION.into(),
            artifact_id: artifact.into(),
            host_calls: vec![HostCallRequirement {
                contract_id: MEASUREMENT_CHART_FLOW_FACE_HOST_CALL.into(),
                target_kind: Some(kind.kind_id),
                maximum_in_flight: 1,
                maximum_input_bytes: conduit_data::measurement_chart_canonical_bytes() as u32,
                maximum_output_bytes: 0,
            }],
            resource_requirements: vec![resource_requirement(
                RUNTIME_MEMORY_RESOURCE_CLASS,
                workspace,
            )],
            authority_requirements: vec![],
        },
    )
    .build())
}
