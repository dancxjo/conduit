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
    let mut offer = BackOfferBuilder::new(
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
    .build();
    // Core and Planner require canonical description order, independently of
    // the execution service's append/read operation-slot order.
    offer.host_calls.sort();
    offer.authority_requirements.sort();
    Ok(offer)
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
        assert!(offer.host_calls.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(offer
            .authority_requirements
            .windows(2)
            .all(|pair| pair[0] < pair[1]));
        let append = offer
            .host_calls
            .iter()
            .find(|call| call.contract_id.as_str() == crate::native_append::APPEND_OPERATION)
            .unwrap();
        let read = offer
            .host_calls
            .iter()
            .find(|call| call.contract_id.as_str() == READ_OPERATION)
            .unwrap();
        assert_eq!(
            append.target_kind,
            Some(kind_id(crate::append_contract::APPEND_KIND))
        );
        assert_eq!(
            read.target_kind,
            Some(kind_id(crate::read_contract::READ_KIND))
        );
        assert_eq!(append.maximum_input_bytes, 200);
        assert_eq!(
            append.maximum_output_bytes,
            crate::append_result::APPEND_RESULT_BYTES
        );
        assert_eq!(read.maximum_input_bytes, 8);
        assert_eq!(
            read.maximum_output_bytes as usize,
            crate::read_result::READ_RESULT_BYTES
        );
        assert!(offer
            .host_calls
            .iter()
            .all(|call| call.maximum_in_flight == 1));
        assert_eq!(offer.authority_requirements.len(), 2);
        for authority in &offer.authority_requirements {
            let call = offer
                .host_calls
                .iter()
                .find(|call| authority.host_call_contract_id == call.contract_id)
                .unwrap();
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
