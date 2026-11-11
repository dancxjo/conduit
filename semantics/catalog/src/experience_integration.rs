//! Ordinary bounded integration, without source lookup or effect authority.
use alloc::{
    string::{String, ToString},
    vec,
};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, ExternalEffectBehavior, Kind, KindSemanticLaw,
    PortDescriptor, PortDirection, PortTemporal, StructuredInfoType, TemporalStateBehavior,
};
use conduit_plot::KindSignature;

pub const EXPERIENCE_INTEGRATION_KIND: &str = "experience/integrate-current";
pub const EXPERIENCE_INTEGRATION_REVISION: &str = "conduit.experience/integrate-current@1";

pub fn experience_integration_update_type() -> StructuredInfoType {
    conduit_human::ExperienceIntegrationUpdate::semantic_type().expect("native integration update")
}

pub fn experience_integration_contract() -> Kind {
    let port = |name, ty: StructuredInfoType, direction| PortDescriptor {
        port_id: port_id(name),
        value_kind: ty
            .profile()
            .expect("bounded integration Type")
            .value_kind()
            .clone(),
        direction,
        temporal: PortTemporal::Flow { closes: true },
        abnormal_kind: None,
    };
    Kind {
        kind_id: kind_id(EXPERIENCE_INTEGRATION_KIND),
        kind_contract_revision: EXPERIENCE_INTEGRATION_REVISION.into(),
        startup_parameters: vec![],
        shorthand: None,
        inputs: vec![port(
            "update",
            experience_integration_update_type(),
            PortDirection::Input,
        )],
        outputs: vec![port(
            "request",
            crate::experience_evidence_request_type(),
            PortDirection::Output,
        )],
        configuration: vec![],
        semantic_laws: vec![
            KindSemanticLaw::ExternalEffects(ExternalEffectBehavior::None),
            KindSemanticLaw::TemporalState(TemporalStateBehavior::Retained),
        ],
        limits: CapabilityLimits {
            max_active_instances: 8,
            max_queue_items: 1,
            max_queue_bytes: crate::EXPERIENCE_EVIDENCE_MAX_BYTES as u32,
        },
    }
}

pub fn install_experience_integration_catalogs(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), String> {
    for (name, ty) in [
        (
            "ExperienceIntegrationUpdate",
            experience_integration_update_type(),
        ),
        (
            "ExperienceEvidenceRequest",
            crate::experience_evidence_request_type(),
        ),
    ] {
        startup
            .ensure_structured_type(name, ty)
            .map_err(|e| e.to_string())?;
    }
    let kind = experience_integration_contract();
    startup
        .insert(KindSignature {
            kind: kind.kind_id.as_str().into(),
            startup_parameters: vec![],
        })
        .map_err(|e| e.to_string())?;
    profile.insert_kind(kind).map_err(|e| e.to_string())
}
