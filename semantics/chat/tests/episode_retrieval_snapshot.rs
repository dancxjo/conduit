#![cfg(feature = "plot-catalog")]
extern crate alloc;
use conduit_chat::*;
use conduit_core::*;
use conduit_plot::rust_binding::NativeRustBinding;
#[path = "../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

mod common_episode_retrieval;
use common_episode_retrieval::*;
#[test]
fn capture_is_allocation_free_and_preserves_original_record_bytes() {
    let prepared = PreparedEpisodeRetrievalSnapshot::prepare();
    let fixture = Fixture::new();
    let (captured, allocation) = allocation_probe::observe(|| prepared.capture(fixture.input()));
    let captured = captured.unwrap();
    assert_eq!((allocation.allocations, allocation.reallocations), (0, 0));
    assert_eq!(captured.record_count(), 2);
    assert_eq!(captured.input().memory.as_ptr(), fixture.memory.as_ptr());
    let original = validate_canonical_structured_value(&fixture.memory)
        .unwrap()
        .record_field("memories")
        .unwrap()
        .unwrap()
        .collection_index(0)
        .unwrap()
        .unwrap();
    assert_eq!(
        captured.original_record(1).unwrap().value_node(),
        original.value_node()
    );
    assert_eq!(
        captured.original_record(3).err(),
        Some(EpisodeRetrievalSnapshotRefusal::EpisodeNotFound)
    );
    captured.match_identity(&captured.identity()).unwrap();
    assert_eq!(
        captured.match_identity(&[0; 32]),
        Err(EpisodeRetrievalSnapshotRefusal::SnapshotMismatch)
    );
}
#[test]
fn identity_binds_configuration_time_owner_and_source_snapshot() {
    let prepared = PreparedEpisodeRetrievalSnapshot::prepare();
    let fixture = Fixture::new();
    let original = prepared.capture(fixture.input()).unwrap().identity();
    let mut input = fixture.input();
    input.reference.ticks += 1;
    let changed = prepared.capture(input).unwrap();
    assert_ne!(original, changed.identity());
    assert_eq!(
        changed.match_identity(&original),
        Err(EpisodeRetrievalSnapshotRefusal::SnapshotMismatch)
    );
    let other_intent = conduit_ai::TemporalRetrievalIntent::LatestEvidence
        .encode()
        .unwrap();
    let mut input = fixture.input();
    input.intent = &other_intent;
    assert_ne!(original, prepared.capture(input).unwrap().identity());
    let mut other_source = fixture.source.clone();
    other_source.identity = ResourceSemanticIdentity::from_digest([2; 32]);
    let mut input = fixture.input();
    input.source = &other_source;
    assert_ne!(original, prepared.capture(input).unwrap().identity());
}
#[test]
fn owner_order_and_native_type_mismatches_refuse() {
    let prepared = PreparedEpisodeRetrievalSnapshot::prepare();
    let fixture = Fixture::new();
    let mut input = fixture.input();
    input.expected.body_id = "body/other";
    assert_eq!(
        prepared.capture(input).err(),
        Some(EpisodeRetrievalSnapshotRefusal::OwnerMismatch)
    );
    let mut input = fixture.input();
    input.expected.wake_sequence = 4;
    assert_eq!(
        prepared.capture(input).err(),
        Some(EpisodeRetrievalSnapshotRefusal::OwnerMismatch)
    );
    let unordered = memory(&[2, 1], "body/one", "wake/one");
    let unordered_source = source(&unordered);
    let mut input = fixture.input();
    input.memory = &unordered;
    input.source = &unordered_source;
    assert_eq!(
        prepared.capture(input).err(),
        Some(EpisodeRetrievalSnapshotRefusal::MemoryOrder)
    );
    let mut input = fixture.input();
    input.intent = &fixture.memory;
    assert_eq!(
        prepared.capture(input).err(),
        Some(EpisodeRetrievalSnapshotRefusal::IntentType)
    );
    let mut input = fixture.input();
    input.experience = &fixture.memory;
    assert_eq!(
        prepared.capture(input).err(),
        Some(EpisodeRetrievalSnapshotRefusal::ExperienceType)
    );
}
#[test]
fn clock_precision_future_and_source_contracts_refuse() {
    let prepared = PreparedEpisodeRetrievalSnapshot::prepare();
    let fixture = Fixture::new();
    for change in [0, 1, 2] {
        let mut input = fixture.input();
        match change {
            0 => input.reference.scale = TemporalScale::Seconds,
            1 => input.reference.resolution_ticks = 2,
            _ => input.reference.uncertainty_ticks = 1,
        }
        assert_eq!(
            prepared.capture(input).err(),
            Some(EpisodeRetrievalSnapshotRefusal::UnsupportedTime)
        );
    }
    let mut input = fixture.input();
    input.retrieved.clock_basis = "clock/other";
    assert_eq!(
        prepared.capture(input).err(),
        Some(EpisodeRetrievalSnapshotRefusal::ClockMismatch)
    );
    let mut input = fixture.input();
    input.retrieved.ticks = 101;
    assert_eq!(
        prepared.capture(input).err(),
        Some(EpisodeRetrievalSnapshotRefusal::RetrievalAfterReference)
    );
    for change in [0, 1, 2] {
        let mut wrong = fixture.source.clone();
        match change {
            0 => wrong.extent.bytes += 1,
            1 => wrong.content_profile = kind_id("value/text"),
            _ => wrong.lifetime.version = ResourceVersionIdentity::from_digest([9; 32]),
        }
        let mut input = fixture.input();
        input.source = &wrong;
        let expected = match change {
            0 => EpisodeRetrievalSnapshotRefusal::SourceExtent,
            1 => EpisodeRetrievalSnapshotRefusal::SourceProfile,
            _ => EpisodeRetrievalSnapshotRefusal::SourceVersion,
        };
        assert_eq!(prepared.capture(input).err(), Some(expected));
    }
}
#[test]
fn finite_input_bounds_refuse_before_decoding() {
    let prepared = PreparedEpisodeRetrievalSnapshot::prepare();
    let fixture = Fixture::new();
    let oversized = vec![0; MAXIMUM_EPISODE_MEMORY_BYTES + 1];
    let mut input = fixture.input();
    input.memory = &oversized;
    assert_eq!(
        prepared.capture(input).err(),
        Some(EpisodeRetrievalSnapshotRefusal::MemoryBound)
    );
    let oversized = vec![0; MAXIMUM_EPISODE_EXPERIENCE_BYTES + 1];
    let mut input = fixture.input();
    input.experience = &oversized;
    assert_eq!(
        prepared.capture(input).err(),
        Some(EpisodeRetrievalSnapshotRefusal::ExperienceBound)
    );
    let oversized = vec![0; MAXIMUM_RETRIEVAL_CONFIGURATION_BYTES + 1];
    let mut input = fixture.input();
    input.context_policy = &oversized;
    assert_eq!(
        prepared.capture(input).err(),
        Some(EpisodeRetrievalSnapshotRefusal::ConfigurationBound)
    );
}

#[test]
fn first_empty_snapshot_and_four_record_window_keep_existing_bounds() {
    let prepared = PreparedEpisodeRetrievalSnapshot::prepare();
    let fixture = Fixture::new();
    let empty = conduit_chat::initial_episode_memory();
    let empty_source = source(&empty);
    let mut input = fixture.input();
    input.memory = &empty;
    input.source = &empty_source;
    assert_eq!(prepared.capture(input).unwrap().record_count(), 0);
    let full = memory(&[1, 2, 3, 4], "body/one", "wake/one");
    let full_source = source(&full);
    let mut input = fixture.input();
    input.memory = &full;
    input.source = &full_source;
    let snapshot = prepared.capture(input).unwrap();
    assert_eq!(snapshot.record_count(), 4);
    assert!(snapshot.original_record(4).is_ok());
    let duplicates = memory(&[1, 1], "body/one", "wake/one");
    let duplicate_source = source(&duplicates);
    let mut input = fixture.input();
    input.memory = &duplicates;
    input.source = &duplicate_source;
    assert_eq!(
        prepared.capture(input).err(),
        Some(EpisodeRetrievalSnapshotRefusal::MemoryOrder)
    );
}

#[test]
fn explicit_clock_class_is_validated_and_bound_without_text_inference() {
    use conduit_ai::BorrowedTemporalClockBasis as Clock;
    let prepared = PreparedEpisodeRetrievalSnapshot::prepare();
    let fixture = Fixture::new();
    let mut input = fixture.input();
    input.clock_basis = Clock::UnixEpochMilliseconds;
    assert_eq!(
        prepared.capture(input).err(),
        Some(EpisodeRetrievalSnapshotRefusal::ClockMismatch)
    );
    input.reference.clock_basis = UNIX_UTC_CLOCK_BASIS;
    input.retrieved.clock_basis = UNIX_UTC_CLOCK_BASIS;
    let unix = prepared.capture(input).unwrap();
    input.clock_basis = Clock::MonotonicMilliseconds(UNIX_UTC_CLOCK_BASIS);
    let named = prepared.capture(input).unwrap();
    assert_ne!(unix.identity(), named.identity());
    assert_eq!(
        named.match_identity(&unix.identity()),
        Err(EpisodeRetrievalSnapshotRefusal::SnapshotMismatch)
    );
    assert_eq!(
        named
            .records()
            .map(|(episode, _)| episode)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    for label in ["", "other"] {
        input.clock_basis = Clock::MonotonicMilliseconds(label);
        let expected = if label.is_empty() {
            EpisodeRetrievalSnapshotRefusal::UnsupportedClockBasis
        } else {
            EpisodeRetrievalSnapshotRefusal::ClockMismatch
        };
        assert_eq!(prepared.capture(input).err(), Some(expected));
    }
    let long = "x".repeat(129);
    input.clock_basis = Clock::MonotonicMilliseconds(&long);
    assert_eq!(
        prepared.capture(input).err(),
        Some(EpisodeRetrievalSnapshotRefusal::UnsupportedClockBasis)
    );
}
