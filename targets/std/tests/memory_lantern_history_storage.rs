use conduit_core::{
    bind_active_play, kind_id, BootId, BoundedResourceRef, HostId, PlanId, ResourceClassId,
    ResourceExtent, ResourceLifetime, ResourceSemanticIdentity, ResourceVersionIdentity,
    TemporalInstant, TemporalScale,
};
use conduit_kernel::{FixedValueStore, HostedValueStore, StorageError, ValueStorage};
use conduit_time::{
    decode_historical_timeline, decode_replay_state, encode_historical_timeline_into,
    encode_replay_command_into, encode_replay_timeline_into, BoundedHistoricalTimeline,
    BoundedReplayOperation, HistoricalEntryOrigin, HistoricalOverflowPolicy,
    HistoricalTimelineCodecRefusal, ReplayCommand, ReplayPolicy, ReplayState,
    MAXIMUM_HISTORICAL_TIMELINE_SNAPSHOT_BYTES, MAXIMUM_REPLAY_COMMAND_BYTES,
    MAXIMUM_REPLAY_EVENT_BYTES, MAXIMUM_REPLAY_STATE_BYTES, MAXIMUM_REPLAY_TIMELINE_BYTES,
};

const SNAPSHOT_BYTES: u32 = MAXIMUM_HISTORICAL_TIMELINE_SNAPSHOT_BYTES as u32;

struct DeterministicDurableFixture {
    host_id: HostId,
    available: bool,
    bytes: Vec<u8>,
}

#[derive(Debug, PartialEq, Eq)]
enum DurableFixtureRefusal {
    Unavailable,
    WrongHost,
    Corrupt(HistoricalTimelineCodecRefusal),
}

impl DeterministicDurableFixture {
    fn new(host_id: HostId) -> Self {
        Self {
            host_id,
            available: true,
            bytes: Vec::with_capacity(MAXIMUM_HISTORICAL_TIMELINE_SNAPSHOT_BYTES),
        }
    }

    fn replace(&mut self, snapshot: &[u8]) {
        assert!(snapshot.len() <= self.bytes.capacity());
        self.bytes.clear();
        self.bytes.extend_from_slice(snapshot);
    }

    fn set_available(&mut self, available: bool) {
        self.available = available;
    }

    fn corrupt(&mut self) {
        self.bytes[0] ^= 1;
    }

    fn reload(
        &self,
        requesting_host: &HostId,
    ) -> Result<BoundedHistoricalTimeline, DurableFixtureRefusal> {
        if !self.available {
            return Err(DurableFixtureRefusal::Unavailable);
        }
        if requesting_host != &self.host_id {
            return Err(DurableFixtureRefusal::WrongHost);
        }
        decode_historical_timeline(&self.bytes).map_err(DurableFixtureRefusal::Corrupt)
    }
}

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

#[test]
fn unavailable_corrupt_quota_and_replay_failure_remain_distinct() {
    let host_id = HostId::from("memory-lantern/browser-host");
    let mut durable = DeterministicDurableFixture::new(host_id.clone());
    durable.replace(&encoded_history());
    durable.set_available(false);
    assert!(matches!(
        durable.reload(&host_id),
        Err(DurableFixtureRefusal::Unavailable)
    ));

    durable.set_available(true);
    durable.corrupt();
    assert!(matches!(
        durable.reload(&host_id),
        Err(DurableFixtureRefusal::Corrupt(
            HistoricalTimelineCodecRefusal::Integrity
        ))
    ));

    let encoded = encoded_history();
    let mut quota =
        HostedValueStore::new(1, SNAPSHOT_BYTES, u32::try_from(encoded.len() - 1).unwrap())
            .unwrap();
    assert_eq!(
        quota.store(&encoded),
        Err(StorageError::ByteCapacityExceeded)
    );

    let entries = history().replay_metadata();
    let mut timeline = vec![0; MAXIMUM_REPLAY_TIMELINE_BYTES];
    let timeline_length = encode_replay_timeline_into(&entries, &mut timeline).unwrap();
    let mut replay = BoundedReplayOperation::new(ReplayPolicy::Step).unwrap();
    replay.load_timeline(&timeline[..timeline_length]).unwrap();
    let mut command = [0; MAXIMUM_REPLAY_COMMAND_BYTES];
    let command_length =
        encode_replay_command_into(ReplayCommand::Fail { code: 37 }, &mut command).unwrap();
    let mut event = [0; MAXIMUM_REPLAY_EVENT_BYTES];
    let mut state = [0; MAXIMUM_REPLAY_STATE_BYTES];
    let failed = replay
        .apply_command(&command[..command_length], 0, &mut event, &mut state)
        .unwrap();
    assert_eq!(
        decode_replay_state(&state[..failed.state_bytes.unwrap()]),
        Ok(ReplayState::Failed { code: 37 })
    );
}

#[test]
fn durable_fixture_preserves_history_across_same_host_fresh_boot() {
    let plan_id = PlanId::from("memory-lantern/replay-plan");
    let first_boot = BootId::from("memory-lantern/browser-boot-1");
    let fresh_boot = BootId::from("memory-lantern/browser-boot-2");
    let mut durable = DeterministicDurableFixture::new(HostId::from("memory-lantern/browser-host"));
    let first_play = bind_active_play(&plan_id, &durable.host_id, &first_boot, 0);

    durable.replace(&encoded_history());
    let fresh_play = bind_active_play(&plan_id, &durable.host_id, &fresh_boot, 0);
    let restored = durable.reload(&fresh_play.host_id).unwrap();

    assert_eq!(first_play.host_id, fresh_play.host_id);
    assert_ne!(first_play.boot_id, fresh_play.boot_id);
    assert_ne!(first_play.active_play_id, fresh_play.active_play_id);
    assert_eq!(restored.entry(0), history().entry(0));
    assert_eq!(restored.entry(1), history().entry(1));
    assert_eq!(restored.replay_metadata(), history().replay_metadata());
    assert!(matches!(
        durable.reload(&HostId::from("memory-lantern/reset-host")),
        Err(DurableFixtureRefusal::WrongHost)
    ));

    let mut cleared = restored;
    cleared.clear().unwrap();
    let mut encoded = vec![0; MAXIMUM_HISTORICAL_TIMELINE_SNAPSHOT_BYTES];
    let length = encode_historical_timeline_into(&cleared, &mut encoded).unwrap();
    durable.replace(&encoded[..length]);
    let reloaded_clear = durable.reload(&fresh_play.host_id).unwrap();
    assert!(reloaded_clear.is_empty());
    assert_eq!(reloaded_clear.clear_revision(), 1);
    assert_eq!(durable.host_id, first_play.host_id);
}
