//! Exact installed projection/custody operations for reported original recording generations.
use conduit_core::{
    resource_requirement, semantic_digest, Back, BackOfferBuilder, CapabilityOffer,
    HostCallRequirement, FACE_RESOURCE_CLASS, RUNTIME_MEMORY_RESOURCE_CLASS,
};

pub const REPORTED_GENERATION_FACE_BASE: &str = "face/reported-generation-series";
pub const REPORTED_GENERATION_FACE_DRIVER: &str = "std/reported-generation-face@1";
pub const REPORTED_GENERATION_FACE_IMPLEMENTATION: &str = "std/reported-generation-series@1";
pub const REPORTED_GENERATION_PROJECT_CALL: &str = "conduit.host/reported-generation-project@1";
pub const REPORTED_GENERATION_CUSTODY_CALL: &str =
    "conduit.host/reported-generation-transfer-custody@1";

pub fn reported_generation_face_offer() -> Result<CapabilityOffer, String> {
    let kind = conduit_semantic_catalog::reported_generation_face_kind()?;
    let maximum = conduit_semantic_catalog::reported_generation_maximum_bytes()?;
    let operation = |contract: &'static str| HostCallRequirement {
        contract_id: contract.into(),
        target_kind: Some(kind.kind_id.clone()),
        maximum_in_flight: 1,
        maximum_input_bytes: maximum,
        maximum_output_bytes: 0,
    };
    let calls = vec![
        operation(REPORTED_GENERATION_PROJECT_CALL),
        operation(REPORTED_GENERATION_CUSTODY_CALL),
    ];
    let digest = semantic_digest(
        REPORTED_GENERATION_FACE_IMPLEMENTATION,
        concat!(
            include_str!("reported_generation_face.rs"),
            include_str!("../../src/installed_std/reported_generation_factory.rs"),
            include_str!("../../src/installed_std/reported_generation_route.rs"),
            include_str!("../../src/installed_std/face_contribution_back.rs"),
            include_str!("../../src/installed_std/prepared_face_route.rs"),
            include_str!("../../src/installed_std/face_contribution_route.rs"),
            include_str!("../../src/installed_std/body_kernel/face_contributions.rs"),
            include_str!("../../src/body_execution/face_contribution.rs"),
            include_str!("../../src/body_execution/face_contribution/gate.rs"),
            include_str!("../../src/body_execution/face_contribution/retention.rs"),
            include_str!("../../src/image_initialization/reported_generation_face.rs"),
            include_str!("../../../../semantics/catalog/src/reported_generation_face.rs"),
            include_str!("../../../../semantics/face/src/quantity_observation_face.rs"),
            include_str!("../../../../semantics/face/src/quantity_observation_face/builder.rs"),
            include_str!("../../../../semantics/face/src/quantity_observation_face/model.rs"),
            include_str!("../../../../semantics/face/src/inspection_fragment_memory.rs"),
            include_str!("../../../../mechanisms/devices/dht11/src/lib.rs"),
            include_str!("../../../../mechanisms/devices/dht11/src/packed.rs"),
            include_str!("../../../../mechanisms/devices/dht11/src/recording.rs"),
            include_str!("../../../../mechanisms/devices/dht11/src/recording/reader.rs"),
            include_str!("../../../../mechanisms/devices/dht11/src/recording/history.rs"),
            include_str!("../../../../mechanisms/devices/dht11/src/recording_backing.rs"),
            include_str!("../../../../mechanisms/devices/dht11/src/reported_generation.rs"),
            include_str!("../../../../mechanisms/devices/dht11/src/reported_generation/wire.rs"),
            include_str!("../../../../mechanisms/devices/dht11/src/reported_generation/schema.rs"),
            include_str!("../../../../mechanisms/devices/dht11/src/reported_generation/backing.rs"),
            include_str!("../../../../mechanisms/devices/dht11/build.rs"),
            include_str!("../../../../mechanisms/devices/dht11/build_recording.rs"),
            include_str!("../../../../mechanisms/devices/dht11/recording.conduit"),
            include_str!("../../../../plots/device-protocols/dht11.conduit"),
            include_str!("../../../../plots/device-protocols/pulse-types.conduit"),
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
            capability_id: REPORTED_GENERATION_FACE_IMPLEMENTATION.into(),
            implementation_id: REPORTED_GENERATION_FACE_IMPLEMENTATION.into(),
            execution_profile_id: "std/reported-generation-face-hosted@1".into(),
            artifact_id: artifact.into(),
            host_calls: calls,
            resource_requirements: vec![
                resource_requirement(FACE_RESOURCE_CLASS, 1),
                resource_requirement(
                    RUNTIME_MEMORY_RESOURCE_CLASS,
                    conduit_semantic_catalog::reported_generation_face_workspace_bytes()?,
                ),
            ],
            authority_requirements: vec![],
        },
    )
    .build())
}
