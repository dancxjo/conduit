use conduit_core::{
    kind_id, BoundedResourceRef, ResourceClassId, ResourceExtent, ResourceLifetime,
    ResourceSemanticIdentity, ResourceVersionIdentity, TemporalInstant, TemporalScale,
};
use conduit_time::*;

fn time(ticks: u64) -> TemporalInstant {
    TemporalInstant {
        ticks,
        scale: TemporalScale::Milliseconds,
        clock_basis: "clock/source".into(),
        resolution_ticks: 1,
        uncertainty_ticks: 0,
    }
}

fn value(seed: u8, bytes: u64, profile: &str) -> BoundedResourceRef {
    BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest([seed; 32]),
        content_profile: kind_id(profile),
        access_class: ResourceClassId::from("conduit.resource/history-value@1"),
        extent: ResourceExtent {
            bytes,
            items: Some(1),
        },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest([seed.wrapping_add(1); 32]),
            expires_at: None,
        },
    }
}

fn timeline(
    entries: usize,
    bytes: u64,
    policy: HistoricalOverflowPolicy,
) -> BoundedHistoricalTimeline {
    BoundedHistoricalTimeline::new(
        kind_id("observation/temperature@1"),
        "clock/source",
        TemporalScale::Milliseconds,
        entries,
        bytes,
        policy,
        10,
    )
    .unwrap()
}

#[test]
fn exact_typed_resources_time_origin_and_sequence_are_retained() {
    let mut history = timeline(2, 20, HistoricalOverflowPolicy::Refuse);
    assert_eq!(
        history.append(
            "entry/a".into(),
            time(5),
            HistoricalEntryOrigin::MachineObservation,
            value(1, 7, "observation/temperature@1")
        ),
        Ok(10)
    );
    assert_eq!(
        history.append(
            "entry/b".into(),
            time(6),
            HistoricalEntryOrigin::OperatorAuthored,
            value(3, 8, "observation/temperature@1")
        ),
        Ok(11)
    );
    assert_eq!(history.len(), 2);
    assert_eq!(history.referenced_bytes(), 15);
    assert_eq!(history.entry(0).unwrap().identity, "entry/a");
    assert_eq!(
        history.entry(1).unwrap().origin,
        HistoricalEntryOrigin::OperatorAuthored
    );
    assert_eq!(history.entry(1).unwrap().event_time, time(6));
}

#[test]
fn refusal_policy_keeps_item_and_byte_pressure_distinct() {
    let mut item_full = timeline(1, 20, HistoricalOverflowPolicy::Refuse);
    item_full
        .append(
            "a".into(),
            time(1),
            HistoricalEntryOrigin::MachineObservation,
            value(1, 5, "observation/temperature@1"),
        )
        .unwrap();
    assert_eq!(
        item_full.append(
            "b".into(),
            time(2),
            HistoricalEntryOrigin::MachineObservation,
            value(3, 5, "observation/temperature@1")
        ),
        Err(HistoricalTimelineRefusal::Full)
    );

    let mut byte_full = timeline(3, 8, HistoricalOverflowPolicy::Refuse);
    byte_full
        .append(
            "a".into(),
            time(1),
            HistoricalEntryOrigin::MachineObservation,
            value(1, 5, "observation/temperature@1"),
        )
        .unwrap();
    assert_eq!(
        byte_full.append(
            "b".into(),
            time(2),
            HistoricalEntryOrigin::MachineObservation,
            value(3, 4, "observation/temperature@1")
        ),
        Err(HistoricalTimelineRefusal::ByteCapacityExceeded)
    );
}

#[test]
fn eviction_reports_the_exact_whole_entry_gap() {
    let mut history = timeline(3, 10, HistoricalOverflowPolicy::EvictOldestWithGap);
    history
        .append(
            "a".into(),
            time(1),
            HistoricalEntryOrigin::MachineObservation,
            value(1, 6, "observation/temperature@1"),
        )
        .unwrap();
    history
        .append(
            "b".into(),
            time(2),
            HistoricalEntryOrigin::MachineObservation,
            value(3, 4, "observation/temperature@1"),
        )
        .unwrap();
    history
        .append(
            "c".into(),
            time(3),
            HistoricalEntryOrigin::MachineObservation,
            value(5, 7, "observation/temperature@1"),
        )
        .unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history.entry(0).unwrap().identity, "c");
    assert_eq!(
        history.retention_gap(),
        Some(HistoricalRetentionGap {
            first_sequence: 10,
            last_sequence: 11,
            entries: 2,
            referenced_bytes: 10
        })
    );
    let replay = history.replay_metadata();
    assert_eq!(replay.len(), 1);
    assert_eq!(replay[0].identity, "c");
    assert_eq!(replay[0].event_time, time(3));
    let mut controller =
        BoundedReplayController::new(&replay, ReplayPolicy::OriginalTiming).unwrap();
    controller.start(100).unwrap();
    let emitted = controller.poll(100).unwrap().unwrap();
    assert_eq!(emitted.historical_identity, "c");
    assert_eq!(emitted.historical_event_time, &time(3));
}

#[test]
fn wrong_type_time_resource_order_and_entry_size_refuse_before_mutation() {
    let mut history = timeline(2, 10, HistoricalOverflowPolicy::Refuse);
    assert_eq!(
        history.append(
            "a".into(),
            time(1),
            HistoricalEntryOrigin::MachineObservation,
            value(1, 1, "observation/humidity@1")
        ),
        Err(HistoricalTimelineRefusal::WrongValueProfile)
    );
    let mut other_clock = time(1);
    other_clock.clock_basis = "clock/other".into();
    assert_eq!(
        history.append(
            "a".into(),
            other_clock,
            HistoricalEntryOrigin::MachineObservation,
            value(1, 1, "observation/temperature@1")
        ),
        Err(HistoricalTimelineRefusal::IncomparableEventTime)
    );
    assert_eq!(
        history.append(
            "huge".into(),
            time(1),
            HistoricalEntryOrigin::MachineObservation,
            value(1, 11, "observation/temperature@1")
        ),
        Err(HistoricalTimelineRefusal::EntryExceedsByteLimit)
    );
    history
        .append(
            "a".into(),
            time(2),
            HistoricalEntryOrigin::MachineObservation,
            value(1, 1, "observation/temperature@1"),
        )
        .unwrap();
    assert_eq!(
        history.append(
            "late".into(),
            time(1),
            HistoricalEntryOrigin::MachineObservation,
            value(3, 1, "observation/temperature@1")
        ),
        Err(HistoricalTimelineRefusal::ReorderedEventTime)
    );
    assert_eq!(history.len(), 1);
}

#[test]
fn timeline_value_profile_is_bounded_before_any_snapshot_exists() {
    assert!(matches!(
        BoundedHistoricalTimeline::new(
            kind_id(&"x".repeat(conduit_core::MAXIMUM_RESOURCE_REFERENCE_IDENTITY_BYTES + 1)),
            "clock/source",
            TemporalScale::Milliseconds,
            1,
            1,
            HistoricalOverflowPolicy::Refuse,
            0,
        ),
        Err(HistoricalTimelineRefusal::InvalidValueProfile)
    ));
}

#[test]
fn remove_and_clear_are_explicit_without_rewinding_sequence() {
    let mut history = timeline(3, 20, HistoricalOverflowPolicy::Refuse);
    history
        .append(
            "a".into(),
            time(1),
            HistoricalEntryOrigin::MachineObservation,
            value(1, 2, "observation/temperature@1"),
        )
        .unwrap();
    history
        .append(
            "b".into(),
            time(2),
            HistoricalEntryOrigin::MachineObservation,
            value(3, 3, "observation/temperature@1"),
        )
        .unwrap();
    assert_eq!(history.remove(10).unwrap().identity, "a");
    assert_eq!(history.entry(0).unwrap().sequence, 11);
    history.clear().unwrap();
    assert!(history.is_empty());
    assert_eq!(history.clear_revision(), 1);
    assert_eq!(
        history.append(
            "c".into(),
            time(3),
            HistoricalEntryOrigin::MachineObservation,
            value(5, 1, "observation/temperature@1")
        ),
        Ok(12)
    );
}

#[cfg(feature = "form-catalog")]
#[test]
fn typed_history_is_an_ordinary_checked_form_with_explicit_policy() {
    use conduit_form::{
        check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
        ProfileCatalog, StartupCatalog,
    };
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_historical_timeline_catalog(&mut startup, &mut profile).unwrap();
    let source = include_str!("../../../forms/bounded-typed-history/main.conduit");
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let authored =
        expand_canonical_form_for_authoring(&checked, "bounded-typed-history", &profile).unwrap();
    assert_eq!(authored.input_bindings.len(), 1);
    assert_eq!(authored.output_bindings.len(), 1);
    let history = &authored.expanded.gears[0];
    assert_eq!(history.kind_id.as_str(), HISTORICAL_TIMELINE_KIND);
    assert_eq!(history.configuration.len(), 7);
}
