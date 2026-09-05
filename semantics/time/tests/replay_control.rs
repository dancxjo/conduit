use conduit_core::{ConfigurationEntry, ConfigurationValue, TemporalInstant, TemporalScale};
use conduit_time::*;

fn entry(identity: impl Into<String>, ticks: u64) -> HistoricalReplayEntry {
    HistoricalReplayEntry {
        identity: identity.into(),
        event_time: TemporalInstant {
            ticks,
            scale: TemporalScale::Milliseconds,
            clock_basis: "history-clock".into(),
            resolution_ticks: 1,
            uncertainty_ticks: 0,
        },
    }
}

fn entries() -> Vec<HistoricalReplayEntry> {
    vec![
        entry("event/a", 100),
        entry("event/b", 120),
        entry("event/c", 160),
    ]
}

#[test]
fn original_timing_preserves_history_and_uses_a_separate_playback_clock() {
    let mut replay =
        BoundedReplayController::new(&entries(), ReplayPolicy::OriginalTiming).unwrap();
    replay.start(1_000).unwrap();
    let first = replay.poll(1_000).unwrap().unwrap();
    assert_eq!(
        (
            first.ordinal,
            first.historical_identity,
            first.historical_event_time.ticks
        ),
        (0, "event/a", 100)
    );
    assert_eq!(first.playback_ticks, 1_000);
    assert_eq!(replay.poll(999), Err(ReplayRefusal::PlaybackClockRegressed));
    assert_eq!(replay.poll(1_019), Ok(None));
    assert_eq!(
        replay.poll(1_020).unwrap().unwrap().historical_identity,
        "event/b"
    );
    assert_eq!(
        replay
            .poll(1_060)
            .unwrap()
            .unwrap()
            .historical_event_time
            .ticks,
        160
    );
    assert_eq!(replay.state(), ReplayState::Completed);
}

#[test]
fn pause_excludes_paused_duration_and_rate_scales_only_playback_schedule() {
    let mut replay = BoundedReplayController::new(
        &entries(),
        ReplayPolicy::Rate {
            numerator: 2,
            denominator: 1,
        },
    )
    .unwrap();
    replay.start(10).unwrap();
    replay.poll(10).unwrap().unwrap();
    replay.pause(14).unwrap();
    assert_eq!(replay.poll(100), Err(ReplayRefusal::InvalidState));
    replay.resume(100).unwrap();
    assert_eq!(replay.poll(105), Ok(None));
    let second = replay.poll(106).unwrap().unwrap();
    assert_eq!(second.historical_event_time.ticks, 120);
    assert_eq!(second.playback_ticks, 106);
}

#[test]
fn step_restart_failure_and_invalid_state_are_explicit() {
    let mut replay = BoundedReplayController::new(&entries(), ReplayPolicy::Step).unwrap();
    replay.start(5).unwrap();
    assert_eq!(replay.poll(5), Err(ReplayRefusal::InvalidState));
    assert_eq!(replay.step(99).unwrap().ordinal, 0);
    replay.fail(7).unwrap();
    assert_eq!(replay.state(), ReplayState::Failed { code: 7 });
    assert_eq!(replay.step(100), Err(ReplayRefusal::InvalidState));
    replay.restart();
    replay.start(200).unwrap();
    assert_eq!(replay.step(201).unwrap().historical_identity, "event/a");
}

#[test]
fn timeline_identity_order_count_rate_and_clock_fail_closed() {
    assert!(matches!(
        BoundedReplayController::new(&[], ReplayPolicy::Step),
        Err(ReplayRefusal::EmptyTimeline)
    ));
    let duplicate = vec![entry("same", 1), entry("same", 2)];
    assert!(matches!(
        BoundedReplayController::new(&duplicate, ReplayPolicy::Step),
        Err(ReplayRefusal::DuplicateIdentity)
    ));
    let reordered = vec![entry("a", 2), entry("b", 1)];
    assert!(matches!(
        BoundedReplayController::new(&reordered, ReplayPolicy::Step),
        Err(ReplayRefusal::ReorderedHistoricalTime)
    ));
    let mut invalid_time = entry("invalid", 1);
    invalid_time.event_time.resolution_ticks = 0;
    assert!(matches!(
        BoundedReplayController::new(&[invalid_time], ReplayPolicy::Step),
        Err(ReplayRefusal::InvalidHistoricalTime)
    ));
    let mut incomparable = vec![entry("a", 1), entry("b", 2)];
    incomparable[1].event_time.scale = TemporalScale::Seconds;
    assert!(matches!(
        BoundedReplayController::new(&incomparable, ReplayPolicy::Step),
        Err(ReplayRefusal::IncomparableHistoricalTime)
    ));
    assert!(matches!(
        BoundedReplayController::new(
            &entries(),
            ReplayPolicy::Rate {
                numerator: 0,
                denominator: 1
            }
        ),
        Err(ReplayRefusal::InvalidRate)
    ));
    let too_many = (0..=MAXIMUM_REPLAY_ENTRIES)
        .map(|index| entry(format!("event/{index}"), index as u64))
        .collect::<Vec<_>>();
    assert!(matches!(
        BoundedReplayController::new(&too_many, ReplayPolicy::Step),
        Err(ReplayRefusal::TooManyEntries)
    ));
    let long_identity = vec![entry("x".repeat(MAXIMUM_REPLAY_IDENTITY_BYTES + 1), 0)];
    assert!(matches!(
        BoundedReplayController::new(&long_identity, ReplayPolicy::Step),
        Err(ReplayRefusal::IdentityTooLong)
    ));
    let mut replay =
        BoundedReplayController::new(&entries(), ReplayPolicy::OriginalTiming).unwrap();
    replay.start(20).unwrap();
    assert_eq!(replay.poll(19), Err(ReplayRefusal::PlaybackClockRegressed));
}

#[test]
fn arithmetic_refusal_does_not_advance_the_playback_clock() {
    let entries = vec![entry("event/first", 0), entry("event/overflow", u64::MAX)];
    let mut replay = BoundedReplayController::new(
        &entries,
        ReplayPolicy::Rate {
            numerator: 1,
            denominator: MAXIMUM_REPLAY_RATE_TERM,
        },
    )
    .unwrap();
    replay.start(0).unwrap();
    assert_eq!(replay.poll(0).unwrap().unwrap().ordinal, 0);

    assert_eq!(replay.poll(10), Err(ReplayRefusal::ArithmeticOverflow));
    assert_eq!(replay.cursor(), 1);
    assert_eq!(replay.state(), ReplayState::Running);
    assert_eq!(replay.poll(9), Err(ReplayRefusal::ArithmeticOverflow));
    assert_eq!(replay.cursor(), 1);
    assert_eq!(replay.state(), ReplayState::Running);
}

#[test]
fn replay_policy_is_exact_inspectable_configuration() {
    let entries = |mode: ConfigurationValue, numerator, denominator| {
        vec![
            ConfigurationEntry {
                key: "mode".into(),
                value: mode,
            },
            ConfigurationEntry {
                key: "rate-numerator".into(),
                value: ConfigurationValue::U64(numerator),
            },
            ConfigurationEntry {
                key: "rate-denominator".into(),
                value: ConfigurationValue::U64(denominator),
            },
        ]
    };
    assert_eq!(
        replay_policy_from_configuration(&entries(
            ConfigurationValue::Text(REPLAY_MODE_STEP.into()),
            1,
            1,
        )),
        Ok(ReplayPolicy::Step)
    );
    assert_eq!(
        replay_policy_from_configuration(&entries(
            ConfigurationValue::Text(REPLAY_MODE_RATE.into()),
            2,
            3,
        )),
        Ok(ReplayPolicy::Rate {
            numerator: 2,
            denominator: 3,
        })
    );
    assert_eq!(
        replay_policy_from_configuration(&entries(
            ConfigurationValue::Text("invented".into()),
            1,
            1,
        )),
        Err(ReplayPolicyConfigurationRefusal::UnknownMode)
    );
    assert_eq!(
        replay_policy_from_configuration(&entries(
            ConfigurationValue::Text(REPLAY_MODE_RATE.into()),
            0,
            1,
        )),
        Err(ReplayPolicyConfigurationRefusal::RateOutOfBounds)
    );
    assert_eq!(
        replay_policy_from_configuration(&[]),
        Err(ReplayPolicyConfigurationRefusal::Missing("mode"))
    );
    let mut wrong_type = entries(ConfigurationValue::Text(REPLAY_MODE_RATE.into()), 1, 1);
    wrong_type[1].value = ConfigurationValue::Text("one".into());
    assert_eq!(
        replay_policy_from_configuration(&wrong_type),
        Err(ReplayPolicyConfigurationRefusal::WrongType(
            "rate-numerator"
        ))
    );
}

#[cfg(feature = "form-catalog")]
#[test]
fn replay_control_is_an_ordinary_checked_form() {
    use conduit_form::{
        check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
        ProfileCatalog, StartupCatalog,
    };
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_replay_control_catalog(&mut startup, &mut profile).unwrap();
    let source = include_str!("../../../forms/bounded-replay-control/main.conduit");
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let authored =
        expand_canonical_form_for_authoring(&checked, "bounded-replay-control", &profile).unwrap();
    assert_eq!(authored.input_bindings.len(), 3);
    assert_eq!(authored.output_bindings.len(), 2);
    assert_eq!(
        authored.expanded.gears[0].kind_id.as_str(),
        REPLAY_CONTROL_KIND
    );
    assert_eq!(authored.expanded.gears[0].configuration.len(), 3);
    assert_eq!(
        replay_policy_from_configuration(&authored.expanded.gears[0].configuration),
        Ok(ReplayPolicy::OriginalTiming)
    );
}
