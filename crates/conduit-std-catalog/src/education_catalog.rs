//! Canonical portable education Form catalog.

use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};
use conduit_core::{
    kind_id, port_id, KindContractRevision, KindId, PortDescriptor, PortDirection, PortTemporal,
    StructuredInfoType,
};
use conduit_form::{KindDefinition, KindSignature};

use crate::{
    education_assessment_type, education_lesson_feedback_type, education_progress_type,
    education_question_type, education_registered_types, education_response_type,
    education_rhythm_feedback_type, timing_feedback_type,
};

pub const EDUCATION_ARITHMETIC_FIXTURE_KIND: &str = "education/arithmetic-fixture";
pub const EDUCATION_EVALUATE_ARITHMETIC_KIND: &str = "education/evaluate-arithmetic";
pub const EDUCATION_RHYTHM_FEEDBACK_KIND: &str = "education/rhythm-feedback";
pub const EDUCATION_REVISION: &str = "conduit.std/education-assessment@1";

pub type EducationKindContract = (KindId, Vec<PortDescriptor>, Vec<PortDescriptor>);

/// Exact portable education Kinds and typed faces, without any Host realization facts.
pub fn education_kind_contracts() -> Vec<EducationKindContract> {
    vec![
        (
            kind_id(EDUCATION_ARITHMETIC_FIXTURE_KIND),
            vec![],
            vec![
                value_port(
                    "question",
                    &education_question_type(),
                    PortDirection::Output,
                ),
                value_port(
                    "response",
                    &education_response_type(),
                    PortDirection::Output,
                ),
            ],
        ),
        (
            kind_id(EDUCATION_EVALUATE_ARITHMETIC_KIND),
            vec![
                value_port("question", &education_question_type(), PortDirection::Input),
                value_port("response", &education_response_type(), PortDirection::Input),
            ],
            vec![
                value_port(
                    "assessment",
                    &education_assessment_type(),
                    PortDirection::Output,
                ),
                value_port(
                    "feedback",
                    &education_lesson_feedback_type(),
                    PortDirection::Output,
                ),
                value_port(
                    "progress",
                    &education_progress_type(),
                    PortDirection::Output,
                ),
            ],
        ),
        (
            kind_id(EDUCATION_RHYTHM_FEEDBACK_KIND),
            vec![flow_port(
                "timing",
                &timing_feedback_type(),
                PortDirection::Input,
            )],
            vec![flow_port(
                "feedback",
                &education_rhythm_feedback_type(),
                PortDirection::Output,
            )],
        ),
    ]
}

pub fn install_education_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    for (name, value_type) in education_registered_types() {
        startup
            .insert_structured_type(name, value_type)
            .map_err(|error| error.to_string())?;
    }
    for (kind, inputs, outputs) in education_kind_contracts() {
        insert_kind(startup, profile, kind.as_str(), inputs, outputs)?;
    }
    Ok(())
}

fn insert_kind(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
    kind: &str,
    inputs: Vec<PortDescriptor>,
    outputs: Vec<PortDescriptor>,
) -> Result<(), String> {
    startup
        .insert(KindSignature {
            kind: kind.into(),
            startup_parameters: vec![],
        })
        .map_err(|error| error.to_string())?;
    profile
        .insert(KindDefinition {
            kind_id: kind_id(kind),
            kind_contract_revision: KindContractRevision::from(EDUCATION_REVISION),
            inputs,
            outputs,
            configuration: vec![],
        })
        .map_err(|error| error.to_string())
}

fn value_port(
    name: &str,
    value_type: &StructuredInfoType,
    direction: PortDirection,
) -> PortDescriptor {
    port(name, value_type, direction, PortTemporal::Value)
}

fn flow_port(
    name: &str,
    value_type: &StructuredInfoType,
    direction: PortDirection,
) -> PortDescriptor {
    port(
        name,
        value_type,
        direction,
        PortTemporal::Flow { closes: true },
    )
}

fn port(
    name: &str,
    value_type: &StructuredInfoType,
    direction: PortDirection,
    temporal: PortTemporal,
) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: value_type
            .profile()
            .expect("reviewed education profile")
            .value_kind()
            .clone(),
        direction,
        temporal,
    }
}
