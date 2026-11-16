//! Finite native hosted realization of the existing final-window semantic Kind.
use conduit_core::*;
pub const MEASUREMENT_WINDOW_IMPLEMENTATION: &str = "std/measurement-count-window@1";
pub const MEASUREMENT_WINDOW_PROFILE_CALL: &str = "conduit.host/measurement-window-profile@1";
pub const MEASUREMENT_WINDOW_SAMPLE_CALL: &str = "conduit.host/measurement-window-sample@1";
pub use conduit_data::measurement_window_value_bounds;
pub fn measurement_window_workspace_bytes() -> u32 {
    conduit_data::measurement_window_dynamic_workspace_bytes()
        .try_into()
        .expect("window workspace")
}
pub fn measurement_window_std_offer() -> CapabilityOffer {
    let contract = conduit_data::measurement_window_semantic_contract();
    let kind = contract.kind_id.clone();
    let b = measurement_window_value_bounds();
    let digest = semantic_digest(
        "std/measurement-count-window@1",
        concat!(
            include_str!("measurement_window.rs"),
            include_str!("../../../../semantics/data/src/measurement_window_step.rs"),
            include_str!("../../../../semantics/data/src/measurement_window_provider.rs"),
            include_str!("../../../../semantics/data/src/measurement_window.rs"),
            include_str!("../../../../semantics/data/src/measurement_wire.rs"),
            include_str!("../../../../semantics/data/src/measurement_profile_wire.rs"),
            include_str!("../../../../semantics/data/types.conduit")
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
    let mut offer = BackOfferBuilder::new(
        contract,
        Back {
            capability_id: MEASUREMENT_WINDOW_IMPLEMENTATION.into(),
            execution_profile_id: "std/measurement-count-window-hosted@1".into(),
            implementation_id: MEASUREMENT_WINDOW_IMPLEMENTATION.into(),
            artifact_id: artifact.into(),
            host_calls: vec![
                HostCallRequirement {
                    contract_id: MEASUREMENT_WINDOW_PROFILE_CALL.into(),
                    target_kind: Some(kind.clone()),
                    maximum_in_flight: 1,
                    maximum_input_bytes: b[0],
                    maximum_output_bytes: 0,
                },
                HostCallRequirement {
                    contract_id: MEASUREMENT_WINDOW_SAMPLE_CALL.into(),
                    target_kind: Some(kind),
                    maximum_in_flight: 1,
                    maximum_input_bytes: b[1],
                    maximum_output_bytes: b[2],
                },
            ],
            resource_requirements: vec![resource_requirement(
                RUNTIME_MEMORY_RESOURCE_CLASS,
                measurement_window_workspace_bytes(),
            )],
            authority_requirements: vec![],
        },
    )
    .build();
    offer.limits.max_queue_items = 2;
    offer.limits.max_queue_bytes = b[2].max(b[0]).max(b[1]);
    offer
}
