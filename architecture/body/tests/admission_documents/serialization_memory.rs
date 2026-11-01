//! Codec preparation only: no worker membership, native workspace or Play proof.
use super::*;
use conduit_core as c;
#[path = "../common/admission_allocation_probe.rs"]
mod allocation;
#[global_allocator]
static ALLOCATOR: allocation::Allocator = allocation::Allocator;

fn capability(size: usize) -> c::CapabilityOffer {
    let ty = c::StructuredInfoType::leaf(c::kind_id(c::TEXT_INFO_ID)).unwrap();
    let profile = ty.profile().unwrap().value_kind().clone();
    let value = c::StructuredInfoValue::leaf(ty, vec![b'x'; size]).unwrap();
    let default =
        c::StructuredConfigurationValue::new(profile.clone(), value.canonical_bytes().unwrap())
            .unwrap();
    c::BackOfferBuilder::new(
        c::Kind {
            startup_parameters: vec![c::FrontStartupParameter {
                name: "default".into(),
                value_type: profile.clone(),
                has_default: true,
            }],
            shorthand: None,
            kind_id: "fixture/immutable-default".into(),
            kind_contract_revision: "fixture/revision".into(),
            inputs: vec![],
            outputs: vec![],
            configuration: vec![c::KindConfigurationField {
                key: "default".into(),
                default_value: c::ConfigurationValue::Structured(default),
                rule: c::KindConfigurationRule::Structured { profile },
            }],
            semantic_laws: vec![],
            limits: c::CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: 1,
            },
        },
        c::Back {
            capability_id: "fixture/back".into(),
            execution_profile_id: "fixture/profile".into(),
            implementation_id: "fixture/implementation".into(),
            artifact_id: "fixture/artifact".into(),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build()
}
fn request(size: usize) -> RoutedAdmissionRequest {
    let mut f = fixture();
    f.request
        .host_advertisement
        .capabilities
        .push(capability(size));
    RoutedAdmissionRequest {
        schema: COMPACT_ROUTED_ADMISSION_REQUEST_SCHEMA.into(),
        invitation_id: f.request.invitation_id.as_str().into(),
        request: f.request,
    }
}
#[test]
fn oversized_immutable_defaults_refuse_without_cloning_or_expanding_the_advertisement() {
    let request = request(129_539);
    let canonical = serde_json::to_vec(&request.request.host_advertisement).unwrap();
    assert!(canonical.len() > 65_536);
    let (_, legacy) = allocation::observe(|| {
        let cloned = request.request.host_advertisement.clone();
        let bytes = serde_json::to_vec(&cloned).unwrap();
        assert_eq!(bytes, canonical);
    });
    let c::ConfigurationValue::Structured(value) = &request.request.host_advertisement.capabilities
        [0]
    .semantic_contract
    .configuration[0]
        .default_value
    else {
        unreachable!()
    };
    let guard = allocation::refuse_next_allocation_size(value.canonical_value().len());
    let (result, current) = allocation::observe(|| serde_json::to_vec(&request));
    drop(guard);
    assert!(result.is_err());
    assert!(legacy.peak_bytes > 129_539);
    assert!(current.peak_bytes < 4096, "{current:?}");
    assert!(current.requested_bytes < 4096, "{current:?}");
    assert!(current.retained_bytes() < 512, "{current:?}");
    assert_eq!(
        serde_json::to_vec(&request.request.host_advertisement).unwrap(),
        canonical
    );
}
#[test]
fn receipt_encoding_borrows_the_same_complete_advertisement() {
    let mut f = fixture();
    f.request
        .host_advertisement
        .capabilities
        .push(capability(129_539));
    let mut receipt = admitted(&mut f);
    receipt.current_offers_available = true;
    let response = RoutedAdmissionResponse::Admitted {
        schema: COMPACT_ROUTED_ADMISSION_RESPONSE_SCHEMA.into(),
        receipt: Box::new(receipt),
    };
    let (result, current) = allocation::observe(|| serde_json::to_vec(&response));
    assert!(result.is_err());
    assert!(current.peak_bytes < 4096, "{current:?}");
    assert!(current.requested_bytes < 4096, "{current:?}");
    assert!(current.retained_bytes() < 512, "{current:?}");
}
#[test]
fn in_bound_full_descriptions_keep_the_original_profile_bytes_and_roundtrip() {
    let request = request(2048);
    let canonical = serde_json::to_vec(&request.request.host_advertisement).unwrap();
    let compressed = miniz_oxide::deflate::compress_to_vec_zlib(&canonical, 6);
    let expected: String = compressed.iter().map(|b| format!("{b:02x}")).collect();
    let encoded = serde_json::to_vec(&request).unwrap();
    let wire: Value = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(
        wire["request"]["host_advertisement"]["expanded_bytes"],
        canonical.len()
    );
    assert_eq!(wire["request"]["host_advertisement"]["zlib_hex"], expected);
    let decoded: RoutedAdmissionRequest = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(
        decoded.request.host_advertisement,
        request.request.host_advertisement
    );
    assert_eq!(decoded.request.signature, request.request.signature);
    assert_eq!(serde_json::to_vec(&decoded).unwrap(), encoded);
}

#[test]
fn aggregate_defaults_and_realization_payloads_share_the_same_existing_bound() {
    for realization in [false, true] {
        let mut request = request(40_000);
        let first = &mut request.request.host_advertisement.capabilities[0];
        if realization {
            let c::ConfigurationValue::Structured(value) =
                &first.semantic_contract.configuration[0].default_value
            else {
                unreachable!()
            };
            first.realization_properties.push(value.clone());
        } else {
            request
                .request
                .host_advertisement
                .capabilities
                .push(capability(40_000));
        }
        let (result, allocation) = allocation::observe(|| serde_json::to_vec(&request));
        assert!(result.is_err());
        assert!(allocation.peak_bytes < 4096, "{allocation:?}");
    }
}
