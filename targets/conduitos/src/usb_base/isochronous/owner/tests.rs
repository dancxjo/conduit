//! Cooperative capability lifetime tests; native stop acknowledgement is external.
use super::*;
use alloc::{vec, vec::Vec};
use conduit_core::*;
#[path = "tests/fixture.rs"]
pub(crate) mod fixture;
use conduit_kernel::HostCallId;
fn case_type(ty: &StructuredInfoType, tag: &str) -> StructuredInfoType {
    let StructuredInfoTypeShape::Variant { cases, .. } = ty.shape() else {
        panic!("variant");
    };
    cases
        .iter()
        .find(|case| case.tag() == tag)
        .unwrap()
        .payload_type()
        .clone()
}
fn field_type(ty: &StructuredInfoType, name: &str) -> StructuredInfoType {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("record");
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .unwrap()
        .value_type()
        .clone()
}
fn batch_input(count: usize, length: usize) -> Vec<u8> {
    let contract = IsochronousOutContract::prepare().unwrap();
    let batch = case_type(contract.request_type(), "batch");
    let sequence = field_type(&batch, "packets");
    let StructuredInfoTypeShape::Sequence { element, .. } = sequence.shape() else {
        panic!("sequence");
    };
    let packets = (0..count)
        .map(|index| {
            StructuredInfoValue::record(
                element.clone(),
                vec![
                    StructuredFieldValue::new(
                        "frame",
                        StructuredInfoValue::leaf(
                            field_type(element, "frame"),
                            (1024 + index as u64).to_le_bytes().to_vec(),
                        )
                        .unwrap(),
                    )
                    .unwrap(),
                    StructuredFieldValue::new(
                        "wire",
                        StructuredInfoValue::leaf(field_type(element, "wire"), vec![0x5a; length])
                            .unwrap(),
                    )
                    .unwrap(),
                ],
            )
            .unwrap()
        })
        .collect();
    let payload = StructuredInfoValue::record(
        batch,
        vec![
            StructuredFieldValue::new(
                "packets",
                StructuredInfoValue::sequence(sequence, packets).unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    StructuredInfoValue::variant(contract.request_type().clone(), "batch", payload)
        .unwrap()
        .canonical_bytes()
        .unwrap()
}
fn completions(count: usize) -> [IsochronousCompletion; 8] {
    let mut result = [IsochronousCompletion::default(); 8];
    for (index, item) in result.iter_mut().enumerate().take(count) {
        *item = IsochronousCompletion {
            frame: 1024 + index as u16,
            requested: 256,
            actual: 256,
        };
    }
    result
}
#[test]
fn full_eight_packet_canonical_envelope_is_validated_without_restricting_batch() {
    let contract = IsochronousOutContract::prepare().unwrap();
    let input = batch_input(8, 256);
    assert!(
        input.len() <= ISOCHRONOUS_OUT_MAXIMUM_BYTES as usize,
        "worst request {}",
        input.len()
    );
    let decoder = PreparedIsochronousRequestDecoder::new(&contract).unwrap();
    let DecodedIsochronousRequest::Batch(batch) = decoder.decode(&input, 256).unwrap() else {
        panic!("batch");
    };
    assert_eq!(batch.count(), 8);
    for index in 0..8 {
        assert_eq!(batch.packet(index).unwrap().1.len(), 256);
    }
    let mut encoder = PreparedIsochronousResultEncoder::new(&contract).unwrap();
    println!("ISO prepared result storage={} decoder inline={} owner inline={}", encoder.prepared_storage_bytes(), core::mem::size_of::<super::super::request::PreparedIsochronousRequestDecoder>(), core::mem::size_of::<IsochronousOutCallOwner>());
    let actual = completions(8);
    let result = encoder
        .encode(&actual, IsochronousOutcome::Completed)
        .unwrap();
    assert!(
        result.len() <= ISOCHRONOUS_OUT_MAXIMUM_BYTES as usize,
        "worst result {}",
        result.len()
    );
    let value = validate_canonical_structured_value(result).unwrap();
    let packets = value
        .variant_payload("completed")
        .unwrap()
        .unwrap()
        .record_field("packets")
        .unwrap()
        .unwrap();
    assert_eq!(packets.collection_length().unwrap(), 8);
    let first = packets.collection_index(0).unwrap().unwrap();
    assert_eq!(
        first
            .record_field("frame")
            .unwrap()
            .unwrap()
            .primitive_bytes("value/u64")
            .unwrap(),
        &1024_u64.to_le_bytes()
    );
    assert_eq!(
        first
            .record_field("actual")
            .unwrap()
            .unwrap()
            .primitive_bytes("value/u64")
            .unwrap(),
        &256_u64.to_le_bytes()
    );
    let input_bytes = input.len();
    let mut maximum_result = result.len();
    for outcome in [
        IsochronousOutcome::Short(IsochronousCompletion {
            frame: 1032,
            requested: 256,
            actual: 255,
        }),
        IsochronousOutcome::MissedService { frame: 1032 },
        IsochronousOutcome::Underrun,
        IsochronousOutcome::ProviderLost,
        IsochronousOutcome::Drained,
        IsochronousOutcome::Snapshot(
            crate::usb_base::isochronous_schedule::IsochronousOutSnapshot {
                current_microframe: 0x3fff,
                scheduling_threshold: 15,
                service_interval_microframes: 8,
                maximum_packet_bytes: 256,
                pending_packets: 8,
                maximum_pending_packets: 8,
                dma_resource_bytes: 3072,
            },
        ),
    ] {
        let encoded = encoder.encode(&actual, outcome).unwrap();
        maximum_result = maximum_result.max(encoded.len());
        validate_canonical_structured_value(encoded).unwrap();
    }
    assert!(maximum_result <= ISOCHRONOUS_OUT_MAXIMUM_BYTES as usize);
    std::println!(
        "ISO canonical envelopes: eight256 request={input_bytes}, maximum checked result={maximum_result}, admitted={}",
        ISOCHRONOUS_OUT_MAXIMUM_BYTES
    );
}
#[test]
fn batch_begin_retains_one_lease_until_all_exact_completions() {
    let mut owner = fixture::owner(7);
    let node = owner.node;
    let mut input = batch_input(8, 256);
    let submission = owner
        .begin(node, HostCallId(0), RequestId(0), &input)
        .unwrap();
    input.fill(0);
    let DecodedIsochronousRequest::Batch(batch) = submission.request() else {
        panic!("batch");
    };
    assert_eq!(batch.packet(0).unwrap().1, &[0x5a; 256]);
    assert_eq!(
        owner
            .table
            .inspections()
            .next()
            .unwrap()
            .in_flight_operations,
        1
    );
    assert!(owner.pending.is_some());
    assert_eq!(
        owner
            .begin(node, HostCallId(0), RequestId(1), &batch_input(1, 256))
            .err(),
        Some(IsochronousOwnerRefusal::Pressure)
    );
    let actual = completions(8);
    assert_eq!(
        unsafe { owner.finish_quiesced(&submission, &actual[..7], IsochronousOutcome::Completed) }
            .err(),
        Some(IsochronousOwnerRefusal::Observation)
    );
    assert!(owner.pending.is_some());
    unsafe { owner.finish_quiesced(&submission, &actual, IsochronousOutcome::Completed) }.unwrap();
    assert!(owner.pending.is_none());
    assert_eq!(
        owner
            .table
            .inspections()
            .next()
            .unwrap()
            .completed_operations,
        1
    );
    assert_eq!(
        owner
            .begin(node, HostCallId(0), RequestId(0), &batch_input(1, 256))
            .err(),
        Some(IsochronousOwnerRefusal::StaleTransfer)
    );
}
#[test]
fn cancelled_batch_retains_original_submission_until_acknowledged_discard() {
    let mut owner = fixture::owner(9);
    let node = owner.node;
    let submission = owner
        .begin(node, HostCallId(0), RequestId(0), &batch_input(8, 256))
        .unwrap();
    owner.revoke(node, HostCallId(0)).unwrap();
    assert!(owner.pending.is_some());
    assert_eq!(
        owner.table.inspections().next().unwrap().lifecycle,
        CapabilityLifecycle::Revoked
    );
    assert_eq!(
        owner
            .begin(node, HostCallId(0), RequestId(1), &batch_input(1, 256))
            .err(),
        Some(IsochronousOwnerRefusal::Capability(
            BaseCapabilityRefusal::Revoked
        ))
    );
    // Unit test supplies the unsafe acknowledgement obligation; no hardware proof.
    unsafe { owner.discard_quiesced(&submission) }.unwrap();
    assert!(owner.pending.is_none());
    assert_eq!(
        unsafe { owner.discard_quiesced(&submission) }.err(),
        Some(IsochronousOwnerRefusal::StaleTransfer)
    );
}
#[test]
fn missed_service_preserves_exact_completed_prefix_and_closes_attachment() {
    let mut owner = fixture::owner(3);
    let node = owner.node;
    let submission = owner
        .begin(node, HostCallId(0), RequestId(0), &batch_input(8, 256))
        .unwrap();
    let actual = completions(3);
    let result = unsafe {
        owner.finish_quiesced(
            &submission,
            &actual[..3],
            IsochronousOutcome::MissedService { frame: 1027 },
        )
    }
    .unwrap();
    let value = validate_canonical_structured_value(result).unwrap();
    assert_eq!(value.variant_tag().unwrap(), "missed-service");
    let payload = value.variant_payload("missed-service").unwrap().unwrap();
    assert_eq!(
        payload
            .record_field("completed")
            .unwrap()
            .unwrap()
            .collection_length()
            .unwrap(),
        3
    );
    assert_eq!(
        payload
            .record_field("frame")
            .unwrap()
            .unwrap()
            .primitive_bytes("value/u64")
            .unwrap(),
        &1027_u64.to_le_bytes()
    );
    assert!(owner.closed);
    assert_eq!(
        owner
            .begin(node, HostCallId(0), RequestId(1), &batch_input(1, 256))
            .err(),
        Some(IsochronousOwnerRefusal::Possession)
    );
}
#[test]
fn empty_batch_packet_pressure_and_wire_envelope_refuse_before_lease() {
    let mut owner = fixture::owner(4);
    let node = owner.node;
    assert_eq!(
        owner
            .begin(node, HostCallId(0), RequestId(0), &batch_input(0, 256))
            .err(),
        Some(IsochronousOwnerRefusal::Decode(
            IsochronousRequestRefusal::PacketCount
        ))
    );
    assert!(owner.pending.is_none());
    assert_eq!(
        owner
            .table
            .inspections()
            .next()
            .unwrap()
            .in_flight_operations,
        0
    );
    let input = vec![0; ISOCHRONOUS_OUT_MAXIMUM_BYTES as usize + 1];
    assert!(
        owner
            .begin(node, HostCallId(0), RequestId(0), &input)
            .is_err()
    );
    assert_eq!(
        owner
            .table
            .inspections()
            .next()
            .unwrap()
            .in_flight_operations,
        0
    );
}

#[test]
fn repeated_eight_packet_batches_and_metadata_composition_allocate_nothing() {
    let mut owner = fixture::owner(11);
    let node = owner.node;
    let input = batch_input(8, 256);
    let actual = completions(8);
    let allocations = crate::test_allocations::allocations(|| {
        for request in 0..1024 {
            let submission = owner
                .begin(node, HostCallId(0), RequestId(request), &input)
                .unwrap();
            let result = unsafe {
                owner.finish_quiesced(&submission, &actual, IsochronousOutcome::Completed)
            }
            .unwrap();
            assert_eq!(
                validate_canonical_structured_value(result)
                    .unwrap()
                    .variant_tag()
                    .unwrap(),
                "completed"
            );
        }
    });
    assert_eq!(allocations, 0);
    assert_eq!(
        owner
            .table
            .inspections()
            .next()
            .unwrap()
            .completed_operations,
        1024
    );
}

#[test]
fn native_short_tag_remains_distinct_even_when_extent_equals_requested() {
    let mut owner = fixture::owner(13);
    let node = owner.node;
    let submission = owner
        .begin(node, HostCallId(0), RequestId(0), &batch_input(1, 256))
        .unwrap();
    let result = unsafe {
        owner.finish_quiesced(
            &submission,
            &[],
            IsochronousOutcome::Short(IsochronousCompletion {
                frame: 1024,
                requested: 256,
                actual: 256,
            }),
        )
    }
    .unwrap();
    assert_eq!(
        validate_canonical_structured_value(result)
            .unwrap()
            .variant_tag()
            .unwrap(),
        "short"
    );
    assert!(owner.closed);
}
