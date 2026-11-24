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

pub const AMBIENT_CLOCK_FACE_IMPLEMENTATION: &str = "std/resident-ambient-clock-face@1";
pub const AMBIENT_CLOCK_PROJECT_CALL: &str = "conduit.host/resident-ambient-clock-project@1";
pub const AMBIENT_CLOCK_CUSTODY_CALL: &str =
    "conduit.host/resident-ambient-clock-transfer-custody@1";

#[derive(Clone, Copy)]
pub enum AmbientFaceProjection {
    Home,
    Clock,
}

impl AmbientFaceProjection {
    pub fn for_implementation(id: &str) -> Option<Self> {
        match id {
            AMBIENT_HOME_FACE_IMPLEMENTATION => Some(Self::Home),
            AMBIENT_CLOCK_FACE_IMPLEMENTATION => Some(Self::Clock),
            _ => None,
        }
    }

    pub fn implementation(self) -> &'static str {
        match self {
            Self::Home => AMBIENT_HOME_FACE_IMPLEMENTATION,
            Self::Clock => AMBIENT_CLOCK_FACE_IMPLEMENTATION,
        }
    }

    pub fn calls(self) -> (&'static str, &'static str) {
        match self {
            Self::Home => (AMBIENT_HOME_PROJECT_CALL, AMBIENT_HOME_CUSTODY_CALL),
            Self::Clock => (AMBIENT_CLOCK_PROJECT_CALL, AMBIENT_CLOCK_CUSTODY_CALL),
        }
    }

    pub fn kind(self) -> &'static str {
        match self {
            Self::Home => conduit_semantic_catalog::AMBIENT_HOME_FACE_KIND,
            Self::Clock => conduit_semantic_catalog::AMBIENT_CLOCK_FACE_KIND,
        }
    }

    pub fn input_bytes(self) -> u32 {
        match self {
            Self::Home => AMBIENT_HOME_OBSERVATION_BYTES,
            Self::Clock => conduit_semantic_catalog::AMBIENT_CLOCK_OBSERVATION_BYTES,
        }
    }

    pub fn offer(self) -> Result<CapabilityOffer, String> {
        let kind = match self {
            Self::Home => ambient_home_face_kind()?,
            Self::Clock => conduit_semantic_catalog::ambient_clock_face_kind()?,
        };
        let operation = |contract: &'static str| HostCallRequirement {
            contract_id: contract.into(),
            target_kind: Some(kind.kind_id.clone()),
            maximum_in_flight: 1,
            maximum_input_bytes: self.input_bytes(),
            maximum_output_bytes: 0,
        };
        let (project, custody) = self.calls();
        let calls = vec![operation(project), operation(custody)];
        let workspace = ambient_home_workspace_bytes()?;
        let digest = semantic_digest(
            self.implementation(),
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
                capability_id: self.implementation().into(),
                implementation_id: self.implementation().into(),
                execution_profile_id: match self {
                    Self::Home => "std/resident-ambient-home-hosted@1",
                    Self::Clock => "std/resident-ambient-clock-hosted@1",
                }
                .into(),
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
}

pub fn resident_ambient_home_face_offer() -> Result<CapabilityOffer, String> {
    AmbientFaceProjection::Home.offer()
}
