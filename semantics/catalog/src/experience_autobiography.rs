//! Explicit finite autobiographical state on ordinary Cords.
use alloc::{
    string::{String, ToString},
    vec,
};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, ExternalEffectBehavior, Kind, KindSemanticLaw,
    PortDescriptor, PortDirection, PortTemporal, StructuredInfoType,
};
use conduit_human::{ExperienceAutobiographyRecords, ExperienceAutobiographyState};
use conduit_plot::rust_binding::BoundedSequence;
use conduit_plot::KindSignature;

pub const EXPERIENCE_AUTOBIOGRAPHY_RETAIN_KIND: &str = "experience/autobiography-retain";
pub const EXPERIENCE_AUTOBIOGRAPHY_RETAIN_REVISION: &str =
    "conduit.experience/autobiography-retain@1";
pub const EXPERIENCE_AUTOBIOGRAPHY_MAX_BYTES: usize = 65_536;
pub const EXPERIENCE_AUTOBIOGRAPHY_MAX_RESPONSE_BYTES: usize = 8_192;
pub fn experience_autobiography_state_type() -> StructuredInfoType {
    ExperienceAutobiographyState::semantic_type().expect("native autobiography state")
}
pub fn experience_autobiography_pending_type() -> StructuredInfoType {
    conduit_human::ExperienceAutobiographyPending::semantic_type()
        .expect("native autobiography pending")
}
pub fn initial_experience_autobiography_state() -> ExperienceAutobiographyState {
    ExperienceAutobiographyState::new(
        0,
        ExperienceAutobiographyRecords::new(BoundedSequence::new()).expect("empty records"),
    )
    .expect("initial autobiography")
}
/// Retain at most two selected episodes, evicting the oldest selected record.
/// Every actual result advances completed, including unselected episodes.
pub fn experience_autobiography_retain_contract() -> Kind {
    let port = |name, value_kind, direction| PortDescriptor {
        port_id: port_id(name),
        value_kind,
        direction,
        temporal: PortTemporal::Flow { closes: true },
        abnormal_kind: None,
    };
    Kind {
        kind_id: kind_id(EXPERIENCE_AUTOBIOGRAPHY_RETAIN_KIND),
        kind_contract_revision: EXPERIENCE_AUTOBIOGRAPHY_RETAIN_REVISION.into(),
        startup_parameters: vec![],
        shorthand: None,
        inputs: vec![
            port(
                "pending",
                experience_autobiography_pending_type()
                    .profile()
                    .expect("pending profile")
                    .value_kind()
                    .clone(),
                PortDirection::Input,
            ),
            port(
                "result",
                kind_id("llm/generated-result@1"),
                PortDirection::Input,
            ),
        ],
        outputs: vec![port(
            "next",
            experience_autobiography_state_type()
                .profile()
                .expect("state profile")
                .value_kind()
                .clone(),
            PortDirection::Output,
        )],
        configuration: vec![],
        semantic_laws: vec![KindSemanticLaw::ExternalEffects(
            ExternalEffectBehavior::None,
        )],
        limits: CapabilityLimits {
            max_active_instances: 8,
            max_queue_items: 1,
            max_queue_bytes: EXPERIENCE_AUTOBIOGRAPHY_MAX_BYTES as u32,
        },
    }
}
pub fn install_experience_autobiography_catalogs(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), String> {
    for (name, ty) in [
        (
            "ExperienceAutobiographyState",
            experience_autobiography_state_type(),
        ),
        (
            "ExperienceAutobiographyPending",
            experience_autobiography_pending_type(),
        ),
    ] {
        startup
            .ensure_structured_type(name, ty)
            .map_err(|e| e.to_string())?;
    }
    let kind = experience_autobiography_retain_contract();
    startup
        .insert(KindSignature {
            kind: kind.kind_id.as_str().into(),
            startup_parameters: vec![],
        })
        .map_err(|e| e.to_string())?;
    profile.insert_kind(kind).map_err(|e| e.to_string())
}
