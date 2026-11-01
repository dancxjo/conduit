use super::*;
use conduit_environment_measurement::source_observation::{
    PreparedObservationCodec, SourceFailure, SourceObservation,
};
fn state() -> EnvironmentalFaceState {
    EnvironmentalFaceState {
        host: "host/collector".into(),
        boot: "boot/collector".into(),
        session: "session/green".into(),
        capability: "capability/green".into(),
        basis: FaceContributionBasis {
            checked_plot_id: "checked/sample".into(),
            plan_id: "plan/sample".into(),
            active_play_id: "play/sample".into(),
            required_interaction_context: None,
        },
        subject: "instrument/green".into(),
        name: "Green".into(),
        stale_after_ms: 100,
        bytes: [[0; MAXIMUM_BYTES]; MAX_SOURCE_SERIES_SAMPLES],
        lengths: [0; MAX_SOURCE_SERIES_SAMPLES],
        sequences: [0; MAX_SOURCE_SERIES_SAMPLES],
        count: 0,
        next: 0,
        accepted: 0,
        last_now: 0,
        last_observed: 0,
        last_counter: None,
        last_reported: None,
        active: true,
        contract: ObservationContract::prepare().unwrap(),
    }
}
fn scope() -> (HostId, BootId, BaseInstanceId, CapabilityId, ActivePlayId) {
    (
        "host/collector".into(),
        "boot/collector".into(),
        "session/green".into(),
        "capability/green".into(),
        "play/sample".into(),
    )
}
fn value(time: u64) -> Vec<u8> {
    let contract = ObservationContract::prepare().unwrap();
    PreparedObservationCodec::prepare(&contract)
        .unwrap()
        .encode(SourceObservation {
            observed_at_host_boot_ms: time,
            outcome: SourceOutcome::Unavailable(SourceFailure::CaptureTimeout),
        })
        .unwrap()
        .to_vec()
}
#[test]
fn bounds_preserve_original_bytes_and_order_after_eviction() {
    let mut state = state();
    let (host, boot, session, capability, play) = scope();
    let scope = CompletionScope {
        host: &host,
        boot: &boot,
        session: &session,
        capability: &capability,
        play: &play,
    };
    for sequence in 1..=20 {
        state
            .accept(&scope, sequence, &value(sequence), sequence)
            .unwrap();
    }
    let records = state.records();
    assert_eq!(records.len(), MAX_SOURCE_SERIES_SAMPLES);
    assert_eq!(records[0].sequence, 5);
    assert_eq!(records[15].sequence, 20);
    assert_eq!(records[0].observation, value(5));
    assert!(contribution(&[&state], 20).is_ok());
}
#[test]
fn wrong_boot_duplicate_sequence_future_clock_and_bad_value_leave_state_unchanged() {
    let mut state = state();
    let (host, boot, session, capability, play) = scope();
    let scope = CompletionScope {
        host: &host,
        boot: &boot,
        session: &session,
        capability: &capability,
        play: &play,
    };
    state.accept(&scope, 1, &value(10), 10).unwrap();
    assert_eq!(
        state.accept(&scope, 1, &value(11), 11),
        Err(Refusal::Sequence)
    );
    assert_eq!(state.accept(&scope, 2, &value(12), 11), Err(Refusal::Clock));
    assert_eq!(
        state.accept(&scope, 2, b"invalid", 11),
        Err(Refusal::Observation)
    );
    let other_boot = "boot/foreign".into();
    let foreign = CompletionScope {
        boot: &other_boot,
        ..scope
    };
    assert_eq!(
        state.accept(&foreign, 2, &value(11), 11),
        Err(Refusal::Scope)
    );
    assert_eq!(state.accepted, 1);
    assert_eq!(state.records()[0].observation, value(10));
}
#[test]
fn retirement_erases_cache_and_late_completion_cannot_restore_contribution() {
    let mut state = state();
    let (host, boot, session, capability, play) = scope();
    let scope = CompletionScope {
        host: &host,
        boot: &boot,
        session: &session,
        capability: &capability,
        play: &play,
    };
    state.accept(&scope, 1, &value(10), 10).unwrap();
    state.retire(&scope).unwrap();
    assert_eq!(
        state.accept(&scope, 2, &value(11), 11),
        Err(Refusal::Retired)
    );
    assert!(state.bytes.iter().flatten().all(|byte| *byte == 0));
    assert!(contribution(&[&state], 11).is_err());
}
