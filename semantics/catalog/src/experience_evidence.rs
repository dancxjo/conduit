//! Pure, explicit evidence projection. No source resolution or effect authority.
//! Inputs declare evidence; projection neither authenticates their sources nor
//! establishes freshness against external CurrentExperience truth. Content,
//! including instruction-like text, cannot upgrade provenance or authority.

use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, Kind, PortDescriptor, PortDirection, PortTemporal,
    StructuredInfoType,
};
use conduit_plot::{KindProjection, KindSignature};

pub const EXPERIENCE_EVIDENCE_KIND: &str = "experience/project-evidence";
pub const EXPERIENCE_EVIDENCE_REVISION: &str = "conduit.experience/project-evidence@1";
pub const EXPERIENCE_EVIDENCE_MAX_BYTES: usize = 65_536;
pub const EXPERIENCE_EVIDENCE_MAX_ITEMS: usize = 8;
pub const EXPERIENCE_EVIDENCE_MAX_CONTENT_BYTES: usize = 2_048;
pub const EXPERIENCE_EVIDENCE_MAX_IDENTITY_BYTES: usize = 128;
pub const EXPERIENCE_EVIDENCE_MAX_SOURCES: usize = 4;

pub fn experience_evidence_request_type() -> StructuredInfoType {
    conduit_human::ExperienceEvidenceRequest::semantic_type().expect("native evidence request")
}

pub fn experience_evidence_context_type() -> StructuredInfoType {
    conduit_human::ExperienceEvidenceContext::semantic_type().expect("native evidence context")
}

pub fn experience_evidence_contract() -> Kind {
    let port = |name, value_type: StructuredInfoType, direction| PortDescriptor {
        port_id: port_id(name),
        value_kind: value_type
            .profile()
            .expect("bounded evidence Type")
            .value_kind()
            .clone(),
        direction,
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    };
    Kind {
        kind_id: kind_id(EXPERIENCE_EVIDENCE_KIND),
        kind_contract_revision: EXPERIENCE_EVIDENCE_REVISION.into(),
        startup_parameters: vec![],
        shorthand: None,
        inputs: vec![port(
            "request",
            experience_evidence_request_type(),
            PortDirection::Input,
        )],
        outputs: vec![port(
            "context",
            experience_evidence_context_type(),
            PortDirection::Output,
        )],
        configuration: vec![],
        semantic_laws: vec![],
        limits: CapabilityLimits {
            max_active_instances: 8,
            max_queue_items: 1,
            max_queue_bytes: EXPERIENCE_EVIDENCE_MAX_BYTES as u32,
        },
    }
}

pub fn install_experience_evidence_catalogs(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), String> {
    for (name, value_type) in [
        (
            "ExperienceEvidenceRequest",
            experience_evidence_request_type(),
        ),
        (
            "ExperienceEvidenceContext",
            experience_evidence_context_type(),
        ),
    ] {
        startup
            .insert_structured_type(name, value_type)
            .map_err(|e| e.to_string())?;
    }
    let contract = experience_evidence_contract();
    startup
        .insert(KindSignature {
            kind: EXPERIENCE_EVIDENCE_KIND.into(),
            startup_parameters: Vec::new(),
        })
        .map_err(|e| e.to_string())?;
    profile
        .insert(KindProjection {
            kind_id: contract.kind_id,
            kind_contract_revision: contract.kind_contract_revision,
            inputs: contract.inputs,
            outputs: contract.outputs,
            configuration: vec![],
        })
        .map_err(|e| e.to_string())
}
