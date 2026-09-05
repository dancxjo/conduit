//! Cross-domain proof that portable image/text values reuse ordinary history and record delivery.

use conduit_core::{
    kind_id, BootId, BoundedResourceRef, HostId, Observation, ObservationKind, ResourceClassId,
    ResourceExtent, ResourceLifetime, ResourceSemanticIdentity, ResourceVersionIdentity, SignId,
    TemporalInstant, TemporalScale, ValuePayload,
};
use conduit_human::{
    compose_image_text, ImageObservationReference, ImageTextMetadata, ImageTextRecord,
};
use conduit_net::{
    deframe_typed_record_value, frame_typed_record_value_into, framed_typed_record_value,
    typed_record_value, value_from_typed_record, BoundedOrderedRecordQueue,
    BoundedRecordTranscript, RecordDeliveryStateRef, RecordDeliveryTracker,
    RecordTranscriptDirection, MAXIMUM_TYPED_RECORD_FRAME_BYTES,
};
use conduit_semantic_catalog::{
    image_text_record_from_value, image_text_record_type, image_text_record_value,
};
use conduit_time::{
    reload_historical_timeline, retain_historical_timeline, BoundedHistoricalTimeline,
    BoundedMemorySnapshotStore, HistoricalEntryOrigin, HistoricalOverflowPolicy,
    HistoricalTimelineRefusal, MAXIMUM_HISTORICAL_TIMELINE_SNAPSHOT_BYTES,
};

fn resource(profile: &str, identity: u8, bytes: u64) -> BoundedResourceRef {
    BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest([identity; 32]),
        content_profile: kind_id(profile),
        access_class: ResourceClassId::from("conduit.resource/portable-content@1"),
        extent: ResourceExtent {
            bytes,
            items: Some(1),
        },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest([identity + 1; 32]),
            expires_at: None,
        },
    }
}

fn composed() -> (conduit_core::KindId, ImageTextRecord) {
    let image_profile = kind_id("media/image-rgba8@1");
    let record = compose_image_text(
        &image_profile,
        ImageObservationReference {
            content: resource(image_profile.as_str(), 1, 4_096),
            width: 640,
            height: 480,
        },
        "Inspection point A".into(),
        vec![ImageTextMetadata {
            key: "operator".into(),
            value: "Ada".into(),
        }],
    )
    .unwrap();
    (image_profile, record)
}

#[test]
fn composed_value_uses_shared_framing_queue_receipt_and_transcript() {
    let (image_profile, original) = composed();
    let value = image_text_record_value(&original, &image_profile).unwrap();
    let typed = typed_record_value(&value).unwrap();
    let mut frame_buffer = [0; MAXIMUM_TYPED_RECORD_FRAME_BYTES];
    let frame_length = frame_typed_record_value_into(&typed, &mut frame_buffer).unwrap();
    let frame = &frame_buffer[..frame_length];

    let mut queue = BoundedOrderedRecordQueue::new(2, frame_length, 40).unwrap();
    let queue_sequence = queue.enqueue(frame).unwrap();
    let mut delivery =
        RecordDeliveryTracker::locally_accepted(b"polaroid-1", frame_length).unwrap();
    delivery.framed_queued(queue_sequence).unwrap();
    assert!(matches!(
        delivery.state(),
        RecordDeliveryStateRef::FramedQueued { queue_sequence: 40 }
    ));

    let received_frame = queue.dequeue().unwrap().frame.to_vec();
    let framed = framed_typed_record_value(&received_frame).unwrap();
    let received_typed = deframe_typed_record_value(&framed).unwrap();
    let received_value = value_from_typed_record(&received_typed).unwrap();
    let reconstructed = image_text_record_from_value(&received_value, &image_profile).unwrap();
    assert_eq!(reconstructed, original);

    delivery.remote_accepted(b"semantic-receipt-1").unwrap();
    assert!(matches!(
        delivery.state(),
        RecordDeliveryStateRef::RemoteAccepted {
            receipt: b"semantic-receipt-1"
        }
    ));
    let mut transcript =
        BoundedRecordTranscript::new(2, frame_length, frame_length * 2, 0).unwrap();
    transcript
        .record(RecordTranscriptDirection::Received, &received_frame)
        .unwrap();
    assert_eq!(transcript.len(), 1);
}

#[test]
fn composed_record_uses_shared_history_and_durable_snapshot_contract() {
    let (image_profile, record) = composed();
    let value = image_text_record_value(&record, &image_profile).unwrap();
    let bytes = value.canonical_bytes().unwrap();
    let record_profile = image_text_record_type()
        .profile()
        .unwrap()
        .value_kind()
        .clone();
    let retained = resource(record_profile.as_str(), 7, bytes.len() as u64);
    let mut history = BoundedHistoricalTimeline::new(
        record_profile,
        "operator-clock",
        TemporalScale::Milliseconds,
        2,
        bytes.len() as u64 * 2,
        HistoricalOverflowPolicy::Refuse,
        10,
    )
    .unwrap();
    history
        .append(
            "inspection-a".into(),
            TemporalInstant {
                ticks: 1_000,
                scale: TemporalScale::Milliseconds,
                clock_basis: "operator-clock".into(),
                resolution_ticks: 1,
                uncertainty_ticks: 0,
            },
            HistoricalEntryOrigin::OperatorAuthored,
            retained,
        )
        .unwrap();

    let mut storage = BoundedMemorySnapshotStore::new(
        1,
        MAXIMUM_HISTORICAL_TIMELINE_SNAPSHOT_BYTES,
        MAXIMUM_HISTORICAL_TIMELINE_SNAPSHOT_BYTES,
    )
    .unwrap();
    let mut scratch = [0; MAXIMUM_HISTORICAL_TIMELINE_SNAPSHOT_BYTES];
    retain_historical_timeline(&mut storage, "polaroid/history", &history, &mut scratch).unwrap();
    let reloaded = reload_historical_timeline(&storage, "polaroid/history").unwrap();
    assert_eq!(reloaded.len(), 1);
    assert_eq!(reloaded.entry(0).unwrap().identity, "inspection-a");

    let second = reloaded.entry(0).unwrap().value.clone();
    let mut full = reloaded;
    full.append(
        "inspection-b".into(),
        TemporalInstant {
            ticks: 2_000,
            scale: TemporalScale::Milliseconds,
            clock_basis: "operator-clock".into(),
            resolution_ticks: 1,
            uncertainty_ticks: 0,
        },
        HistoricalEntryOrigin::OperatorAuthored,
        second.clone(),
    )
    .unwrap();
    assert_eq!(
        full.append(
            "inspection-c".into(),
            TemporalInstant {
                ticks: 3_000,
                scale: TemporalScale::Milliseconds,
                clock_basis: "operator-clock".into(),
                resolution_ticks: 1,
                uncertainty_ticks: 0,
            },
            HistoricalEntryOrigin::OperatorAuthored,
            second,
        ),
        Err(HistoricalTimelineRefusal::Full)
    );
}

#[test]
fn transport_loss_is_not_misreported_as_storage_or_composition_failure() {
    let (_, record) = composed();
    let mut delivery = RecordDeliveryTracker::locally_accepted(b"polaroid-loss", 128).unwrap();
    delivery.transport_unavailable(17).unwrap();
    assert_eq!(
        delivery.state(),
        RecordDeliveryStateRef::TransportUnavailable { code: 17 }
    );
    assert!(record.caption.starts_with("Inspection"));
}

#[test]
fn structural_inspection_is_sign_bound_and_does_not_claim_image_manifestation() {
    let (profile, record) = composed();
    let value = image_text_record_value(&record, &profile).unwrap();
    let sign = Observation {
        sign_id: SignId::from("sign/polaroid-composed"),
        active_play_id: None,
        presentation_id: None,
        host_id: HostId::from("host/fixture"),
        boot_id: BootId::from("boot/fixture"),
        plan_id: None,
        placement_id: None,
        connection_id: None,
        kind: ObservationKind::ValuePresented {
            value: ValuePayload {
                value_kind: value.value_type().profile().unwrap().value_kind().clone(),
                encoded: value.canonical_bytes().unwrap(),
            },
        },
    };
    let inspection =
        conduit_presentation::StructuredSignPresentation::from_sign(1, &sign, value.value_type())
            .unwrap();
    assert_eq!(inspection.presentation.basis.sign_ids, vec![sign.sign_id]);
    assert!(inspection.presentation.text.is_empty());
    assert!(inspection.presentation.properties.iter().any(|property| {
        property.name == "leaf-content-redacted"
            && property.value == conduit_presentation::PresentationPropertyValue::Flag(true)
    }));
}
