use conduit_core::{
    kind_id, BoundedResourceRef, ResourceClassId, ResourceExtent, ResourceLifetime,
    ResourceSemanticIdentity, ResourceVersionIdentity, TemporalInstant, TemporalScale,
};
use conduit_time::*;

fn timeline() -> BoundedHistoricalTimeline {
    BoundedHistoricalTimeline::new(
        kind_id("bench/observation@1"),
        "bench/event-clock",
        TemporalScale::Milliseconds,
        2,
        16,
        HistoricalOverflowPolicy::Refuse,
        10,
    )
    .unwrap()
}

fn append(identity: &str, ticks: u64, seed: u8) -> HistoricalTimelineCommand {
    HistoricalTimelineCommand::Append {
        identity: identity.into(),
        event_time: TemporalInstant {
            ticks,
            scale: TemporalScale::Milliseconds,
            clock_basis: "bench/event-clock".into(),
            resolution_ticks: 1,
            uncertainty_ticks: 0,
        },
        origin: HistoricalEntryOrigin::MachineObservation,
        value: BoundedResourceRef {
            identity: ResourceSemanticIdentity::from_digest([seed; 32]),
            content_profile: kind_id("bench/observation@1"),
            access_class: ResourceClassId::from("conduit.resource/history-value@1"),
            extent: ResourceExtent {
                bytes: 4,
                items: Some(1),
            },
            lifetime: ResourceLifetime {
                version: ResourceVersionIdentity::from_digest([seed.wrapping_add(1); 32]),
                expires_at: None,
            },
        },
    }
}

fn encode(command: &HistoricalTimelineCommand) -> Vec<u8> {
    let mut encoded = vec![0; MAXIMUM_HISTORICAL_TIMELINE_COMMAND_BYTES];
    let length = encode_historical_timeline_command_into(command, &mut encoded).unwrap();
    encoded.truncate(length);
    encoded
}

#[test]
fn encoded_commands_emit_complete_canonical_history_snapshots() {
    let mut operation = BoundedHistoricalOperation::new(timeline());
    let mut snapshot = vec![0; MAXIMUM_HISTORICAL_TIMELINE_SNAPSHOT_BYTES];

    let appended = operation
        .apply_command(&encode(&append("observation/a", 100, 1)), &mut snapshot)
        .unwrap();
    assert_eq!(
        appended.outcome,
        HistoricalTimelineOutcome::Appended { sequence: 10 }
    );
    let restored = decode_historical_timeline(&snapshot[..appended.timeline_bytes]).unwrap();
    assert_eq!(restored.entry(0), operation.timeline().entry(0));
    assert_eq!(restored.entry(0).unwrap().identity, "observation/a");

    let cleared = operation
        .apply_command(&encode(&HistoricalTimelineCommand::Clear), &mut snapshot)
        .unwrap();
    assert_eq!(
        cleared.outcome,
        HistoricalTimelineOutcome::Cleared { revision: 1 }
    );
    let restored = decode_historical_timeline(&snapshot[..cleared.timeline_bytes]).unwrap();
    assert!(restored.is_empty());
    assert_eq!(restored.clear_revision(), 1);
}

#[test]
fn malformed_command_and_output_pressure_refuse_before_mutation() {
    let mut operation = BoundedHistoricalOperation::new(timeline());
    let mut snapshot = vec![0; MAXIMUM_HISTORICAL_TIMELINE_SNAPSHOT_BYTES];
    let append = encode(&append("observation/a", 100, 1));

    assert_eq!(
        operation.apply_command(
            &append,
            &mut snapshot[..MAXIMUM_HISTORICAL_TIMELINE_SNAPSHOT_BYTES - 1],
        ),
        Err(HistoricalOperationRefusal::TimelineOutputTooSmall)
    );
    assert!(operation.timeline().is_empty());
    assert!(matches!(
        operation.apply_command(b"bad", &mut snapshot),
        Err(HistoricalOperationRefusal::Command(_))
    ));
    assert!(operation.timeline().is_empty());

    operation.apply_command(&append, &mut snapshot).unwrap();
    assert_eq!(operation.timeline().len(), 1);
}

#[test]
fn semantic_capacity_failure_remains_distinct_and_preserves_prior_snapshot() {
    let mut operation = BoundedHistoricalOperation::new(timeline());
    let mut snapshot = vec![0; MAXIMUM_HISTORICAL_TIMELINE_SNAPSHOT_BYTES];
    operation
        .apply_command(&encode(&append("observation/a", 100, 1)), &mut snapshot)
        .unwrap();
    operation
        .apply_command(&encode(&append("observation/b", 110, 3)), &mut snapshot)
        .unwrap();
    assert_eq!(operation.timeline().len(), 2);

    assert_eq!(
        operation.apply_command(&encode(&append("observation/c", 120, 5)), &mut snapshot,),
        Err(HistoricalOperationRefusal::Timeline(
            HistoricalTimelineRefusal::Full
        ))
    );
    assert_eq!(operation.timeline().len(), 2);
    assert_eq!(
        operation.timeline().entry(0).unwrap().identity,
        "observation/a"
    );
    assert_eq!(
        operation.timeline().entry(1).unwrap().identity,
        "observation/b"
    );
}
