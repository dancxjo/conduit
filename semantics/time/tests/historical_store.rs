use conduit_core::{
    kind_id, BoundedResourceRef, ResourceClassId, ResourceExtent, ResourceLifetime,
    ResourceSemanticIdentity, ResourceVersionIdentity, TemporalInstant, TemporalScale,
};
use conduit_time::*;

fn resource(identity: u8) -> BoundedResourceRef {
    BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest([identity; 32]),
        content_profile: kind_id("human/image-text-record@1"),
        access_class: ResourceClassId::from("fixture/snapshot-content@1"),
        extent: ResourceExtent {
            bytes: 1_024,
            items: Some(1),
        },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest([identity + 1; 32]),
            expires_at: None,
        },
    }
}

fn timeline() -> BoundedHistoricalTimeline {
    let mut timeline = BoundedHistoricalTimeline::new(
        kind_id("human/image-text-record@1"),
        "operator-clock",
        TemporalScale::Milliseconds,
        2,
        2_048,
        HistoricalOverflowPolicy::Refuse,
        10,
    )
    .unwrap();
    timeline
        .append(
            "observation-a".into(),
            TemporalInstant {
                ticks: 1_000,
                scale: TemporalScale::Milliseconds,
                clock_basis: "operator-clock".into(),
                resolution_ticks: 1,
                uncertainty_ticks: 0,
            },
            HistoricalEntryOrigin::OperatorAuthored,
            resource(1),
        )
        .unwrap();
    timeline
}

#[test]
fn deterministic_store_round_trips_the_complete_timeline_contract() {
    let mut store = BoundedMemorySnapshotStore::new(
        2,
        MAXIMUM_HISTORICAL_TIMELINE_SNAPSHOT_BYTES,
        MAXIMUM_HISTORICAL_TIMELINE_SNAPSHOT_BYTES * 2,
    )
    .unwrap();
    let mut scratch = [0; MAXIMUM_HISTORICAL_TIMELINE_SNAPSHOT_BYTES];
    let written =
        retain_historical_timeline(&mut store, "polaroid/history", &timeline(), &mut scratch)
            .unwrap();
    assert_eq!(store.retained_bytes(), written);
    let restored = reload_historical_timeline(&store, "polaroid/history").unwrap();
    assert_eq!(restored.len(), 1);
    assert_eq!(restored.entry(0).unwrap().identity, "observation-a");
    assert_eq!(store.allocated_capacities(0), Some((128, 48 * 1024)));
}

#[test]
fn unavailable_missing_quota_and_corruption_remain_distinct() {
    let mut store = BoundedMemorySnapshotStore::new(1, 128, 128).unwrap();
    assert_eq!(
        store.read_snapshot("missing"),
        Err(HistoricalStoreRefusal::Missing)
    );
    store.set_available(false);
    assert_eq!(
        store.write_snapshot("one", &[1]),
        Err(HistoricalStoreRefusal::Unavailable)
    );
    assert_eq!(store.retained_bytes(), 0);
    store.set_available(true);
    store.write_snapshot("one", &[1]).unwrap();
    assert_eq!(
        store.write_snapshot("two", &[2]),
        Err(HistoricalStoreRefusal::QuotaExhausted)
    );
    assert!(matches!(
        reload_historical_timeline(&store, "one"),
        Err(HistoricalStoreRefusal::CorruptSnapshot(
            HistoricalTimelineCodecRefusal::Truncated
        ))
    ));
}

#[test]
fn delete_is_explicit_and_does_not_change_store_availability() {
    let mut store = BoundedMemorySnapshotStore::new(1, 128, 128).unwrap();
    store.write_snapshot("one", &[1, 2, 3]).unwrap();
    store.delete_snapshot("one").unwrap();
    assert!(store.is_available());
    assert_eq!(store.retained_bytes(), 0);
    assert_eq!(
        store.read_snapshot("one"),
        Err(HistoricalStoreRefusal::Missing)
    );
}
