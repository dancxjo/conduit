//! Current situation from explicit typed observations and temporal context.
use alloc::{
    string::{String, ToString},
    vec,
};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, ExternalEffectBehavior, Kind, KindSemanticLaw,
    PortDescriptor, PortDirection, PortTemporal, StructuredInfoType,
};
use conduit_plot::KindSignature;
pub const EXPERIENCE_SITUATION_KIND: &str = "experience/situation-current";
pub const EXPERIENCE_SITUATION_REVISION: &str = "conduit.experience/situation-current@1";
pub fn experience_situation_frame_type() -> StructuredInfoType {
    conduit_human::ExperienceSituationFrame::semantic_type().expect("native Situation frame")
}
pub fn experience_situation_contract() -> Kind {
    let port = |name, ty: StructuredInfoType, direction| PortDescriptor {
        port_id: port_id(name),
        value_kind: ty
            .profile()
            .expect("native Situation profile")
            .value_kind()
            .clone(),
        direction,
        temporal: PortTemporal::Flow { closes: true },
        abnormal_kind: None,
    };
    Kind {
        kind_id: kind_id(EXPERIENCE_SITUATION_KIND),
        kind_contract_revision: EXPERIENCE_SITUATION_REVISION.into(),
        startup_parameters: vec![],
        shorthand: None,
        inputs: vec![port(
            "frame",
            experience_situation_frame_type(),
            PortDirection::Input,
        )],
        outputs: vec![
            port(
                "event",
                crate::experience_autobiography_event_type(),
                PortDirection::Output,
            ),
            port(
                "situation",
                crate::experience_evidence_request_type(),
                PortDirection::Output,
            ),
        ],
        configuration: vec![],
        semantic_laws: vec![KindSemanticLaw::ExternalEffects(
            ExternalEffectBehavior::None,
        )],
        limits: CapabilityLimits {
            max_active_instances: 8,
            max_queue_items: 1,
            max_queue_bytes: crate::EXPERIENCE_EVIDENCE_MAX_BYTES as u32,
        },
    }
}
pub fn install_experience_situation_catalogs(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), String> {
    for (name, ty) in [
        (
            "ExperienceSituationFrame",
            experience_situation_frame_type(),
        ),
        (
            "ExperienceAutobiographyEvent",
            crate::experience_autobiography_event_type(),
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
    let kind = experience_situation_contract();
    startup
        .insert(KindSignature {
            kind: kind.kind_id.as_str().into(),
            startup_parameters: vec![],
        })
        .map_err(|e| e.to_string())?;
    profile.insert_kind(kind).map_err(|e| e.to_string())
}
