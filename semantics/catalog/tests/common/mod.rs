// Each integration-test crate compiles this shared proof support independently,
// so helpers used by sibling proof binaries are intentionally unused locally.
#![allow(dead_code)]

use conduit_core::{
    kind_id, port_id, resource_requirement, ArtifactId, AuthorityContractId, AuthorityRequirement,
    CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId, FaceStartupParameter,
    HostOperationContractId, HostOperationRequirement, ImplementationId, ImplementationOffer,
    KindContractRevision, PortDescriptor, PortDirection, PortTemporal, StructuredInfoType,
};

pub const JOB_PROOF_RUN_OPERATION: &str = "proof.host/process-job-run@1";
pub const JOB_PROOF_RESOURCE_CLASS: &str = "proof.resource/executable@1";
pub const REMINDER_PROOF_DELIVER_OPERATION: &str = "proof.host/reminder-delivery@1";
pub const DOMAIN_PROOF_OPERATION: &str = "proof.host/deterministic-domain@1";
pub const MOTION_PROOF_OPERATION: &str = "proof.host/robotics-motion@1";
pub const MOTION_PROOF_AUTHORITY: &str = "proof.authority/robotics-motion@1";

pub fn education_proof_offers() -> Vec<CapabilityOffer> {
    proof_domain_offers(
        conduit_semantic_catalog::education_kind_contracts(),
        conduit_semantic_catalog::EDUCATION_REVISION,
    )
}

pub fn vision_proof_offers() -> Vec<CapabilityOffer> {
    proof_domain_offers(
        conduit_semantic_catalog::vision_kind_contracts(),
        conduit_semantic_catalog::VISION_REVISION,
    )
}

pub fn robotics_structured_proof_offers() -> Vec<CapabilityOffer> {
    proof_domain_offers(
        conduit_semantic_catalog::robotics_structured_kind_contracts()
            .into_iter()
            .filter(|(kind, _, _)| {
                kind.as_str() != conduit_semantic_catalog::ROBOTICS_EXECUTE_MOTION_KIND
            })
            .collect(),
        conduit_semantic_catalog::ROBOTICS_STRUCTURED_REVISION,
    )
}

pub fn robotics_motion_proof_offer() -> CapabilityOffer {
    let (kind, inputs, outputs) = conduit_semantic_catalog::robotics_structured_kind_contracts()
        .into_iter()
        .find(|(kind, _, _)| {
            kind.as_str() == conduit_semantic_catalog::ROBOTICS_EXECUTE_MOTION_KIND
        })
        .expect("portable motion contract");
    let mut offer = proof_domain_offer(
        kind.clone(),
        inputs,
        outputs,
        conduit_semantic_catalog::ROBOTICS_STRUCTURED_REVISION,
        MOTION_PROOF_OPERATION,
    );
    offer.authority_requirements.push(AuthorityRequirement {
        contract_id: AuthorityContractId::from(MOTION_PROOF_AUTHORITY),
        host_operation_contract_id: HostOperationContractId::from(MOTION_PROOF_OPERATION),
        subject_kind: kind,
    });
    offer.limits.max_active_instances = 1;
    offer.limits.max_queue_items = 1;
    offer.limits.max_queue_bytes = conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32;
    offer
}

fn proof_domain_offers(
    contracts: Vec<(
        conduit_core::KindId,
        Vec<PortDescriptor>,
        Vec<PortDescriptor>,
    )>,
    revision: &str,
) -> Vec<CapabilityOffer> {
    contracts
        .into_iter()
        .map(|(kind, inputs, outputs)| {
            proof_domain_offer(kind, inputs, outputs, revision, DOMAIN_PROOF_OPERATION)
        })
        .collect()
}

fn proof_domain_offer(
    kind: conduit_core::KindId,
    inputs: Vec<PortDescriptor>,
    outputs: Vec<PortDescriptor>,
    revision: &str,
    operation: &str,
) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: vec![],
        shorthand: None,
        capability_id: CapabilityId::from(format!("proof/{}@1", kind.as_str())),
        kind_id: kind.clone(),
        kind_contract_revision: KindContractRevision::from(revision),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from("proof/deterministic-domain@1"),
            implementation_id: ImplementationId::from(format!("proof/{}@1", kind.as_str())),
            artifact_id: ArtifactId::from("proof/deterministic-domain@1"),
        },
        inputs,
        outputs,
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(operation),
            target_kind: Some(kind),
            maximum_in_flight: 1,
            maximum_input_bytes: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            maximum_output_bytes: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        }],
        resource_requirements: vec![],
        authority_requirements: vec![],
        limits: CapabilityLimits {
            max_active_instances: 4,
            max_queue_items: 4,
            max_queue_bytes: (conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES * 4) as u32,
        },
    }
}

pub fn recurrence_proof_offer() -> CapabilityOffer {
    let result = conduit_semantic_catalog::recurrence_result_type();
    CapabilityOffer {
        startup_parameters: vec![FaceStartupParameter {
            name: "request".into(),
            value_type: conduit_semantic_catalog::RECURRENCE_REQUEST_TYPE.into(),
            has_default: false,
        }],
        shorthand: None,
        capability_id: CapabilityId::from("proof/time-expand-recurrence"),
        kind_id: kind_id(conduit_semantic_catalog::RECURRENCE_KIND),
        kind_contract_revision: KindContractRevision::from(
            conduit_semantic_catalog::RECURRENCE_REVISION,
        ),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from("proof/recurrence-kernel@1"),
            implementation_id: ImplementationId::from("proof/kernel-expand-recurrence@1"),
            artifact_id: ArtifactId::from("proof/expand-recurrence@1"),
        },
        inputs: vec![],
        outputs: vec![PortDescriptor {
            port_id: port_id("occurrences"),
            value_kind: result.profile().unwrap().value_kind().clone(),
            direction: PortDirection::Output,
            temporal: PortTemporal::Value,
        }],
        host_operations: vec![],
        resource_requirements: vec![],
        authority_requirements: vec![],
        limits: CapabilityLimits {
            max_active_instances: 4,
            max_queue_items: conduit_semantic_catalog::RECURRENCE_MAXIMUM_RESULTS,
            max_queue_bytes: (conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
                * usize::from(conduit_semantic_catalog::RECURRENCE_MAXIMUM_RESULTS))
                as u32,
        },
    }
}

pub fn job_proof_offers() -> Vec<CapabilityOffer> {
    vec![
        workflow_proof_offer(
            conduit_semantic_catalog::JOB_FIXTURE_KIND,
            conduit_semantic_catalog::JOB_REVISION,
            "proof.host/process-job-fixture@1",
            vec![],
            vec![typed_port(
                "request",
                &conduit_semantic_catalog::job_request_type(),
                PortDirection::Output,
            )],
            None,
            None,
        ),
        workflow_proof_offer(
            conduit_semantic_catalog::JOB_RUN_KIND,
            conduit_semantic_catalog::JOB_REVISION,
            JOB_PROOF_RUN_OPERATION,
            vec![typed_port(
                "request",
                &conduit_semantic_catalog::job_request_type(),
                PortDirection::Input,
            )],
            vec![typed_port(
                "lifecycle",
                &conduit_semantic_catalog::job_lifecycle_type(),
                PortDirection::Output,
            )],
            Some(JOB_PROOF_RESOURCE_CLASS),
            Some(conduit_semantic_catalog::JOB_EXECUTABLE_AUTHORITY),
        ),
    ]
}

pub fn reminder_proof_offers() -> Vec<CapabilityOffer> {
    let reminder = conduit_semantic_catalog::reminder_occurrence_type();
    vec![
        workflow_proof_offer(
            conduit_semantic_catalog::REMINDER_FIXTURE_KIND,
            conduit_semantic_catalog::REMINDER_REVISION,
            "proof.host/reminder-fixture@1",
            vec![],
            vec![typed_port("reminder", &reminder, PortDirection::Output)],
            None,
            None,
        ),
        workflow_proof_offer(
            conduit_semantic_catalog::REMINDER_DELIVER_KIND,
            conduit_semantic_catalog::REMINDER_REVISION,
            REMINDER_PROOF_DELIVER_OPERATION,
            vec![typed_port("reminder", &reminder, PortDirection::Input)],
            vec![],
            None,
            Some(conduit_semantic_catalog::REMINDER_DELIVERY_AUTHORITY),
        ),
    ]
}

#[allow(clippy::too_many_arguments)]
fn workflow_proof_offer(
    kind: &str,
    revision: &str,
    operation: &str,
    inputs: Vec<PortDescriptor>,
    outputs: Vec<PortDescriptor>,
    resource_class: Option<&str>,
    authority_contract: Option<&str>,
) -> CapabilityOffer {
    let operation = HostOperationRequirement {
        contract_id: HostOperationContractId::from(operation),
        target_kind: Some(kind_id(kind)),
        maximum_in_flight: 1,
        maximum_input_bytes: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        maximum_output_bytes: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
    };
    CapabilityOffer {
        startup_parameters: vec![],
        shorthand: None,
        capability_id: CapabilityId::from(format!("proof/{kind}@1")),
        kind_id: kind_id(kind),
        kind_contract_revision: KindContractRevision::from(revision),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from("proof/workflow@1"),
            implementation_id: ImplementationId::from(format!("proof/{kind}@1")),
            artifact_id: ArtifactId::from("proof/workflow@1"),
        },
        inputs,
        outputs,
        host_operations: vec![operation.clone()],
        resource_requirements: resource_class
            .map(|class| resource_requirement(class, 1))
            .into_iter()
            .collect(),
        authority_requirements: authority_contract
            .map(|contract| AuthorityRequirement {
                contract_id: AuthorityContractId::from(contract),
                host_operation_contract_id: operation.contract_id,
                subject_kind: kind_id(kind),
            })
            .into_iter()
            .collect(),
        limits: CapabilityLimits {
            max_active_instances: 4,
            max_queue_items: 4,
            max_queue_bytes: (conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES * 4) as u32,
        },
    }
}

fn typed_port(
    name: &str,
    value_type: &StructuredInfoType,
    direction: PortDirection,
) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: value_type.profile().unwrap().value_kind().clone(),
        direction,
        temporal: PortTemporal::Value,
    }
}
