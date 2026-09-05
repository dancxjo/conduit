use conduit_kernel::{FixedValueStore, HostedValueStore, ValueStorage};
use conduit_time::{
    decode_replay_timeline, encode_replay_timeline_into, HistoricalReplayEntry,
    MAXIMUM_REPLAY_TIMELINE_BYTES,
};

const TIMELINE_BYTES: u32 = MAXIMUM_REPLAY_TIMELINE_BYTES as u32;

fn source() -> Vec<HistoricalReplayEntry> {
    vec![
        HistoricalReplayEntry {
            identity: "memory/choice/1".into(),
            event_ticks: 1_000,
        },
        HistoricalReplayEntry {
            identity: "memory/choice/2".into(),
            event_ticks: 1_250,
        },
    ]
}

fn encoded_source() -> Vec<u8> {
    let mut encoded = vec![0_u8; MAXIMUM_REPLAY_TIMELINE_BYTES];
    let length = encode_replay_timeline_into(&source(), &mut encoded).unwrap();
    encoded.truncate(length);
    encoded
}

fn round_trip(storage: &mut impl ValueStorage) {
    let encoded = encoded_source();
    let reference = storage.store(&encoded).unwrap();
    assert_eq!(storage.used_items(), 1);
    assert_eq!(storage.used_bytes(), encoded.len() as u32);
    let restored = decode_replay_timeline(storage.get(reference).unwrap()).unwrap();
    assert_eq!(restored, source());
}

#[test]
fn identical_timeline_round_trips_through_fixed_and_hosted_value_storage() {
    let mut fixed =
        FixedValueStore::<1, MAXIMUM_REPLAY_TIMELINE_BYTES>::new(TIMELINE_BYTES).unwrap();
    let mut hosted = HostedValueStore::new(1, TIMELINE_BYTES, TIMELINE_BYTES).unwrap();
    let hosted_capacities = hosted.allocation_capacities();

    round_trip(&mut fixed);
    round_trip(&mut hosted);

    assert_eq!(hosted.allocation_capacities(), hosted_capacities);
}

#[test]
fn storage_capacity_failure_is_not_reclassified_as_replay_failure() {
    let encoded = encoded_source();
    let maximum = u32::try_from(encoded.len() - 1).unwrap();
    let mut hosted = HostedValueStore::new(1, TIMELINE_BYTES, maximum).unwrap();
    assert_eq!(
        hosted.store(&encoded),
        Err(conduit_kernel::StorageError::ByteCapacityExceeded)
    );
    assert_eq!(hosted.used_items(), 0);
}
