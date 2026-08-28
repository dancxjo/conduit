use conduit_core::{
    kind_id, port_id, resource_requirement, ArtifactId, AuthorityContractId, AuthorityRequirement,
    CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId, HostOperationContractId,
    HostOperationRequirement, ImplementationId, ImplementationOffer, KindContractRevision,
    PortDescriptor, PortDirection, PortTemporal, StructuredInfoType,
    MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};

pub const JOB_PROFILE: &str = "std/process-job-hosted@1";
pub const JOB_ARTIFACT: &str = "conduit-std-host/process-job@1";
pub const JOB_FIXTURE_OPERATION: &str = "conduit.host/process-job-fixture@1";
pub const JOB_RUN_OPERATION: &str = "conduit.host/process-job-run@1";
pub const JOB_EXECUTABLE_RESOURCE_CLASS: &str = "conduit.resource/executable@1";

pub const REMINDER_PROFILE: &str = "std/reminder-delivery-hosted@1";
pub const REMINDER_ARTIFACT: &str = "conduit-std-host/reminder-delivery@1";
pub const REMINDER_FIXTURE_OPERATION: &str = "conduit.host/reminder-fixture@1";
pub const REMINDER_DELIVER_OPERATION: &str = "conduit.host/reminder-delivery@1";

pub fn job_std_offers() -> Vec<CapabilityOffer> {
    vec![
        workflow_offer(
            conduit_std_catalog::JOB_FIXTURE_KIND,
            conduit_std_catalog::JOB_REVISION,
            JOB_PROFILE,
            JOB_ARTIFACT,
            JOB_FIXTURE_OPERATION,
            vec![],
            vec![typed_port(
                "request",
                &conduit_std_catalog::job_request_type(),
                PortDirection::Output,
            )],
            None,
            None,
        ),
        workflow_offer(
            conduit_std_catalog::JOB_RUN_KIND,
            conduit_std_catalog::JOB_REVISION,
            JOB_PROFILE,
            JOB_ARTIFACT,
            JOB_RUN_OPERATION,
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
            Some(JOB_EXECUTABLE_RESOURCE_CLASS),
            Some(conduit_std_catalog::JOB_EXECUTABLE_AUTHORITY),
        ),
    ]
}

pub fn reminder_std_offers() -> Vec<CapabilityOffer> {
    let reminder = conduit_std_catalog::reminder_occurrence_type();
    vec![
        workflow_offer(
            conduit_std_catalog::REMINDER_FIXTURE_KIND,
            conduit_std_catalog::REMINDER_REVISION,
            REMINDER_PROFILE,
            REMINDER_ARTIFACT,
            REMINDER_FIXTURE_OPERATION,
            vec![],
            vec![typed_port("reminder", &reminder, PortDirection::Output)],
            None,
            None,
        ),
        workflow_offer(
            conduit_std_catalog::REMINDER_DELIVER_KIND,
            conduit_std_catalog::REMINDER_REVISION,
            REMINDER_PROFILE,
            REMINDER_ARTIFACT,
            REMINDER_DELIVER_OPERATION,
            vec![typed_port("reminder", &reminder, PortDirection::Input)],
            vec![],
            None,
            Some(conduit_std_catalog::REMINDER_DELIVERY_AUTHORITY),
        ),
    ]
}

#[allow(clippy::too_many_arguments)]
fn workflow_offer(
    kind: &str,
    revision: &str,
    profile: &str,
    artifact: &str,
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
        maximum_input_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        maximum_output_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
    };
    CapabilityOffer {
        startup_parameters: vec![],
        shorthand: None,
        capability_id: CapabilityId::from(format!("std/{kind}@1")),
        kind_id: kind_id(kind),
        kind_contract_revision: KindContractRevision::from(revision),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(profile),
            implementation_id: ImplementationId::from(format!("std/{kind}@1")),
            artifact_id: ArtifactId::from(artifact),
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
            max_queue_bytes: (MAXIMUM_STRUCTURED_CANONICAL_BYTES * 4) as u32,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effectful_offers_preserve_finite_operation_resource_and_authority_truth() {
        let job = job_std_offers();
        let reminder = reminder_std_offers();
        for offer in job.iter().chain(&reminder) {
            assert_eq!(offer.host_operations.len(), 1);
            assert_eq!(offer.host_operations[0].maximum_in_flight, 1);
            assert_eq!(offer.limits.max_active_instances, 4);
            assert_eq!(offer.limits.max_queue_items, 4);
        }
        assert_eq!(job[1].resource_requirements.len(), 1);
        assert_eq!(job[1].authority_requirements.len(), 1);
        assert_eq!(reminder[1].authority_requirements.len(), 1);
        assert!(job[0].authority_requirements.is_empty());
        assert!(reminder[0].authority_requirements.is_empty());
    }
}
