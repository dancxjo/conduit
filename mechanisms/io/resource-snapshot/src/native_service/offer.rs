//! Immutable Source/Back facts only. Publishing requires the actual recovered
//! Ready owner, original grants and resource funding; this creates none of them.
use super::*;
use alloc::vec;
use conduit_core::*;

pub fn capability_offer(
    artifact: &ArtifactId,
    implementation: &ImplementationId,
) -> Result<CapabilityOffer, ServiceRefusal> {
    if artifact.as_str().is_empty() || implementation.as_str().is_empty() {
        return Err(ServiceRefusal::WrongBinding);
    }
    let append_kind = kind_id(crate::append_contract::APPEND_KIND);
    let read_kind = kind_id(crate::read_contract::READ_KIND);
    Ok(BackOfferBuilder::new(
        crate::service_contract::definition(),
        Back {
            capability_id: SERVICE_CAPABILITY.into(),
            execution_profile_id: SERVICE_PROFILE.into(),
            implementation_id: implementation.clone(),
            artifact_id: artifact.clone(),
            host_calls: vec![
                HostCallRequirement {
                    contract_id: crate::native_append::APPEND_OPERATION.into(),
                    target_kind: Some(append_kind.clone()),
                    maximum_in_flight: 1,
                    maximum_input_bytes: MAXIMUM_RECORD_PAYLOAD_BYTES as u32,
                    maximum_output_bytes: crate::append_result::APPEND_RESULT_BYTES,
                },
                HostCallRequirement {
                    contract_id: READ_OPERATION.into(),
                    target_kind: Some(read_kind.clone()),
                    maximum_in_flight: 1,
                    maximum_input_bytes: crate::read_contract::READ_INPUT_BYTES,
                    maximum_output_bytes: crate::read_result::READ_RESULT_BYTES as u32,
                },
            ],
            resource_requirements: vec![resource_requirement(
                crate::native_append::APPEND_CLASS,
                1,
            )],
            authority_requirements: vec![
                AuthorityRequirement {
                    contract_id: crate::native_append::APPEND_AUTHORITY.into(),
                    host_call_contract_id: crate::native_append::APPEND_OPERATION.into(),
                    subject_kind: append_kind,
                },
                AuthorityRequirement {
                    contract_id: READ_AUTHORITY.into(),
                    host_call_contract_id: READ_OPERATION.into(),
                    subject_kind: read_kind,
                },
            ],
        },
    )
    .build())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn immutable_offer_has_two_exact_operations_and_one_affine_residence_requirement() {
        let offer =
            capability_offer(&"artifact/test".into(), &"implementation/test".into()).unwrap();
        assert_eq!(offer.kind_id, crate::service_contract::definition().kind_id);
        assert_eq!(
            offer.semantic_contract,
            crate::service_contract::definition().semantic_contract()
        );
        assert_eq!(offer.host_calls.len(), 2);
        assert_ne!(
            offer.host_calls[0].contract_id,
            offer.host_calls[1].contract_id
        );
        assert_ne!(
            offer.host_calls[0].target_kind,
            offer.host_calls[1].target_kind
        );
        assert_eq!(
            offer.host_calls[0].target_kind,
            Some(kind_id(crate::append_contract::APPEND_KIND))
        );
        assert_eq!(
            offer.host_calls[1].target_kind,
            Some(kind_id(crate::read_contract::READ_KIND))
        );
        assert_eq!(offer.host_calls[0].maximum_input_bytes, 200);
        assert_eq!(
            offer.host_calls[0].maximum_output_bytes,
            crate::append_result::APPEND_RESULT_BYTES
        );
        assert_eq!(offer.host_calls[1].maximum_input_bytes, 8);
        assert_eq!(
            offer.host_calls[1].maximum_output_bytes as usize,
            crate::read_result::READ_RESULT_BYTES
        );
        assert!(
            offer
                .host_calls
                .iter()
                .all(|call| call.maximum_in_flight == 1)
        );
        assert_eq!(offer.authority_requirements.len(), 2);
        for (call, authority) in offer.host_calls.iter().zip(&offer.authority_requirements) {
            assert_eq!(authority.host_call_contract_id, call.contract_id);
            assert_eq!(Some(authority.subject_kind.clone()), call.target_kind);
        }
        assert_ne!(
            offer.authority_requirements[0].contract_id,
            offer.authority_requirements[1].contract_id
        );
        assert_eq!(
            offer.resource_requirements,
            vec![resource_requirement(crate::native_append::APPEND_CLASS, 1)]
        );
    }
    #[test]
    fn empty_implementation_or_artifact_cannot_describe_a_realization() {
        for (artifact, implementation) in [("", "implementation/test"), ("artifact/test", "")] {
            assert!(matches!(
                capability_offer(&artifact.into(), &implementation.into()),
                Err(ServiceRefusal::WrongBinding)
            ));
        }
    }
}
