//! Exact hosted local-CV offers over an explicit finite image residence.

use conduit_core::{
    protected_resource_requirement, ArtifactId, AuthorityContractId, AuthorityRequirement, Back,
    BackOfferBuilder, CapabilityId, CapabilityOffer, ExecutionProfileId, HostCallContractId,
    HostCallRequirement, ImplementationId, Kind,
};

pub const LOCAL_VISION_PROFILE: &str = "conduit.std/local-vision-gray8@1";
pub const LOCAL_VISION_IMPLEMENTATION: &str = "std/continuous-local-vision@1";
pub const LOCAL_VISION_ARTIFACT: &str = "conduit-std-host/continuous-local-vision@1";
pub const LOCAL_VISION_RESOURCE_CLASS: &str = "conduit.resource/finite-image-residence@1";
pub const LOCAL_VISION_RESOURCE_ROLE: &str = "image-residence";
pub const LOCAL_VISION_READ_AUTHORITY: &str = "conduit.authority/read-image-resource@1";
pub const LOCAL_VISION_MOTION_OPERATION: &str = "conduit.host/local-vision-motion@1";
pub const LOCAL_VISION_OBJECTS_OPERATION: &str = "conduit.host/local-vision-objects@1";
pub const LOCAL_VISION_TRACK_PROFILE: &str = "conduit.std/local-vision-track@1";
pub const LOCAL_VISION_TRACK_IMPLEMENTATION: &str = "std/local-vision-track@1";
pub const LOCAL_VISION_TRACK_ARTIFACT: &str = "conduit-std-host/local-vision-track@1";
pub const LOCAL_VISION_TRACK_OPERATION: &str = "conduit.host/local-vision-track@1";

pub fn local_vision_offers() -> [CapabilityOffer; 3] {
    [
        local_vision_offer(
            "local-vision-motion",
            vision_contract(conduit_semantic_catalog::VISION_MOTION_KIND),
            LOCAL_VISION_MOTION_OPERATION,
        ),
        local_vision_offer(
            "local-vision-objects",
            vision_contract(conduit_semantic_catalog::VISION_OBJECTS_KIND),
            LOCAL_VISION_OBJECTS_OPERATION,
        ),
        local_track_offer(),
    ]
}

fn local_track_offer() -> CapabilityOffer {
    let contract = vision_contract(conduit_semantic_catalog::VISION_TRACK_KIND);
    let target_kind = contract.kind_id.clone();
    BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from("local-vision-track"),
            execution_profile_id: ExecutionProfileId::from(LOCAL_VISION_TRACK_PROFILE),
            implementation_id: ImplementationId::from(LOCAL_VISION_TRACK_IMPLEMENTATION),
            artifact_id: ArtifactId::from(LOCAL_VISION_TRACK_ARTIFACT),
            host_calls: vec![HostCallRequirement {
                contract_id: HostCallContractId::from(LOCAL_VISION_TRACK_OPERATION),
                target_kind: Some(target_kind),
                maximum_in_flight: 1,
                maximum_input_bytes: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
                maximum_output_bytes: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            }],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build()
}

fn local_vision_offer(capability: &str, contract: Kind, operation: &str) -> CapabilityOffer {
    let operation = HostCallContractId::from(operation);
    let target_kind = contract.kind_id.clone();
    BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from(capability),
            execution_profile_id: ExecutionProfileId::from(LOCAL_VISION_PROFILE),
            implementation_id: ImplementationId::from(LOCAL_VISION_IMPLEMENTATION),
            artifact_id: ArtifactId::from(LOCAL_VISION_ARTIFACT),
            host_calls: vec![HostCallRequirement {
                contract_id: operation.clone(),
                target_kind: Some(target_kind.clone()),
                maximum_in_flight: 1,
                maximum_input_bytes: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
                maximum_output_bytes: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            }],
            resource_requirements: vec![protected_resource_requirement(
                LOCAL_VISION_RESOURCE_ROLE,
                LOCAL_VISION_RESOURCE_CLASS,
                1,
            )],
            authority_requirements: vec![AuthorityRequirement {
                contract_id: AuthorityContractId::from(LOCAL_VISION_READ_AUTHORITY),
                host_call_contract_id: operation,
                subject_kind: target_kind,
            }],
        },
    )
    .build()
}

fn vision_contract(kind: &str) -> Kind {
    conduit_semantic_catalog::vision_semantic_contracts()
        .into_iter()
        .find(|contract| contract.kind_id.as_str() == kind)
        .expect("local Vision Kind is registered")
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{PortDirection, PortTemporal};

    #[test]
    fn local_cv_offers_preserve_portable_front_and_seal_read_authority() {
        let offers = local_vision_offers();
        assert_eq!(
            offers[0].kind_id.as_str(),
            conduit_semantic_catalog::VISION_MOTION_KIND
        );
        assert_eq!(
            offers[1].kind_id.as_str(),
            conduit_semantic_catalog::VISION_OBJECTS_KIND
        );
        assert_eq!(
            offers[2].kind_id.as_str(),
            conduit_semantic_catalog::VISION_TRACK_KIND
        );
        assert_eq!(
            offers[1].kind_contract_revision.as_str(),
            conduit_semantic_catalog::VISION_LOCAL_OBJECTS_REVISION
        );
        assert_ne!(
            offers[0].kind_contract_revision,
            offers[1].kind_contract_revision
        );
        assert_eq!(
            offers[1].outputs[0].value_kind,
            conduit_semantic_catalog::vision_objects_type()
                .profile()
                .unwrap()
                .value_kind()
                .clone()
        );
        for offer in offers.iter().take(2) {
            assert_eq!(offer.inputs.len(), 1);
            assert_eq!(offer.inputs[0].direction, PortDirection::Input);
            assert_eq!(offer.inputs[0].temporal, PortTemporal::Current);
            assert_eq!(offer.outputs.len(), 1);
            assert_eq!(offer.outputs[0].direction, PortDirection::Output);
            assert_eq!(offer.outputs[0].temporal, PortTemporal::Current);
            assert_eq!(offer.host_calls.len(), 1);
            assert_eq!(offer.resource_requirements.len(), 1);
            assert_eq!(
                offer.resource_requirements[0]
                    .protected_role
                    .as_ref()
                    .map(|role| role.as_str()),
                Some(LOCAL_VISION_RESOURCE_ROLE)
            );
            assert_eq!(offer.authority_requirements.len(), 1);
            assert_eq!(
                offer.authority_requirements[0].host_call_contract_id,
                offer.host_calls[0].contract_id
            );
        }
        assert!(offers[2].resource_requirements.is_empty());
        assert!(offers[2].authority_requirements.is_empty());
        assert_eq!(offers[2].host_calls.len(), 1);
    }
}
