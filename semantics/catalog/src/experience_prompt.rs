//! Bounded model wording from explicit evidence, without provider authority.
use alloc::{
    string::{String, ToString},
    vec,
};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, Kind, PortDescriptor, PortDirection, PortTemporal,
};
use conduit_plot::{KindProjection, KindSignature};

pub const EXPERIENCE_PROMPT_KIND: &str = "experience/context-to-prompt";
pub const EXPERIENCE_PROMPT_REVISION: &str = "conduit.experience/context-to-prompt@1";
pub const EXPERIENCE_GENERATION_REQUEST_KIND: &str = "llm/generation-request@1";
pub const EXPERIENCE_PROMPT_MAX_BYTES: usize = 4096;

pub fn experience_prompt_contract() -> Kind {
    Kind {
        kind_id: kind_id(EXPERIENCE_PROMPT_KIND),
        kind_contract_revision: EXPERIENCE_PROMPT_REVISION.into(),
        startup_parameters: vec![],
        shorthand: None,
        inputs: vec![PortDescriptor {
            port_id: port_id("context"),
            value_kind: crate::experience_evidence_context_type()
                .profile()
                .expect("bounded context")
                .value_kind()
                .clone(),
            direction: PortDirection::Input,
            temporal: PortTemporal::Flow { closes: true },
            abnormal_kind: None,
        }],
        outputs: vec![PortDescriptor {
            port_id: port_id("request"),
            value_kind: kind_id(EXPERIENCE_GENERATION_REQUEST_KIND),
            direction: PortDirection::Output,
            temporal: PortTemporal::Flow { closes: true },
            abnormal_kind: None,
        }],
        configuration: vec![],
        semantic_laws: vec![],
        limits: CapabilityLimits {
            max_active_instances: 8,
            max_queue_items: 1,
            max_queue_bytes: crate::EXPERIENCE_EVIDENCE_MAX_BYTES as u32,
        },
    }
}
pub fn install_experience_prompt_catalogs(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), String> {
    startup
        .ensure_structured_type(
            "ExperienceEvidenceContext",
            crate::experience_evidence_context_type(),
        )
        .map_err(|e| e.to_string())?;
    startup.insert_value_kind_alias(
        "ExperienceModelRequest",
        kind_id(EXPERIENCE_GENERATION_REQUEST_KIND),
    )?;
    let kind = experience_prompt_contract();
    startup
        .insert(KindSignature {
            kind: kind.kind_id.as_str().into(),
            startup_parameters: vec![],
        })
        .map_err(|e| e.to_string())?;
    profile
        .insert(KindProjection {
            kind_id: kind.kind_id,
            kind_contract_revision: kind.kind_contract_revision,
            inputs: kind.inputs,
            outputs: kind.outputs,
            configuration: vec![],
        })
        .map_err(|e| e.to_string())
}
