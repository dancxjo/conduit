#![cfg(all(feature = "semantic-bindings", feature = "kernel"))]
#[path = "common/playback_fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "common/playback_graph.rs"]
mod graph;
use conduit_core::revision::*;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    PortId, ValueRef,
};
use conduit_speech::{
    native_playback_back::*, native_playback_contract as contract, pitch_trajectory::*,
    playback_basis::*, playback_revision::*,
};
macro_rules! tape {
    ($f:ident,$l:ident,$p:ident,$r:ident,$t:ident) => {
        let $l = $f.linguistic();
        let $p = prepare_utterance_pitch(
            &$f.source,
            &[OfferedSegmentPitch {
                event: 0,
                admission: $l.accepted().pitch(),
            }],
        )
        .unwrap();
        let $r = $f.realized();
        let $t = prepare_speech_playback_tape(
            &$r,
            &$p,
            &[PlaybackLinguisticBinding {
                event: 0,
                admitted: &$l,
            }],
            7,
        )
        .unwrap();
    };
}
fn text(value: &str) -> RevisionText<'_> {
    RevisionText::new(value).unwrap()
}
#[test]
fn changed_interpretation_requires_exact_new_plan_and_cancel_cannot_reactivate() {
    let first = fixture::fixture("Hello Travis", "r1");
    tape!(first, l, p, r, tape);
    let next = fixture::fixture("Hello Trent", "r2");
    tape!(next, nl, np, nr, replacement);
    let first_plan = graph::plan(&tape);
    let new_plan = graph::plan(&replacement);
    let gear = |plan: &conduit_core::Plan| {
        plan.fragments[0]
            .placements
            .iter()
            .find(|gear| gear.kind_id.as_str() == contract::KIND)
            .unwrap()
            .clone()
    };
    let old_gear = gear(&first_plan);
    let new_gear = gear(&new_plan);
    let domain = PlaybackRevisionDomain {
        subject: first.source.utterance_id(),
    };
    let context = RevisionContext {
        stream: text("speech"),
        subject: text("utterance"),
        epoch: text("epoch/1"),
        producer: text("owner"),
        policy: text("replan@1"),
    };
    let evidence = [RevisionEvidence {
        source: text("fixture"),
        generation: text("1"),
    }];
    let proposed = PreparedPlaybackChange::interpretation(None, &tape).unwrap();
    let revised = PreparedPlaybackChange::interpretation(Some(&tape), &replacement).unwrap();
    let reference = |sequence, event| RevisionReference {
        context,
        sequence,
        event: text(event),
    };
    let proposal = RevisionEvent::new(
        &domain,
        reference(1, "proposal"),
        &evidence,
        RevisionChange::Proposed { delta: &proposed },
    )
    .unwrap();
    let revision = RevisionEvent::new(
        &domain,
        reference(2, "revision"),
        &evidence,
        RevisionChange::Revised {
            replaces: proposal.reference(),
            delta: &revised,
        },
    )
    .unwrap();
    let producer =
        NativeSpeechPlaybackBack::prepare::<1>(&old_gear, &tape, PortId(0), PortId(0)).unwrap();
    let mut journal = PlaybackRevisionJournal::new(
        &domain,
        context,
        RevisionLimits {
            history_events: 8,
            revisable_units: 1,
        },
        producer,
    )
    .unwrap();
    journal.append(&proposal, None).unwrap();
    let new_producer =
        NativeSpeechPlaybackBack::prepare::<1>(&new_gear, &replacement, PortId(0), PortId(0))
            .unwrap();
    journal.append(&revision, Some(new_producer)).unwrap();
    assert!(journal.requires_replan());
    let trigger = 0u64.to_le_bytes();
    let reference = ValueRef {
        slot: 0,
        generation: 1,
        byte_len: 8,
    };
    let bytes = StepInputBytes::test_frame([Some(trigger.as_slice())], None);
    let mut ready = StepIo::test_frame(
        [Some(reference)],
        [false],
        [Some(contract::MAXIMUM_PCM_BYTES as u32)],
        None,
        contract::REQUIRED_STEP_FUEL,
    );
    assert_eq!(journal.step(&mut ready, &bytes), StepOutcome::Await);
    assert_eq!(journal.producer().unwrap().queued_frames(), 0);
    assert!(matches!(
        journal.rebind_plan::<1>(&old_gear),
        Err(NativePlaybackPreparationRefusal::Configuration
            | NativePlaybackPreparationRefusal::Identity)
    ));
    assert!(journal.requires_replan());
    journal.rebind_plan::<1>(&new_gear).unwrap();
    assert!(!journal.requires_replan());
    let mut ready = StepIo::test_frame(
        [Some(reference)],
        [false],
        [Some(contract::MAXIMUM_PCM_BYTES as u32)],
        None,
        contract::REQUIRED_STEP_FUEL,
    );
    assert_eq!(journal.step(&mut ready, &bytes), StepOutcome::Progress);
    StepBack::<1>::step_committed(&mut journal);
    assert_eq!(journal.producer().unwrap().queued_frames(), 128);
    StepBack::<1>::cancel(&mut journal);
    assert!(journal.rebind_plan::<1>(&new_gear).is_err());
    let mut ready = StepIo::test_frame(
        [Some(reference)],
        [false],
        [Some(contract::MAXIMUM_PCM_BYTES as u32)],
        None,
        contract::REQUIRED_STEP_FUEL,
    );
    assert_eq!(journal.step(&mut ready, &bytes), StepOutcome::Complete);
    assert_eq!(journal.producer().unwrap().queued_frames(), 128);
}
