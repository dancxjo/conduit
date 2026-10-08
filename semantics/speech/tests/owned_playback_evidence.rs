#![cfg(all(feature = "semantic-bindings", feature = "kernel"))]
#[path = "common/playback_fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "common/playback_graph.rs"]
mod graph;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    PortId, ValueRef,
};
use conduit_speech::{
    native_playback_back::*, native_playback_contract as contract, owned_playback_evidence::*,
    pitch_trajectory::*, playback_basis::*, semantic::*,
};
use std::rc::Rc;

#[test]
fn owned_assertion_preserves_epoch_and_refuses_foreign_owners_before_consumption() {
    let fixture = fixture::fixture("Hello Travis", "r1");
    let linguistic = fixture.linguistic();
    let pitch = prepare_utterance_pitch(
        &fixture.source,
        &[OfferedSegmentPitch {
            event: 0,
            admission: linguistic.accepted().pitch(),
        }],
    )
    .unwrap();
    let realized = fixture.realized();
    let prepare = || {
        Rc::new(
            prepare_speech_playback_tape(
                &realized,
                &pitch,
                &[PlaybackLinguisticBinding {
                    event: 0,
                    admitted: &linguistic,
                }],
                7,
            )
            .unwrap(),
        )
    };
    let tape = prepare();
    let foreign = prepare();
    assert!(tape.same_snapshot(&foreign));
    let plan = graph::plan(&tape);
    let gear = plan.fragments[0]
        .placements
        .iter()
        .find(|p| p.kind_id.as_str() == contract::KIND)
        .unwrap();
    let mut back =
        NativeSpeechPlaybackBack::prepare::<1>(gear, &tape, PortId(0), PortId(0)).unwrap();
    let ack = |disposition, first, through| {
        Rc::new(
            SpeechPlaybackAcknowledgement::new(
                tape.basis().clone(),
                disposition,
                first,
                SpeechEvidenceProvenance::new(
                    "explicit test effect-owner assertion".into(),
                    SpeechEvidenceSource::Manual,
                    None,
                )
                .unwrap(),
                through,
            )
            .unwrap(),
        )
    };
    let played = ack(SpeechPlaybackDisposition::Played, 0, 128);
    assert!(matches!(
        acknowledge_owned(&mut back, Rc::clone(&tape), Rc::clone(&played)),
        Err(OwnedPlaybackAcknowledgementRefusal::Acknowledgement(
            PlaybackAcknowledgementRefusal::NotQueued
        ))
    ));
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
    assert_eq!(
        StepBack::<1>::step(&mut back, &mut ready, &bytes),
        StepOutcome::Progress
    );
    StepBack::<1>::step_committed(&mut back);
    assert_eq!(back.queued_frames(), 128);
    assert!(matches!(
        acknowledge_owned(&mut back, foreign, Rc::clone(&played)),
        Err(OwnedPlaybackAcknowledgementRefusal::ForeignTapeOwner)
    ));
    assert_eq!(back.played_frames(), 0);
    assert!(matches!(
        acknowledge_owned(
            &mut back,
            Rc::clone(&tape),
            ack(SpeechPlaybackDisposition::Played, 1, 128)
        ),
        Err(OwnedPlaybackAcknowledgementRefusal::Acknowledgement(
            PlaybackAcknowledgementRefusal::Contiguity
        ))
    ));
    assert_eq!(back.played_frames(), 0);
    assert!(acknowledge_owned(
        &mut back,
        Rc::clone(&tape),
        ack(SpeechPlaybackDisposition::Queued, 0, 128)
    )
    .unwrap()
    .is_none());
    assert_eq!(back.played_frames(), 0);
    StepBack::<1>::cancel(&mut back);
    let evidence = acknowledge_owned(&mut back, Rc::clone(&tape), Rc::clone(&played))
        .unwrap()
        .unwrap();
    assert!(core::ptr::eq(evidence.tape(), tape.as_ref()));
    assert!(core::ptr::eq(evidence.acknowledgement(), played.as_ref()));
    assert!(core::ptr::eq(evidence.source(), &fixture.source));
    assert_eq!(evidence.coverage(), PlaybackEffectCoverage::EpochOnly);
    assert_eq!((evidence.first_frame(), evidence.through_frame()), (0, 128));
    assert_eq!(back.played_frames(), 128);
    assert_eq!(Rc::strong_count(&tape), 2);
    assert_eq!(Rc::strong_count(&played), 2);
    drop(played);
    assert_eq!(
        evidence.acknowledgement().provenance().method(),
        "explicit test effect-owner assertion"
    );
    drop(evidence);
    assert_eq!(Rc::strong_count(&tape), 1);
}
