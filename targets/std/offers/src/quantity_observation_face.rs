//! Exact installed projection/custody operations for reported quantity history.
use conduit_core::{
    resource_requirement, semantic_digest, Back, BackOfferBuilder, CapabilityOffer,
    HostCallRequirement, FACE_RESOURCE_CLASS, RUNTIME_MEMORY_RESOURCE_CLASS,
};

pub const QUANTITY_OBSERVATION_FACE_BASE: &str = "face/quantity-observation-series";
pub const QUANTITY_OBSERVATION_FACE_DRIVER: &str = "std/quantity-observation-projection@1";
pub const QUANTITY_OBSERVATION_FACE_IMPLEMENTATION: &str = "std/quantity-observation-face@1";
pub const QUANTITY_OBSERVATION_PROJECT_CALL: &str = "conduit.host/quantity-observation-project@1";
pub const QUANTITY_OBSERVATION_CUSTODY_CALL: &str =
    "conduit.host/quantity-observation-transfer-custody@1";

pub fn quantity_observation_face_offer() -> Result<CapabilityOffer, String> {
    let kind = conduit_semantic_catalog::quantity_observation_face_kind()?;
    let maximum = conduit_data::resolved_quantity_observation_snapshot_byte_bound()
        .map_err(|why| format!("quantity input: {why:?}"))?;
    let operation = |contract: &'static str| HostCallRequirement {
        contract_id: contract.into(),
        target_kind: Some(kind.kind_id.clone()),
        maximum_in_flight: 1,
        maximum_input_bytes: maximum,
        maximum_output_bytes: 0,
    };
    let calls = vec![
        operation(QUANTITY_OBSERVATION_PROJECT_CALL),
        operation(QUANTITY_OBSERVATION_CUSTODY_CALL),
    ];
    let digest = semantic_digest(
        QUANTITY_OBSERVATION_FACE_IMPLEMENTATION,
        concat!(
            include_str!("quantity_observation_face.rs"),
            include_str!("../../src/installed_std/quantity_observation_factory.rs"),
            include_str!("../../src/installed_std/quantity_observation_route.rs"),
            include_str!("../../src/installed_std/face_contribution_back.rs"),
            include_str!("../../src/installed_std/prepared_face_route.rs"),
            include_str!("../../src/installed_std/face_contribution_route.rs"),
            include_str!("../../src/installed_std/body_kernel/face_contributions.rs"),
            include_str!("../../src/body_execution/face_contribution.rs"),
            include_str!("../../src/body_execution/face_contribution/gate.rs"),
            include_str!("../../src/body_execution/face_contribution/retention.rs"),
            include_str!("../../../../semantics/catalog/src/quantity_observation_face.rs"),
            include_str!("../../../../semantics/face/src/quantity_observation_face.rs"),
            include_str!("../../../../semantics/face/src/quantity_observation_face/builder.rs"),
            include_str!("../../../../semantics/face/src/quantity_observation_face/model.rs"),
            include_str!("../../../../semantics/face/src/quantity_observation_snapshot_face.rs"),
            include_str!("../../../../semantics/data/src/quantity_observation_snapshot.rs"),
            include_str!("../../../../semantics/face/src/inspection_fragment_memory.rs"),
            include_str!("../../../../semantics/data/src/quantity_observation_input.rs"),
            include_str!("../../../../semantics/data/types.conduit")
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
        kind,
        Back {
            capability_id: QUANTITY_OBSERVATION_FACE_IMPLEMENTATION.into(),
            implementation_id: QUANTITY_OBSERVATION_FACE_IMPLEMENTATION.into(),
            execution_profile_id: "std/quantity-observation-face-hosted@1".into(),
            artifact_id: artifact.into(),
            host_calls: calls,
            resource_requirements: vec![
                resource_requirement(FACE_RESOURCE_CLASS, 1),
                resource_requirement(
                    RUNTIME_MEMORY_RESOURCE_CLASS,
                    conduit_semantic_catalog::quantity_observation_face_workspace_bytes()?,
                ),
            ],
            authority_requirements: vec![],
        },
    )
    .build())
}
