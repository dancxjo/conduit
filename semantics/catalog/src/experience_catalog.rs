//! Portable typed boundary for the generic bounded Experiencer.

use alloc::{
    string::{String, ToString},
    vec,
};
use conduit_core::{
    kind_id, port_id, KindIdentity, PortDescriptor, PortDirection, PortTemporal, StructuredInfoType,
};
use conduit_plot::{KindProjection, KindSignature};

pub const EXPERIENCE_RELATE_KIND: &str = "experience/relate-current";
pub const EXPERIENCE_RELATE_REVISION: &str = "conduit.experience/relate-current@1";
pub const EXPERIENCE_HUMAN_INPUT_TYPE: &str = "ExperienceHumanInput";
pub const EXPERIENCE_BODY_INPUT_TYPE: &str = "ExperienceBodyInput";
pub const EXPERIENCE_MEMORY_INPUT_TYPE: &str = "ExperienceMemoryInput";
pub const EXPERIENCE_INFERENCE_INPUT_TYPE: &str = "ExperienceInferenceInput";
pub const EXPERIENCE_HYPOTHESIS_INPUT_TYPE: &str = "ExperienceHypothesisInput";
pub const EXPERIENCE_SOURCE_STATUS_TYPE: &str = "ExperienceSourceStatus";
pub const CURRENT_EXPERIENCE_TYPE: &str = "CurrentExperience";

pub fn experience_human_input_type() -> StructuredInfoType {
    conduit_human::ExperienceHumanInput::semantic_type().expect("checked human input Type")
}

pub fn experience_body_input_type() -> StructuredInfoType {
    conduit_human::ExperienceBodyInput::semantic_type().expect("checked body input Type")
}

pub fn experience_memory_input_type() -> StructuredInfoType {
    conduit_human::ExperienceMemoryInput::semantic_type().expect("checked memory input Type")
}

pub fn experience_inference_input_type() -> StructuredInfoType {
    conduit_human::ExperienceInferenceInput::semantic_type().expect("checked inference input Type")
}

pub fn experience_hypothesis_input_type() -> StructuredInfoType {
    conduit_human::ExperienceHypothesisInput::semantic_type()
        .expect("checked hypothesis input Type")
}

pub fn experience_source_status_type() -> StructuredInfoType {
    conduit_human::ExperienceSourceStatus::semantic_type().expect("checked source status Type")
}

pub fn current_experience_type() -> StructuredInfoType {
    conduit_human::CurrentExperienceProjection::semantic_type()
        .expect("checked current experience projection Type")
}

pub fn install_experience_catalogs(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), String> {
    for (name, value_type) in [
        (EXPERIENCE_HUMAN_INPUT_TYPE, experience_human_input_type()),
        (EXPERIENCE_BODY_INPUT_TYPE, experience_body_input_type()),
        (EXPERIENCE_MEMORY_INPUT_TYPE, experience_memory_input_type()),
        (
            EXPERIENCE_INFERENCE_INPUT_TYPE,
            experience_inference_input_type(),
        ),
        (
            EXPERIENCE_HYPOTHESIS_INPUT_TYPE,
            experience_hypothesis_input_type(),
        ),
        (
            EXPERIENCE_SOURCE_STATUS_TYPE,
            experience_source_status_type(),
        ),
        (CURRENT_EXPERIENCE_TYPE, current_experience_type()),
    ] {
        startup
            .insert_structured_type(name, value_type)
            .map_err(|error| error.to_string())?;
    }
    startup
        .insert(KindSignature {
            kind: EXPERIENCE_RELATE_KIND.into(),
            startup_parameters: vec![],
        })
        .map_err(|error| error.to_string())?;
    profile
        .insert(KindProjection {
            kind_id: kind_id(EXPERIENCE_RELATE_KIND),
            kind_contract_revision: KindIdentity::from(EXPERIENCE_RELATE_REVISION),
            inputs: vec![
                flow_port("body", &experience_body_input_type()),
                flow_port("human", &experience_human_input_type()),
                flow_port("hypothesis", &experience_hypothesis_input_type()),
                flow_port("inference", &experience_inference_input_type()),
                flow_port("memory", &experience_memory_input_type()),
                flow_port("source_status", &experience_source_status_type()),
                flow_port("visual", &crate::visual_experience_type()),
            ],
            outputs: vec![PortDescriptor {
                port_id: port_id("current"),
                value_kind: current_experience_type()
                    .profile()
                    .expect("reviewed current experience")
                    .value_kind()
                    .clone(),
                direction: PortDirection::Output,
                temporal: PortTemporal::Current,
                abnormal_kind: None,
            }],
            configuration: vec![],
        })
        .map_err(|error| error.to_string())
}

fn flow_port(name: &str, value_type: &StructuredInfoType) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: value_type
            .profile()
            .expect("reviewed experience profile")
            .value_kind()
            .clone(),
        direction: PortDirection::Input,
        temporal: PortTemporal::Flow { closes: false },
        abnormal_kind: None,
    }
}
