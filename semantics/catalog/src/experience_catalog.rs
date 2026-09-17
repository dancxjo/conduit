//! Portable typed boundary for the generic bounded Experiencer.

use alloc::{string::ToString, vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, KindContractRevision, PortDescriptor, PortDirection, PortTemporal,
    StructuredFieldType, StructuredInfoType, StructuredVariantCase,
};
use conduit_form::{KindDefinition, KindSignature};

pub const EXPERIENCE_RELATE_KIND: &str = "experience/relate-current";
pub const EXPERIENCE_RELATE_REVISION: &str = "conduit.experience/relate-current@1";
pub const EXPERIENCE_HUMAN_INPUT_TYPE: &str = "ExperienceHumanInput";
pub const EXPERIENCE_BODY_INPUT_TYPE: &str = "ExperienceBodyInput";
pub const EXPERIENCE_MEMORY_INPUT_TYPE: &str = "ExperienceMemoryInput";
pub const EXPERIENCE_INFERENCE_INPUT_TYPE: &str = "ExperienceInferenceInput";
pub const EXPERIENCE_HYPOTHESIS_INPUT_TYPE: &str = "ExperienceHypothesisInput";
pub const EXPERIENCE_SOURCE_STATUS_TYPE: &str = "ExperienceSourceStatus";
pub const CURRENT_EXPERIENCE_TYPE: &str = "CurrentExperience";

fn text() -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id("value/text@1")).expect("reviewed text")
}

fn count() -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id("value/count@1")).expect("reviewed count")
}

fn unit() -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id("value/unit@1")).expect("reviewed unit")
}

fn field(name: &str, value_type: StructuredInfoType) -> StructuredFieldType {
    StructuredFieldType::new(name, value_type).expect("reviewed experience field")
}

fn record(kind: &str, fields: Vec<StructuredFieldType>) -> StructuredInfoType {
    StructuredInfoType::record(kind_id(kind), fields).expect("reviewed experience record")
}

fn case(name: &str) -> StructuredVariantCase {
    StructuredVariantCase::new(name, unit()).expect("reviewed experience case")
}

fn source_input(kind: &str) -> StructuredInfoType {
    record(
        kind,
        vec![
            field("item_identity", text()),
            field("source_identity", text()),
            field("observed_at", text()),
            field("content", text()),
        ],
    )
}

pub fn experience_human_input_type() -> StructuredInfoType {
    source_input("experience/human-input@1")
}

pub fn experience_body_input_type() -> StructuredInfoType {
    source_input("experience/body-input@1")
}

pub fn experience_memory_input_type() -> StructuredInfoType {
    source_input("experience/memory-input@1")
}

pub fn experience_inference_input_type() -> StructuredInfoType {
    source_input("experience/model-inference-input@1")
}

pub fn experience_hypothesis_input_type() -> StructuredInfoType {
    source_input("experience/hypothesis-input@1")
}

pub fn experience_source_status_type() -> StructuredInfoType {
    record(
        "experience/source-status@1",
        vec![
            field("source_identity", text()),
            field(
                "availability",
                StructuredInfoType::variant(
                    kind_id("experience/source-availability@1"),
                    vec![case("present"), case("missing"), case("unavailable")],
                )
                .expect("reviewed source availability"),
            ),
        ],
    )
}

pub fn current_experience_type() -> StructuredInfoType {
    record(
        "experience/current@1",
        vec![
            field("revision", count()),
            field(
                "item_identities",
                StructuredInfoType::collection(text(), Some(32))
                    .expect("bounded current experience identities"),
            ),
            field("provenance_revision", text()),
        ],
    )
}

pub fn install_experience_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    for (name, value_type) in [
        (EXPERIENCE_HUMAN_INPUT_TYPE, experience_human_input_type()),
        (EXPERIENCE_BODY_INPUT_TYPE, experience_body_input_type()),
        (EXPERIENCE_MEMORY_INPUT_TYPE, experience_memory_input_type()),
        (EXPERIENCE_INFERENCE_INPUT_TYPE, experience_inference_input_type()),
        (EXPERIENCE_HYPOTHESIS_INPUT_TYPE, experience_hypothesis_input_type()),
        (EXPERIENCE_SOURCE_STATUS_TYPE, experience_source_status_type()),
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
        .insert(KindDefinition {
            kind_id: kind_id(EXPERIENCE_RELATE_KIND),
            kind_contract_revision: KindContractRevision::from(EXPERIENCE_RELATE_REVISION),
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
    }
}
