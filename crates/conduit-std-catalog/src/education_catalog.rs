//! Canonical education Form catalog and finite deterministic hosted offers.

use alloc::{
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use conduit_core::{
    kind_id, port_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ExecutionProfileId, HostOperationContractId, HostOperationRequirement, ImplementationId,
    ImplementationOffer, KindContractRevision, PortDescriptor, PortDirection, PortTemporal,
    StructuredInfoType, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
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
pub const EDUCATION_PROFILE: &str = "std/education-assessment-hosted@1";
pub const EDUCATION_ARTIFACT: &str = "conduit-std-host/education-assessment@1";
pub const EDUCATION_HOST_OPERATION: &str = "conduit.host/education-deterministic@1";

pub fn install_education_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    for (name, value_type) in education_registered_types() {
        startup
            .insert_structured_type(name, value_type)
            .map_err(|error| error.to_string())?;
    }
    insert_kind(
        startup,
        profile,
        EDUCATION_ARITHMETIC_FIXTURE_KIND,
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
    )?;
    insert_kind(
        startup,
        profile,
        EDUCATION_EVALUATE_ARITHMETIC_KIND,
        vec![
            value_port(
                "question",
                &education_question_type(),
                PortDirection::Input,
            ),
            value_port(
                "response",
                &education_response_type(),
                PortDirection::Input,
            ),
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
    )?;
    insert_kind(
        startup,
        profile,
        EDUCATION_RHYTHM_FEEDBACK_KIND,
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
    )
}

pub fn education_std_offers() -> Vec<CapabilityOffer> {
    vec![
        offer(
            EDUCATION_ARITHMETIC_FIXTURE_KIND,
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
        offer(
            EDUCATION_EVALUATE_ARITHMETIC_KIND,
            vec![
                value_port(
                    "question",
                    &education_question_type(),
                    PortDirection::Input,
                ),
                value_port(
                    "response",
                    &education_response_type(),
                    PortDirection::Input,
                ),
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
        offer(
            EDUCATION_RHYTHM_FEEDBACK_KIND,
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

fn offer(kind: &str, inputs: Vec<PortDescriptor>, outputs: Vec<PortDescriptor>) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: vec![],
        shorthand: None,
        capability_id: CapabilityId::from(format!("std/{kind}@1")),
        kind_id: kind_id(kind),
        kind_contract_revision: KindContractRevision::from(EDUCATION_REVISION),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(EDUCATION_PROFILE),
            implementation_id: ImplementationId::from(format!("std/{kind}@1")),
            artifact_id: ArtifactId::from(EDUCATION_ARTIFACT),
        },
        inputs,
        outputs,
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(EDUCATION_HOST_OPERATION),
            target_kind: Some(kind_id(kind)),
            maximum_in_flight: 1,
            maximum_input_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            maximum_output_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        }],
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 4,
            max_queue_items: 4,
            max_queue_bytes: (MAXIMUM_STRUCTURED_CANONICAL_BYTES * 4) as u32,
        },
    }
}
