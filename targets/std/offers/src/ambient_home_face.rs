//! Planned resources and operations for the hosted Home Face contribution.
use conduit_core::{
    resource_requirement, semantic_digest, Back, BackOfferBuilder, CapabilityOffer,
    HostCallRequirement, FACE_RESOURCE_CLASS, RUNTIME_MEMORY_RESOURCE_CLASS,
};
use conduit_semantic_catalog::{
    ambient_home_face_kind, ambient_home_workspace_bytes, AMBIENT_HOME_OBSERVATION_BYTES,
};

pub const AMBIENT_HOME_FACE_IMPLEMENTATION: &str = "std/resident-ambient-home@1";
pub const AMBIENT_HOME_FACE_BASE: &str = "face/ambient-home";
pub const AMBIENT_HOME_FACE_DRIVER: &str = "std/ambient-home-projection@1";
pub const AMBIENT_HOME_PROJECT_CALL: &str = "conduit.host/resident-ambient-home-project@1";
pub const AMBIENT_HOME_CUSTODY_CALL: &str = "conduit.host/resident-ambient-home-transfer-custody@1";

pub fn resident_ambient_home_face_offer() -> Result<CapabilityOffer, String> {
    let kind = ambient_home_face_kind()?;
    let operation = |contract: &'static str| HostCallRequirement {
        contract_id: contract.into(),
        target_kind: Some(kind.kind_id.clone()),
        maximum_in_flight: 1,
        maximum_input_bytes: AMBIENT_HOME_OBSERVATION_BYTES,
        maximum_output_bytes: 0,
    };
    let calls = vec![
        operation(AMBIENT_HOME_PROJECT_CALL),
        operation(AMBIENT_HOME_CUSTODY_CALL),
    ];
    let workspace = ambient_home_workspace_bytes()?;
    let digest = semantic_digest(
        AMBIENT_HOME_FACE_IMPLEMENTATION,
        concat!(
            include_str!("ambient_home_face.rs"),
            include_str!("../../src/installed_std/ambient_home_back.rs"),
            include_str!("../../src/installed_std/ambient_home_factory.rs"),
            include_str!("../../src/installed_std/ambient_home_route.rs"),
            include_str!("../../src/body_execution/face_contribution.rs"),
            include_str!("../../src/body_execution/face_contribution/gate.rs"),
            include_str!("../../src/body_execution/face_contribution/retention.rs"),
            include_str!("../../../../plots/ambient-display/model/src/face.rs"),
            include_str!("../../../../plots/ambient-display/model/src/home.rs"),
            include_str!("../../../../plots/ambient-display/model/src/civil.rs"),
            include_str!("../../../../plots/ambient-display/model/src/availability.rs"),
            include_str!("../../../../plots/ambient-display/model/src/subject_identity.rs"),
            include_str!("../../../../plots/ambient-display/model/src/projection_memory.rs"),
            include_str!("../../../../plots/ambient-display/model/src/backing.rs"),
            include_str!("../../../../plots/ambient-display/types.conduit")
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
            capability_id: AMBIENT_HOME_FACE_IMPLEMENTATION.into(),
            implementation_id: AMBIENT_HOME_FACE_IMPLEMENTATION.into(),
            execution_profile_id: "std/resident-ambient-home-hosted@1".into(),
            artifact_id: artifact.into(),
            host_calls: calls,
            resource_requirements: vec![
                resource_requirement(FACE_RESOURCE_CLASS, 1),
                resource_requirement(RUNTIME_MEMORY_RESOURCE_CLASS, workspace),
            ],
            authority_requirements: vec![],
        },
    )
    .build())
}
