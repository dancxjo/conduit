//! Finite exact-kind/domain recall over explicitly supplied autobiography.
use alloc::{
    string::{String, ToString},
    vec,
};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, ExternalEffectBehavior, Kind, KindSemanticLaw,
    PortDescriptor, PortDirection, PortTemporal, StructuredInfoType, TemporalStateBehavior,
};
use conduit_plot::KindSignature;
pub const EXPERIENCE_AUTOBIOGRAPHY_RECALL_KIND: &str = "experience/autobiography-recall";
pub const EXPERIENCE_AUTOBIOGRAPHY_RECALL_REVISION: &str =
    "conduit.experience/autobiography-recall@1";
pub const EXPERIENCE_AUTOBIOGRAPHY_RECALL_POLICY: &str = "latest-exact-content-kind-domain@1";
pub fn experience_autobiography_event_type() -> StructuredInfoType {
    conduit_human::ExperienceAutobiographyEvent::semantic_type()
        .expect("native autobiography event")
}
pub fn experience_autobiography_recall_contract() -> Kind {
    let port = |name, ty: StructuredInfoType, direction, temporal| PortDescriptor {
        port_id: port_id(name),
        value_kind: ty
            .profile()
            .expect("native recall profile")
            .value_kind()
            .clone(),
        direction,
        temporal,
        abnormal_kind: None,
    };
    Kind {
        kind_id: kind_id(EXPERIENCE_AUTOBIOGRAPHY_RECALL_KIND),
        kind_contract_revision: EXPERIENCE_AUTOBIOGRAPHY_RECALL_REVISION.into(),
        startup_parameters: vec![],
        shorthand: None,
        inputs: vec![
            port(
                "event",
                experience_autobiography_event_type(),
                PortDirection::Input,
                PortTemporal::Flow { closes: true },
            ),
            port(
                "memory",
                crate::experience_autobiography_state_type(),
                PortDirection::Input,
                PortTemporal::Current,
            ),
        ],
        outputs: vec![
            port(
                "update",
                crate::experience_integration_update_type(),
                PortDirection::Output,
                PortTemporal::Flow { closes: true },
            ),
            port(
                "pending",
                crate::experience_autobiography_pending_type(),
                PortDirection::Output,
                PortTemporal::Flow { closes: true },
            ),
        ],
        configuration: vec![],
        semantic_laws: vec![
            KindSemanticLaw::ExternalEffects(ExternalEffectBehavior::None),
            KindSemanticLaw::TemporalState(TemporalStateBehavior::Retained),
        ],
        limits: CapabilityLimits {
            max_active_instances: 8,
            max_queue_items: 1,
            max_queue_bytes: crate::EXPERIENCE_AUTOBIOGRAPHY_MAX_BYTES as u32,
        },
    }
}
pub fn install_experience_autobiography_recall_catalogs(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), String> {
    for (name, ty) in [
        (
            "ExperienceAutobiographyEvent",
            experience_autobiography_event_type(),
        ),
        (
            "ExperienceAutobiographyState",
            crate::experience_autobiography_state_type(),
        ),
        (
            "ExperienceAutobiographyPending",
            crate::experience_autobiography_pending_type(),
        ),
        (
            "ExperienceIntegrationUpdate",
            crate::experience_integration_update_type(),
        ),
    ] {
        startup
            .ensure_structured_type(name, ty)
            .map_err(|e| e.to_string())?;
    }
    let kind = experience_autobiography_recall_contract();
    startup
        .insert(KindSignature {
            kind: kind.kind_id.as_str().into(),
            startup_parameters: vec![],
        })
        .map_err(|e| e.to_string())?;
    profile.insert_kind(kind).map_err(|e| e.to_string())
}
