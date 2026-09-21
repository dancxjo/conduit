//! Exact protected-file copy realization offers owned by the hosted std Host.

use conduit_core::{
    kind_id, present_host_call_requirement, protected_resource_requirement, resource_requirement,
    ArtifactId, AuthorityContractId, AuthorityRequirement, Back, BackOfferBuilder, CapabilityId,
    CapabilityOffer, ExecutionProfileId, HostCallContractId, HostCallRequirement, ImplementationId,
    PRESENTATION_RESOURCE_CLASS,
};

pub const COPY_FILE_EXECUTION_PROFILE: &str = "conduit.std/file-copy-kernel-hosted@1";
pub const COPY_FILE_IMPLEMENTATION: &str = "std/kernel-file-copy@1";
pub const COPY_FILE_ARTIFACT: &str = "conduit-std-host/file-copy@1";
pub const ISOLATED_COPY_FILE_EXECUTION_PROFILE: &str =
    "conduit.std/file-copy-kernel-isolated-linux@1";
pub const ISOLATED_COPY_FILE_IMPLEMENTATION: &str = "std/isolated-file-copy@1";
pub const ISOLATED_COPY_FILE_ARTIFACT: &str = "conduit-base-files/linux-file-copy@1";
pub const COPY_FILE_CAPABILITY: &str = "file-copy-v1";
pub const COPY_FILE_HOST_CALL_CONTRACT: &str = "conduit.host/file-copy-step@1";
pub const ISOLATED_COPY_FILE_AUTHORITY_CONTRACT: &str = "conduit.authority/file-copy@1";
pub const COPY_COMMAND_BYTES: u32 = 1;
pub const COPY_RESULT_PRESENTATION_IMPLEMENTATION: &str =
    "std/kernel-file-copy-result-presentation@1";

pub fn copy_file_offer() -> CapabilityOffer {
    copy_file_offer_for(
        COPY_FILE_EXECUTION_PROFILE,
        COPY_FILE_IMPLEMENTATION,
        COPY_FILE_ARTIFACT,
        false,
    )
}

pub fn isolated_copy_file_offer() -> CapabilityOffer {
    copy_file_offer_for(
        ISOLATED_COPY_FILE_EXECUTION_PROFILE,
        ISOLATED_COPY_FILE_IMPLEMENTATION,
        ISOLATED_COPY_FILE_ARTIFACT,
        true,
    )
}

fn copy_file_offer_for(
    execution_profile: &str,
    implementation: &str,
    artifact: &str,
    isolated: bool,
) -> CapabilityOffer {
    let contract = conduit_semantic_catalog::copy_file_contract();
    conduit_semantic_catalog::realization_offer(
        contract,
        conduit_semantic_catalog::COPY_FILE_CONTRACT_REVISION,
        conduit_semantic_catalog::RealizationOfferIdentity {
            capability: COPY_FILE_CAPABILITY,
            execution_profile,
            implementation,
            artifact,
        },
        vec![HostCallRequirement {
            contract_id: HostCallContractId::from(COPY_FILE_HOST_CALL_CONTRACT),
            target_kind: Some(kind_id(conduit_semantic_catalog::COPY_FILE_KIND)),
            maximum_in_flight: 1,
            maximum_input_bytes: COPY_COMMAND_BYTES,
            maximum_output_bytes: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        }],
        vec![
            protected_resource_requirement(
                conduit_semantic_catalog::COPY_DESTINATION_ROLE,
                conduit_semantic_catalog::PROTECTED_FILE_RESOURCE_CLASS,
                1,
            ),
            protected_resource_requirement(
                conduit_semantic_catalog::COPY_SOURCE_ROLE,
                conduit_semantic_catalog::PROTECTED_FILE_RESOURCE_CLASS,
                1,
            ),
        ],
        if isolated {
            vec![AuthorityRequirement {
                contract_id: AuthorityContractId::from(ISOLATED_COPY_FILE_AUTHORITY_CONTRACT),
                host_call_contract_id: HostCallContractId::from(COPY_FILE_HOST_CALL_CONTRACT),
                subject_kind: kind_id(conduit_semantic_catalog::COPY_FILE_KIND),
            }]
        } else {
            Vec::new()
        },
    )
}

pub fn copy_result_presentation_offer() -> CapabilityOffer {
    let contract = conduit_semantic_catalog::structured_presentation_contract(
        conduit_semantic_catalog::COPY_RESULT_TYPE,
        &conduit_semantic_catalog::copy_result_type(),
    );
    BackOfferBuilder::new(
        contract.into(),
        Back {
            capability_id: CapabilityId::from("std-file-copy-result-presentation"),
            execution_profile_id: ExecutionProfileId::from(COPY_FILE_EXECUTION_PROFILE),
            implementation_id: ImplementationId::from(COPY_RESULT_PRESENTATION_IMPLEMENTATION),
            artifact_id: ArtifactId::from(COPY_FILE_ARTIFACT),
            host_calls: vec![present_host_call_requirement(
                kind_id(conduit_semantic_catalog::STRUCTURED_PRESENTATION_TARGET),
                conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            )],
            resource_requirements: vec![resource_requirement(PRESENTATION_RESOURCE_CLASS, 1)],
            authority_requirements: Vec::new(),
        },
    )
    .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_offer_requires_two_named_protected_files_and_one_bounded_step() {
        let offer = copy_file_offer();
        assert!(offer.inputs.is_empty());
        assert_eq!(offer.outputs.len(), 1);
        assert_eq!(offer.resource_requirements.len(), 2);
        assert_eq!(
            offer.resource_requirements[0]
                .protected_role
                .as_ref()
                .map(|role| role.as_str()),
            Some(conduit_semantic_catalog::COPY_DESTINATION_ROLE)
        );
        assert_eq!(
            offer.resource_requirements[1]
                .protected_role
                .as_ref()
                .map(|role| role.as_str()),
            Some(conduit_semantic_catalog::COPY_SOURCE_ROLE)
        );
        assert_eq!(offer.host_calls[0].maximum_input_bytes, 1);
        assert_eq!(
            offer.host_calls[0].maximum_output_bytes,
            conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32
        );
    }

    #[test]
    fn copy_result_presenter_preserves_the_generic_structured_contract() {
        let offer = copy_result_presentation_offer();
        let contract = conduit_semantic_catalog::structured_presentation_contract(
            conduit_semantic_catalog::COPY_RESULT_TYPE,
            &conduit_semantic_catalog::copy_result_type(),
        );
        assert_eq!(offer.startup_parameters, contract.startup_parameters);
        assert_eq!(offer.kind_id, contract.kind_id);
        assert_eq!(
            offer.kind_contract_revision,
            contract.kind_contract_revision
        );
        assert_eq!(offer.inputs, contract.inputs);
        assert_eq!(offer.outputs, contract.outputs);
        assert_eq!(offer.limits, contract.limits);
        assert_eq!(offer.host_calls.len(), 1);
        assert_eq!(offer.resource_requirements.len(), 1);
    }
}
