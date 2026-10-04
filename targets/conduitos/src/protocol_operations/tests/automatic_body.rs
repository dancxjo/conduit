//! Canonical Body admission, publication and retirement around the protocol kernel.
use super::automatic_events::{InertBus, InertClock, prepare, prepare_with_body_edit};

#[test]
fn automatic_bus_time_topology_plans_against_exact_retained_native_owners() {
    let (mut play, _, _) = prepare(InertBus, InertClock);
    assert!(play.session().realization().unwrap().play.is_none());
    assert!(matches!(
        play.step(),
        Err(crate::protocol_source::ProtocolBodyRefusal::NotPlaying)
    ));
    play.start().unwrap();
    let current = play.session().realization().unwrap();
    assert!(current.play.as_ref().unwrap().validate_for(&current.plan));
    assert!(play.start().is_err());
    play.cancel().unwrap();
    assert!(play.session().realization().is_none());
    assert_eq!(
        play.session().evidence().body.state,
        conduit_body::BodyState::Lulled
    );
    assert!(play.step().is_err());
    assert!(play.start().is_err());
}

#[test]
fn unstarted_protocol_body_retires_without_native_effects_or_play_evidence() {
    let (mut play, _, _) = prepare(InertBus, InertClock);
    play.cancel().unwrap();
    assert!(play.session().realization().is_none());
    assert!(play.kernel().is_none());
    assert_eq!(
        play.session().evidence().body.state,
        conduit_body::BodyState::Lulled
    );
    play.session().evidence().validate().unwrap();
    assert!(play.start().is_err());
}

#[test]
fn refused_protocol_body_preparation_returns_the_unchanged_biography() {
    let mut before = None;
    let (result, _, _) = prepare_with_body_edit(InertBus, InertClock, |session| {
        let fragment = &session.realization().unwrap().plan.plots[0].plan.fragments[0];
        let (host, boot) = (fragment.host_id.clone(), fragment.boot_id.clone());
        session.lull(&host, &boot, None).unwrap();
        before = Some(session.evidence().clone());
    });
    let Err(refusal) = result else {
        panic!("missing proposal must refuse preparation")
    };
    assert!(matches!(
        refusal.reason,
        crate::protocol_source::ProtocolBodyRefusal::Lifecycle(
            conduit_body::BodyLifecycleSessionError::NoProposal
        )
    ));
    assert_eq!(refusal.session.evidence(), before.as_ref().unwrap());
    assert!(refusal.session.realization().is_none());
}
