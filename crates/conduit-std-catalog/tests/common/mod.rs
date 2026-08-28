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

pub fn recurrence_proof_offer() -> CapabilityOffer {
    let result = conduit_std_catalog::recurrence_result_type();
    CapabilityOffer {
        startup_parameters: vec![FaceStartupParameter {
            name: "request".into(),
            value_type: conduit_std_catalog::RECURRENCE_REQUEST_TYPE.into(),
            has_default: false,
        }],
        shorthand: None,
        capability_id: CapabilityId::from("proof/time-expand-recurrence"),
        kind_id: kind_id(conduit_std_catalog::RECURRENCE_KIND),
        kind_contract_revision: KindContractRevision::from(
            conduit_std_catalog::RECURRENCE_REVISION,
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
            max_queue_items: conduit_std_catalog::RECURRENCE_MAXIMUM_RESULTS,
            max_queue_bytes: (conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
                * usize::from(conduit_std_catalog::RECURRENCE_MAXIMUM_RESULTS))
                as u32,
        },
    }
}

pub fn job_proof_offers() -> Vec<CapabilityOffer> {
    vec![
        workflow_proof_offer(
            conduit_std_catalog::JOB_FIXTURE_KIND,
            conduit_std_catalog::JOB_REVISION,
            "proof.host/process-job-fixture@1",
            vec![],
            vec![typed_port(
                "request",
                &conduit_std_catalog::job_request_type(),
                PortDirection::Output,
            )],
            None,
            None,
        ),
        workflow_proof_offer(
            conduit_std_catalog::JOB_RUN_KIND,
            conduit_std_catalog::JOB_REVISION,
            JOB_PROOF_RUN_OPERATION,
            vec![typed_port(
                "request",
                &conduit_std_catalog::job_request_type(),
                PortDirection::Input,
            )],
            vec![typed_port(
                "lifecycle",
                &conduit_std_catalog::job_lifecycle_type(),
                PortDirection::Output,
            )],
            Some(JOB_PROOF_RESOURCE_CLASS),
            Some(conduit_std_catalog::JOB_EXECUTABLE_AUTHORITY),
        ),
    ]
}

pub fn reminder_proof_offers() -> Vec<CapabilityOffer> {
    let reminder = conduit_std_catalog::reminder_occurrence_type();
    vec![
        workflow_proof_offer(
            conduit_std_catalog::REMINDER_FIXTURE_KIND,
            conduit_std_catalog::REMINDER_REVISION,
            "proof.host/reminder-fixture@1",
            vec![],
            vec![typed_port("reminder", &reminder, PortDirection::Output)],
            None,
            None,
        ),
        workflow_proof_offer(
            conduit_std_catalog::REMINDER_DELIVER_KIND,
            conduit_std_catalog::REMINDER_REVISION,
            REMINDER_PROOF_DELIVER_OPERATION,
            vec![typed_port("reminder", &reminder, PortDirection::Input)],
            vec![],
            None,
            Some(conduit_std_catalog::REMINDER_DELIVERY_AUTHORITY),
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
