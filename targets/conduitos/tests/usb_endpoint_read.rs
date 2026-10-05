//! Bounded endpoint byte conformance; no controller or device execution claim.
#[path = "../../../architecture/plot/tests/prepared_structured_payload/allocation.rs"]
mod allocation;
use conduit_core::*;
use conduitos::usb_base::{
    endpoint_read_contract::EndpointReadContract,
    endpoint_read_request::{EndpointReadRequestRefusal, PreparedEndpointReadRequestDecoder},
    endpoint_read_result::{
        EndpointReadDisposition, EndpointReadResultRefusal, PreparedEndpointReadResultEncoder,
    },
};

fn request(contract: &EndpointReadContract, length: u64) -> Vec<u8> {
    StructuredInfoValue::record(
        contract.request_type().clone(),
        vec![
            StructuredFieldValue::new(
                "length",
                StructuredInfoValue::leaf(
                    StructuredInfoType::leaf(kind_id("value/u64")).unwrap(),
                    length.to_le_bytes().to_vec(),
                )
                .unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}

#[test]
fn every_admitted_length_decodes_without_allocating_and_oversize_never_wraps() {
    let contract = EndpointReadContract::prepare().unwrap();
    let decoder = PreparedEndpointReadRequestDecoder::new(&contract).unwrap();
    let inputs: Vec<_> = (1..=2048).map(|n| request(&contract, n)).collect();
    let allocations = allocation::allocations(|| {
        for (index, input) in inputs.iter().enumerate() {
            assert_eq!(decoder.decode(input, 2048).unwrap(), index as u16 + 1);
        }
    });
    assert_eq!(allocations, 0);
    for length in [0, 2049, 65536, u64::MAX] {
        assert_eq!(
            decoder.decode(&request(&contract, length), 2048),
            Err(EndpointReadRequestRefusal::Length)
        );
    }
    assert_eq!(
        decoder.decode(&request(&contract, 9), 8),
        Err(EndpointReadRequestRefusal::Length)
    );
    for maximum in [0, 2049, u16::MAX] {
        assert_eq!(
            decoder.decode(&inputs[0], maximum),
            Err(EndpointReadRequestRefusal::InvalidEnvelope)
        );
    }
    assert!(matches!(
        decoder.decode(&[0; 32], 2048),
        Err(EndpointReadRequestRefusal::Canonical(_))
    ));
    let wrong = StructuredInfoValue::leaf(
        StructuredInfoType::leaf(kind_id("value/u64")).unwrap(),
        8_u64.to_le_bytes().to_vec(),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    assert!(matches!(
        decoder.decode(&wrong, 2048),
        Err(EndpointReadRequestRefusal::Canonical(_))
    ));
}

#[test]
fn all_received_extents_preserve_exact_octets_shortness_and_reusable_storage() {
    let contract = EndpointReadContract::prepare().unwrap();
    let mut encoder = PreparedEndpointReadResultEncoder::new(&contract).unwrap();
    let wire = core::array::from_fn::<_, 2048, _>(|index| index as u8);
    let maximum_encoded = encoder.completed(2048, 2048, &wire).unwrap().len();
    println!("2048-byte endpoint result: {maximum_encoded} canonical bytes");
    let allocations = allocation::allocations(|| {
        for actual in 0..=2048_u16 {
            let bytes = encoder
                .completed(2048, actual, &wire[..usize::from(actual)])
                .unwrap();
            assert!(bytes.len() <= 4096);
            let result = validate_canonical_structured_value(bytes).unwrap();
            let frame = result.variant_payload("completed").unwrap().unwrap();
            let count = frame
                .record_field("actual")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/u64")
                .unwrap();
            assert_eq!(count, u64::from(actual).to_le_bytes());
            let short = frame
                .record_field("short")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/bool")
                .unwrap();
            assert_eq!(short, [u8::from(actual < 2048)]);
            let value = frame.record_field("wire").unwrap().unwrap();
            assert_eq!(
                value.primitive_bytes("value/bytes").unwrap(),
                &wire[..usize::from(actual)]
            );
        }
    });
    assert_eq!(allocations, 0);
    assert_eq!(
        encoder.completed(8, 9, &wire[..9]),
        Err(EndpointReadResultRefusal::ActualLength)
    );
    assert_eq!(
        encoder.completed(8, 3, &wire[..8]),
        Err(EndpointReadResultRefusal::InputLength)
    );
    assert_eq!(
        encoder.completed(0, 0, &[]),
        Err(EndpointReadResultRefusal::DataEnvelope)
    );
    assert_eq!(
        encoder.completed(2049, 0, &[]),
        Err(EndpointReadResultRefusal::DataEnvelope)
    );
}

#[test]
fn stalls_loss_unsupported_and_timeout_keep_distinct_tags_across_reuse() {
    let contract = EndpointReadContract::prepare().unwrap();
    let mut encoder = PreparedEndpointReadResultEncoder::new(&contract).unwrap();
    let dispositions = [
        (EndpointReadDisposition::Stalled, "stalled"),
        (EndpointReadDisposition::ProviderLost, "provider-lost"),
        (EndpointReadDisposition::Unsupported, "unsupported"),
        (EndpointReadDisposition::Timeout, "timeout"),
    ];
    let allocations = allocation::allocations(|| {
        for _ in 0..64 {
            for (disposition, tag) in dispositions {
                let result =
                    validate_canonical_structured_value(encoder.disposition(disposition).unwrap())
                        .unwrap();
                assert!(result.variant_payload(tag).unwrap().is_some());
            }
        }
    });
    assert_eq!(allocations, 0);
}
