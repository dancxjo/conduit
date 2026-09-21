use conduit_core::{
    resource_requirement, ArtifactId, AuthorityContractId, AuthorityRequirement, Back,
    BackOfferBuilder, CapabilityId, CapabilityOffer, ExecutionProfileId, HostCallContractId,
    HostCallRequirement, ImplementationId, Kind, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
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
    conduit_semantic_catalog::job_semantic_contracts()
        .into_iter()
        .map(|contract| {
            let effectful = contract.kind_id.as_str() == conduit_semantic_catalog::JOB_RUN_KIND;
            workflow_offer(
                contract,
                JOB_PROFILE,
                JOB_ARTIFACT,
                if effectful {
                    JOB_RUN_OPERATION
                } else {
                    JOB_FIXTURE_OPERATION
                },
                effectful.then_some(JOB_EXECUTABLE_RESOURCE_CLASS),
                effectful.then_some(conduit_semantic_catalog::JOB_EXECUTABLE_AUTHORITY),
            )
        })
        .collect()
}

pub fn reminder_std_offers() -> Vec<CapabilityOffer> {
    conduit_semantic_catalog::reminder_semantic_contracts()
        .into_iter()
        .map(|contract| {
            let effectful =
                contract.kind_id.as_str() == conduit_semantic_catalog::REMINDER_DELIVER_KIND;
            workflow_offer(
                contract,
                REMINDER_PROFILE,
                REMINDER_ARTIFACT,
                if effectful {
                    REMINDER_DELIVER_OPERATION
                } else {
                    REMINDER_FIXTURE_OPERATION
                },
                None,
                effectful.then_some(conduit_semantic_catalog::REMINDER_DELIVERY_AUTHORITY),
            )
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn workflow_offer(
    contract: Kind,
    profile: &str,
    artifact: &str,
    operation: &str,
    resource_class: Option<&str>,
    authority_contract: Option<&str>,
) -> CapabilityOffer {
    let kind = contract.kind_id.clone();
    let operation = HostCallRequirement {
        contract_id: HostCallContractId::from(operation),
        target_kind: Some(kind.clone()),
        maximum_in_flight: 1,
        maximum_input_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        maximum_output_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
    };
    BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from(format!("std/{}@1", kind.as_str())),
            execution_profile_id: ExecutionProfileId::from(profile),
            implementation_id: ImplementationId::from(format!("std/{}@1", kind.as_str())),
            artifact_id: ArtifactId::from(artifact),
            host_calls: vec![operation.clone()],
            resource_requirements: resource_class
                .map(|class| resource_requirement(class, 1))
                .into_iter()
                .collect(),
            authority_requirements: authority_contract
                .map(|authority| AuthorityRequirement {
                    contract_id: AuthorityContractId::from(authority),
                    host_call_contract_id: operation.contract_id,
                    subject_kind: kind,
                })
                .into_iter()
                .collect(),
        },
    )
    .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_exact_semantics(offers: &[CapabilityOffer], contracts: &[Kind]) {
        for offer in offers {
            let contract = contracts
                .iter()
                .find(|contract| contract.kind_id == offer.kind_id)
                .expect("every workflow realization has a portable contract");
            assert_eq!(offer.startup_parameters, contract.startup_parameters);
            assert_eq!(offer.shorthand, contract.shorthand);
            assert_eq!(
                offer.kind_contract_revision,
                contract.kind_contract_revision
            );
            assert_eq!(offer.inputs, contract.inputs);
            assert_eq!(offer.outputs, contract.outputs);
            assert_eq!(offer.limits, contract.limits);
        }
    }

    #[test]
    fn effectful_offers_preserve_finite_operation_resource_and_authority_truth() {
        let job = job_std_offers();
        let reminder = reminder_std_offers();
        assert_exact_semantics(&job, &conduit_semantic_catalog::job_semantic_contracts());
        assert_exact_semantics(
            &reminder,
            &conduit_semantic_catalog::reminder_semantic_contracts(),
        );
        for offer in job.iter().chain(&reminder) {
            assert_eq!(offer.host_calls.len(), 1);
            assert_eq!(offer.host_calls[0].maximum_in_flight, 1);
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
