use super::*;
use conduit_core::{
    kind_id, ResourceClassId, ResourceExtent, ResourceLifetime, ResourceSemanticIdentity,
    ResourceVersionIdentity,
};

fn store(items: usize, bytes: usize) -> HostedHistoryContent {
    HostedHistoryContent::new(
        kind_id("value/scalar@1"),
        "controller/events",
        TemporalScale::Milliseconds,
        items,
        bytes,
        7,
    )
    .unwrap()
}

fn metadata(seed: u8, bytes: usize) -> (String, TemporalInstant, BoundedResourceRef) {
    (
        format!("sample/{seed}"),
        TemporalInstant {
            ticks: u64::from(seed),
            scale: TemporalScale::Milliseconds,
            clock_basis: "controller/events".into(),
            resolution_ticks: 1,
            uncertainty_ticks: 0,
        },
        BoundedResourceRef {
            identity: ResourceSemanticIdentity::from_digest([seed; 32]),
            content_profile: kind_id("value/scalar@1"),
            access_class: ResourceClassId::from("conduit.resource/history-value@1"),
            extent: ResourceExtent {
                bytes: bytes as u64,
                items: Some(1),
            },
            lifetime: ResourceLifetime {
                version: ResourceVersionIdentity::from_digest([seed; 32]),
                expires_at: None,
            },
        },
    )
}

fn append(
    store: &mut HostedHistoryContent,
    metadata: (String, TemporalInstant, BoundedResourceRef),
    bytes: &[u8],
) -> Result<u64, HistoryContentRefusal> {
    store.append(
        metadata.0,
        metadata.1,
        HistoricalEntryOrigin::MachineObservation,
        metadata.2,
        bytes,
    )
}

#[test]
fn retains_exact_ordered_samples_without_hot_path_allocations() {
    let mut store = store(3, 24);
    let samples = [
        250_000_i64.to_le_bytes(),
        500_000_i64.to_le_bytes(),
        750_000_i64.to_le_bytes(),
    ];
    let entries = [metadata(1, 8), metadata(2, 8), metadata(3, 8)];
    let allocation = crate::allocation_probe::begin();
    for (index, (entry, sample)) in entries.into_iter().zip(samples).enumerate() {
        assert_eq!(append(&mut store, entry, &sample), Ok(7 + index as u64));
    }
    for (index, expected) in samples.iter().enumerate() {
        let (entry, bytes) = store.entry(index).unwrap();
        assert_eq!(entry.sequence, 7 + index as u64);
        assert_eq!(bytes, expected);
    }
    assert_eq!(allocation.finish(), 0);
    assert_eq!(store.timeline().referenced_bytes(), 24);
    assert!(store.entry(3).is_none());
    assert!(store.timeline().retention_gap().is_none());
}

#[test]
fn item_and_byte_pressure_preserve_prior_content_and_sequence() {
    let mut items = store(1, 16);
    append(&mut items, metadata(1, 8), &[1; 8]).unwrap();
    assert_eq!(
        append(&mut items, metadata(2, 8), &[2; 8]),
        Err(HistoryContentRefusal::History(
            HistoricalTimelineRefusal::Full
        ))
    );
    assert_eq!(items.entry(0).unwrap().1, &[1; 8]);
    assert_eq!(items.timeline().len(), 1);

    let mut bytes = store(3, 12);
    append(&mut bytes, metadata(1, 8), &[1; 8]).unwrap();
    assert_eq!(
        append(&mut bytes, metadata(2, 8), &[2; 8]),
        Err(HistoryContentRefusal::History(
            HistoricalTimelineRefusal::ByteCapacityExceeded
        ))
    );
    assert_eq!(
        append(&mut bytes, metadata(3, 13), &[3; 13]),
        Err(HistoryContentRefusal::History(
            HistoricalTimelineRefusal::EntryExceedsByteLimit
        ))
    );
    assert_eq!(append(&mut bytes, metadata(4, 4), &[4; 4]), Ok(8));
    assert_eq!(bytes.entry(0).unwrap().1, &[1; 8]);
    assert_eq!(bytes.entry(1).unwrap().1, &[4; 4]);
}

#[test]
fn profile_clock_extent_and_version_failures_are_distinct_and_atomic() {
    let mut store = store(8, 64);
    append(&mut store, metadata(2, 8), &[2; 8]).unwrap();
    let mut wrong_profile = metadata(3, 8);
    wrong_profile.2.content_profile = kind_id("value/text@1");
    let mut wrong_clock = metadata(3, 8);
    wrong_clock.1.clock_basis = "another/events".into();
    for (entry, expected) in [
        (
            wrong_profile,
            HistoryContentRefusal::History(HistoricalTimelineRefusal::WrongValueProfile),
        ),
        (
            wrong_clock,
            HistoryContentRefusal::History(HistoricalTimelineRefusal::IncomparableEventTime),
        ),
        (
            metadata(1, 8),
            HistoryContentRefusal::History(HistoricalTimelineRefusal::ReorderedEventTime),
        ),
        (metadata(3, 7), HistoryContentRefusal::ExtentMismatch),
        (
            metadata(2, 8),
            HistoryContentRefusal::DuplicateResourceVersion,
        ),
    ] {
        assert_eq!(append(&mut store, entry, &[9; 8]), Err(expected));
        assert_eq!(store.timeline().len(), 1);
        assert_eq!(store.timeline().referenced_bytes(), 8);
        assert_eq!(store.entry(0).unwrap().1, &[2; 8]);
    }
    assert_eq!(append(&mut store, metadata(4, 8), &[4; 8]), Ok(8));
}

#[test]
fn clear_reclaims_capacity_without_restarting_sequence_identity() {
    let mut store = store(1, 8);
    append(&mut store, metadata(1, 8), &[1; 8]).unwrap();
    store.clear().unwrap();
    assert!(store.entry(0).is_none());
    assert_eq!(store.timeline().clear_revision(), 1);
    assert_eq!(store.timeline().referenced_bytes(), 0);
    assert_eq!(append(&mut store, metadata(2, 8), &[2; 8]), Ok(8));
}

#[test]
fn exhausted_sequence_cannot_publish_or_retain_content() {
    let mut store = HostedHistoryContent::new(
        kind_id("value/scalar@1"),
        "controller/events",
        TemporalScale::Milliseconds,
        1,
        8,
        u64::MAX,
    )
    .unwrap();
    assert_eq!(
        append(&mut store, metadata(1, 8), &[1; 8]),
        Err(HistoryContentRefusal::History(
            HistoricalTimelineRefusal::SequenceExhausted
        ))
    );
    assert!(store.timeline().is_empty());
    assert_eq!(store.storage.used_items(), 0);
    assert_eq!(store.storage.used_bytes(), 0);
}

#[test]
fn invalid_resource_and_time_scale_do_not_publish_content() {
    let mut store = store(1, 8);
    let mut invalid_resource = metadata(1, 8);
    invalid_resource.2.identity = ResourceSemanticIdentity::from_digest([0; 32]);
    let mut wrong_scale = metadata(1, 8);
    wrong_scale.1.scale = TemporalScale::Seconds;
    for (entry, refusal) in [
        (invalid_resource, HistoricalTimelineRefusal::InvalidResource),
        (
            wrong_scale,
            HistoricalTimelineRefusal::IncomparableEventTime,
        ),
    ] {
        assert_eq!(
            append(&mut store, entry, &[1; 8]),
            Err(HistoryContentRefusal::History(refusal))
        );
        assert_eq!(store.storage.used_items(), 0);
        assert_eq!(store.storage.used_bytes(), 0);
    }
}
