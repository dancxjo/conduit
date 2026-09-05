use conduit_core::{
    kind_id, BoundedResourceRef, ResourceClassId, ResourceExtent, ResourceLifetime,
    ResourceSemanticIdentity, ResourceVersionIdentity, TemporalInstant, TemporalScale,
};
use conduit_time::*;

fn value(seed: u8) -> BoundedResourceRef {
    BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest([seed; 32]),
        content_profile: kind_id("bench/record@1"),
        access_class: ResourceClassId::from("conduit.resource/history-value@1"),
        extent: ResourceExtent {
            bytes: 4,
            items: Some(1),
        },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest([seed.wrapping_add(1); 32]),
            expires_at: None,
        },
    }
}

fn event_time(ticks: u64) -> TemporalInstant {
    TemporalInstant {
        ticks,
        scale: TemporalScale::Milliseconds,
        clock_basis: "bench/session-clock".into(),
        resolution_ticks: 1,
        uncertainty_ticks: 0,
    }
}

#[test]
fn bench_history_projects_only_retained_entries_and_keeps_the_gap_explicit() {
    let mut history = BoundedHistoricalTimeline::new(
        kind_id("bench/record@1"),
        "bench/session-clock",
        TemporalScale::Milliseconds,
        2,
        8,
        HistoricalOverflowPolicy::EvictOldestWithGap,
        40,
    )
    .unwrap();
    for (identity, ticks, seed) in [
        ("bench/record/a", 100, 1),
        ("bench/record/b", 120, 3),
        ("bench/record/c", 150, 5),
    ] {
        history
            .append(
                identity.into(),
                event_time(ticks),
                HistoricalEntryOrigin::MachineObservation,
                value(seed),
            )
            .unwrap();
    }

    let projection = project_replay_source(&history);
    assert_eq!(
        projection.entries,
        vec![
            HistoricalReplayEntry {
                identity: "bench/record/b".into(),
                event_ticks: 120,
            },
            HistoricalReplayEntry {
                identity: "bench/record/c".into(),
                event_ticks: 150,
            },
        ]
    );
    assert_eq!(
        projection.retention_gap,
        Some(HistoricalRetentionGap {
            first_sequence: 40,
            last_sequence: 40,
            entries: 1,
            referenced_bytes: 4,
        })
    );

    let mut replay =
        BoundedReplayController::new(&projection.entries, ReplayPolicy::OriginalTiming).unwrap();
    replay.start(1_000).unwrap();
    assert_eq!(
        replay.poll(1_000).unwrap().unwrap().historical_identity,
        "bench/record/b"
    );
    assert_eq!(
        replay.poll(1_030).unwrap().unwrap().historical_identity,
        "bench/record/c"
    );
}

#[cfg(feature = "form-catalog")]
#[test]
fn replay_source_is_an_ordinary_checked_form_between_history_and_control() {
    use conduit_form::{
        check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
        ProfileCatalog, StartupCatalog,
    };

    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_historical_timeline_catalog(&mut startup, &mut profile).unwrap();
    install_replay_control_catalog(&mut startup, &mut profile).unwrap();
    install_replay_source_catalog(&mut startup, &mut profile).unwrap();
    let source = include_str!("../../../forms/bounded-replay-source/main.conduit");
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let authored =
        expand_canonical_form_for_authoring(&checked, "bounded-replay-source", &profile).unwrap();
    assert_eq!(authored.input_bindings.len(), 1);
    assert_eq!(authored.output_bindings.len(), 2);
    assert_eq!(
        authored.expanded.gears[0].kind_id.as_str(),
        REPLAY_SOURCE_KIND
    );
    assert_eq!(
        authored.expanded.gears[0].kind_contract_revision.as_str(),
        REPLAY_SOURCE_CONTRACT_REVISION
    );

    let composition = r#"form history-replay-pipeline (
    > command: HistoricalTimelineCommand...|
    > control: ReplayControl...|
    > clock: PlaybackTick...|
    event: ReplayEvent...| >
    state: ReplayState...| >
    gap: HistoricalRetentionGap >
) {
    history: history/bounded-typed(value-profile = "bench/record@1", maximum-entries = 8, maximum-referenced-bytes = 4096, overflow-policy = "evict-oldest-with-gap")
    source: history/replay-source
    replay: time/replay-control("original-timing", 1, 1)
    command > history.command
    history.timeline > source.timeline
    source.replay > replay.timeline
    control > replay.control
    clock > replay.clock
    replay.event > event
    replay.state > state
    source.gap > gap
}
"#;
    let checked = check_syntax_document(&parse_syntax_document(composition), &startup).unwrap();
    let authored =
        expand_canonical_form_for_authoring(&checked, "history-replay-pipeline", &profile).unwrap();
    assert_eq!(authored.expanded.gears.len(), 3);
    assert_eq!(authored.expanded.connections.len(), 2);
    assert_eq!(authored.input_bindings.len(), 3);
    assert_eq!(authored.output_bindings.len(), 3);
}
