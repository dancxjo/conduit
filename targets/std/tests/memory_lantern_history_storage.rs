use conduit_core::{
    kind_id, BoundedResourceRef, ResourceClassId, ResourceExtent, ResourceLifetime,
    ResourceSemanticIdentity, ResourceVersionIdentity, TemporalInstant, TemporalScale,
};
use conduit_kernel::{FixedValueStore, HostedValueStore, StorageError, ValueStorage};
use conduit_time::{
    decode_historical_timeline, encode_historical_timeline_into, BoundedHistoricalTimeline,
    HistoricalEntryOrigin, HistoricalOverflowPolicy, MAXIMUM_HISTORICAL_TIMELINE_SNAPSHOT_BYTES,
};

const SNAPSHOT_BYTES: u32 = MAXIMUM_HISTORICAL_TIMELINE_SNAPSHOT_BYTES as u32;

fn event_time(ticks: u64) -> TemporalInstant {
    TemporalInstant {
        ticks,
        scale: TemporalScale::Milliseconds,
        clock_basis: "memory-lantern/event-clock".into(),
        resolution_ticks: 1,
        uncertainty_ticks: 0,
    }
}

fn event_value(seed: u8) -> BoundedResourceRef {
    BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest([seed; 32]),
        content_profile: kind_id("memory-lantern/light-choice@1"),
        access_class: ResourceClassId::from("conduit.resource/history-value@1"),
        extent: ResourceExtent {
            bytes: 4,
            items: Some(1),
        },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest([seed + 1; 32]),
            expires_at: None,
        },
    }
}

fn history() -> BoundedHistoricalTimeline {
    let mut history = BoundedHistoricalTimeline::new(
        kind_id("memory-lantern/light-choice@1"),
        "memory-lantern/event-clock",
        TemporalScale::Milliseconds,
        4,
        16,
        HistoricalOverflowPolicy::Refuse,
        0,
    )
    .unwrap();
    history
        .append(
            "memory-lantern/choice/amber".into(),
            event_time(1_000),
            HistoricalEntryOrigin::OperatorAuthored,
            event_value(1),
        )
        .unwrap();
    history
        .append(
            "memory-lantern/choice/blue".into(),
            event_time(1_250),
            HistoricalEntryOrigin::OperatorAuthored,
            event_value(3),
        )
        .unwrap();
    history
}

fn encoded_history() -> Vec<u8> {
    let mut encoded = vec![0; MAXIMUM_HISTORICAL_TIMELINE_SNAPSHOT_BYTES];
    let length = encode_historical_timeline_into(&history(), &mut encoded).unwrap();
    encoded.truncate(length);
    encoded
}

fn round_trip(storage: &mut impl ValueStorage) {
    let encoded = encoded_history();
    let stored = storage.store(&encoded).unwrap();
    assert_eq!(storage.used_items(), 1);
    assert_eq!(storage.used_bytes(), encoded.len() as u32);

    let restored = decode_historical_timeline(storage.get(stored).unwrap()).unwrap();
    let source = history();
    assert_eq!(restored.len(), source.len());
    assert_eq!(restored.entry(0), source.entry(0));
    assert_eq!(restored.entry(1), source.entry(1));
    assert_eq!(restored.replay_metadata(), source.replay_metadata());
}

#[test]
fn identical_semantic_history_round_trips_through_fixed_and_hosted_kernel_storage() {
    let mut fixed =
        FixedValueStore::<1, MAXIMUM_HISTORICAL_TIMELINE_SNAPSHOT_BYTES>::new(SNAPSHOT_BYTES)
            .unwrap();
    let mut hosted = HostedValueStore::new(1, SNAPSHOT_BYTES, SNAPSHOT_BYTES).unwrap();
    let hosted_capacities = hosted.allocation_capacities();

    round_trip(&mut fixed);
    round_trip(&mut hosted);

    assert_eq!(hosted.allocation_capacities(), hosted_capacities);
}

#[test]
fn storage_capacity_failure_remains_distinct_from_history_codec_failure() {
    let encoded = encoded_history();
    let insufficient_bytes = u32::try_from(encoded.len() - 1).unwrap();
    let mut storage = HostedValueStore::new(1, SNAPSHOT_BYTES, insufficient_bytes).unwrap();

    assert_eq!(
        storage.store(&encoded),
        Err(StorageError::ByteCapacityExceeded)
    );
    assert_eq!(storage.used_items(), 0);
    assert_eq!(storage.used_bytes(), 0);
}
