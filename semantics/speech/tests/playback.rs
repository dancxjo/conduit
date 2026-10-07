#![cfg(all(feature = "kernel", feature = "semantic-bindings"))]
#[path = "common/playback_fixture.rs"]
mod fixture;
#[path = "common/playback_graph.rs"]
mod graph;
use conduit_kernel::{
    scheduler::{SchedulerError, StepBack, StepInputBytes, StepIo, StepOutcome},
    PortId, ValueRef,
};
use conduit_speech::{
    native_playback_back::*, native_playback_contract as contract, pitch_trajectory::*,
    playback_basis::*, semantic::*,
};
use std::{cell::Cell, rc::Rc};
fn back<'a>(tape: &'a PreparedSpeechPlaybackTape<'a>) -> NativeSpeechPlaybackBack<'a> {
    let plan = graph::plan(tape);
    let gear = plan.fragments[0]
        .placements
        .iter()
        .find(|p| p.kind_id.as_str() == contract::KIND)
        .unwrap();
    NativeSpeechPlaybackBack::prepare::<1>(gear, tape, PortId(0), PortId(0)).unwrap()
}
fn ack(
    tape: &PreparedSpeechPlaybackTape<'_>,
    disposition: SpeechPlaybackDisposition,
    start: u64,
    end: u64,
) -> SpeechPlaybackAcknowledgement {
    SpeechPlaybackAcknowledgement::new(
        tape.basis().clone(),
        disposition,
        start,
        SpeechEvidenceProvenance::new(
            "effect-owner/fixture".into(),
            SpeechEvidenceSource::Manual,
            None,
        )
        .unwrap(),
        end,
    )
    .unwrap()
}
macro_rules! tape {
    ($fixture:ident,$linguistic:ident,$pitch:ident,$realized:ident,$tape:ident) => {
        let $linguistic = $fixture.linguistic();
        let $pitch = prepare_utterance_pitch(
            &$fixture.source,
            &[OfferedSegmentPitch {
                event: 0,
                admission: $linguistic.accepted().pitch(),
            }],
        )
        .unwrap();
        let $realized = $fixture.realized();
        let $tape = prepare_speech_playback_tape(
            &$realized,
            &$pitch,
            &[PlaybackLinguisticBinding {
                event: 0,
                admitted: &$linguistic,
            }],
            7,
        )
        .unwrap();
    };
}
#[test]
fn staged_output_queue_and_played_feedback_are_distinct_and_source_exact() {
    let fixture = fixture::fixture("Hello Travis", "r1");
    tape!(fixture, linguistic, pitch, realized, tape);
    let foreign = fixture::fixture("Hello Trent", "r1");
    tape!(foreign, other, other_pitch, other_realized, other_tape);
    assert!(!tape.same_snapshot(&other_tape));
    assert_ne!(
        tape.basis(),
        other_tape.basis(),
        "reused IDs cannot hide changed material or alternatives"
    );
    assert!(prepare_speech_playback_tape(
        &realized,
        &pitch,
        &[PlaybackLinguisticBinding {
            event: 0,
            admitted: &other
        }],
        7
    )
    .is_err());
    let mut back = back(&tape);
    let raw = 0u64.to_le_bytes();
    let reference = ValueRef {
        slot: 0,
        generation: 7,
        byte_len: 8,
    };
    let bytes = StepInputBytes::test_frame([Some(raw.as_slice())], None);
    let mut blocked = StepIo::test_frame(
        [Some(reference)],
        [false],
        [None],
        None,
        contract::REQUIRED_STEP_FUEL,
    );
    assert_eq!(back.step(&mut blocked, &bytes), StepOutcome::Await);
    let mut ready = StepIo::test_frame(
        [Some(reference)],
        [false],
        [Some(contract::MAXIMUM_PCM_BYTES as u32)],
        None,
        contract::REQUIRED_STEP_FUEL,
    );
    assert_eq!(back.step(&mut ready, &bytes), StepOutcome::Progress);
    let first = StepBack::<1>::prepared_output(&back, PortId(0))
        .unwrap()
        .to_vec();
    assert_eq!(back.queued_frames(), 0);
    assert_eq!(back.played_frames(), 0);
    let premature = ack(&tape, SpeechPlaybackDisposition::Played, 0, 128);
    assert!(matches!(
        back.acknowledge(&premature),
        Err(PlaybackAcknowledgementRefusal::NotQueued)
    ));
    let mut retry = StepIo::test_frame(
        [Some(reference)],
        [false],
        [Some(contract::MAXIMUM_PCM_BYTES as u32)],
        None,
        contract::REQUIRED_STEP_FUEL,
    );
    assert_eq!(back.step(&mut retry, &bytes), StepOutcome::Progress);
    assert_eq!(
        StepBack::<1>::prepared_output(&back, PortId(0)).unwrap(),
        first
    );
    StepBack::<1>::step_committed(&mut back);
    assert_eq!(back.queued_frames(), 128);
    assert_eq!(back.played_frames(), 0);
    let queued = ack(&tape, SpeechPlaybackDisposition::Queued, 0, 128);
    assert!(back.acknowledge(&queued).unwrap().is_none());
    assert_eq!(back.played_frames(), 0);
    let foreign = ack(&other_tape, SpeechPlaybackDisposition::Played, 0, 128);
    assert!(matches!(
        back.acknowledge(&foreign),
        Err(PlaybackAcknowledgementRefusal::Basis)
    ));
    StepBack::<1>::cancel(&mut back);
    assert!(back.is_cancelled());
    assert_eq!(back.queued_frames(), 128);
    let proof = back.acknowledge(&premature).unwrap().unwrap();
    assert_eq!(proof.acknowledgement().basis(), tape.basis());
    assert_eq!(back.played_frames(), 128);
    assert!(matches!(
        back.acknowledge(&premature),
        Err(PlaybackAcknowledgementRefusal::Contiguity)
    ));
    let mut later = StepIo::test_frame(
        [Some(reference)],
        [false],
        [Some(contract::MAXIMUM_PCM_BYTES as u32)],
        None,
        contract::REQUIRED_STEP_FUEL,
    );
    assert_eq!(back.step(&mut later, &bytes), StepOutcome::Complete);
    assert_eq!(back.queued_frames(), 128);
}
#[test]
fn ordinary_plan_atomic_fanout_pressure_preserves_tape_and_cancel_stops_future_frames() {
    let fixture = fixture::fixture("Hello Travis", "r1");
    tape!(fixture, linguistic, pitch, realized, tape);
    let pause = Rc::new(Cell::new(true));
    let domain = PlaybackRevisionDomain {
        subject: fixture.source.utterance_id(),
    };
    let change = PreparedPlaybackChange::interpretation(None, &tape).unwrap();
    let proposal = event(
        &domain,
        1,
        "proposal",
        RevisionChange::Proposed { delta: &change },
    );
    let mut epoch = PlaybackRevisionJournal::new(
        &domain,
        context(),
        RevisionLimits {
            history_events: 16,
            revisable_units: 1,
        },
        back(&tape),
    )
    .unwrap();
    epoch.append(&proposal, None).unwrap();
    let mut scheduler = graph::scheduler_with_epoch(&tape, pause.clone(), epoch);

    for _ in 0..32 {
        scheduler.step().unwrap();
    }
    let frames = |scheduler: &graph::Scheduler<'_>| {
        scheduler
            .drivers()
            .iter()
            .find_map(|d| {
                if let graph::Driver::Voice(back) = d {
                    Some(back.queued_frames())
                } else if let graph::Driver::Epoch(epoch) = d {
                    Some(epoch.producer().unwrap().queued_frames())
                } else {
                    None
                }
            })
            .unwrap()
    };
    assert_eq!(frames(&scheduler), 128);
    for _ in 0..32 {
        scheduler.step().unwrap();
    }
    assert_eq!(frames(&scheduler), 128);
    pause.set(false);
    scheduler.run(20000).unwrap();
    assert_eq!(frames(&scheduler), 1600);
    let mut renderer = tape.renderer().unwrap();
    let mut expected = Vec::new();
    let mut pcm = [0; 128];
    while !renderer.is_complete() {
        let n = renderer.render(&mut pcm).unwrap();
        for sample in &pcm[..n] {
            expected.extend_from_slice(&sample.to_le_bytes());
        }
    }
    for driver in scheduler.drivers() {
        if let graph::Driver::Sink { pcm, complete, .. } = driver {
            assert!(*complete);
            assert_eq!(*pcm, expected);
        }
        if let graph::Driver::Voice(back) = driver {
            assert_eq!(back.played_frames(), 0);
        } else if let graph::Driver::Epoch(epoch) = driver {
            assert_eq!(epoch.producer().unwrap().played_frames(), 0);
        }
    }
    let mut cancelled = graph::scheduler(&tape, Rc::new(Cell::new(true)));
    for _ in 0..32 {
        cancelled.step().unwrap();
    }
    let before = frames(&cancelled);
    cancelled.cancel().unwrap();
    assert_eq!(cancelled.run(20000), Err(SchedulerError::Cancelled));
    assert_eq!(frames(&cancelled), before);
}
use conduit_core::revision::*;
use conduit_speech::playback_revision::*;
fn text(value: &str) -> RevisionText<'_> {
    RevisionText::new(value).unwrap()
}
fn context() -> RevisionContext<'static> {
    RevisionContext {
        stream: text("speech"),
        subject: text("utterance"),
        epoch: text("epoch/1"),
        producer: text("admitted-tape"),
        policy: text("played-epoch@1"),
    }
}
const EVIDENCE: [RevisionEvidence<'static>; 1] = [RevisionEvidence {
    source: match RevisionText::new("fixture/owner") {
        Ok(value) => value,
        Err(_) => panic!(),
    },
    generation: match RevisionText::new("1") {
        Ok(value) => value,
        Err(_) => panic!(),
    },
}];
fn event<'a>(
    domain: &'a PlaybackRevisionDomain<'a>,
    sequence: u64,
    name: &'a str,
    change: RevisionChange<'a, PreparedPlaybackChange<'a>, PlaybackEpochCursor<'a>>,
) -> RevisionEvent<'a, PlaybackRevisionDomain<'a>> {
    RevisionEvent::new(
        domain,
        RevisionReference {
            context: context(),
            sequence,
            event: text(name),
        },
        &EVIDENCE,
        change,
    )
    .unwrap()
}
#[test]
fn preplay_revision_withdrawal_and_stability_do_not_commit_playback() {
    let first = fixture::fixture("Hello Travis", "r1");
    tape!(first, linguistic, pitch, realized, tape);
    let second = fixture::fixture("Hello Trent", "r2");
    tape!(second, other, other_pitch, other_realized, replacement);
    let producer = back(&tape);
    let domain = PlaybackRevisionDomain {
        subject: first.source.utterance_id(),
    };
    let proposed = PreparedPlaybackChange::interpretation(None, &tape).unwrap();
    let revised = PreparedPlaybackChange::interpretation(Some(&tape), &replacement).unwrap();
    let withdrawn = PreparedPlaybackChange::withdrawal(&replacement).unwrap();
    let p = event(
        &domain,
        1,
        "proposal",
        RevisionChange::Proposed { delta: &proposed },
    );
    let stable = event(
        &domain,
        2,
        "stable",
        RevisionChange::Stable {
            revision: p.reference(),
            through: PlaybackEpochCursor::interpreted(),
        },
    );
    let premature = event(
        &domain,
        3,
        "premature",
        RevisionChange::Committed {
            revision: p.reference(),
            through: PlaybackEpochCursor::interpreted(),
        },
    );
    let r = event(
        &domain,
        3,
        "revision",
        RevisionChange::Revised {
            replaces: p.reference(),
            delta: &revised,
        },
    );
    let w = event(
        &domain,
        4,
        "withdrawal",
        RevisionChange::Withdrawn {
            revision: r.reference(),
            delta: &withdrawn,
            reason: text("cancelled before playback"),
        },
    );
    let mut journal = PlaybackRevisionJournal::new(
        &domain,
        context(),
        RevisionLimits {
            history_events: 16,
            revisable_units: 1,
        },
        producer,
    )
    .unwrap();
    journal.append(&p, None).unwrap();
    journal.append(&stable, None).unwrap();
    assert!(journal.journal().frontiers().stable_through.is_some());
    assert_eq!(
        journal.journal().frontiers().committed.acknowledgement(),
        None
    );
    assert_eq!(
        journal.append(&premature, None),
        Err(PlaybackRevisionRefusal::PlayedEvidence)
    );
    journal.append(&r, Some(back(&replacement))).unwrap();
    assert!(journal.journal().frontiers().stable_through.is_none());
    StepBack::<1>::cancel(&mut journal);
    journal.append(&w, None).unwrap();
    assert!(journal.journal().current_proposal().is_none());
    assert_eq!(journal.journal().history().count(), 4);
    assert_eq!(
        revised.interpretation_data().unwrap().prior().as_ref(),
        Some(tape.basis())
    );
}
#[test]
fn queued_tape_cannot_be_rewritten_and_played_correction_keeps_exact_old_receipt() {
    let first = fixture::fixture("Hello Travis", "r1");
    tape!(first, linguistic, pitch, realized, tape);
    let second = fixture::fixture("Hello Trent", "r2");
    tape!(second, other, other_pitch, other_realized, replacement);
    let domain = PlaybackRevisionDomain {
        subject: first.source.utterance_id(),
    };
    let proposed = PreparedPlaybackChange::interpretation(None, &tape).unwrap();
    let revised = PreparedPlaybackChange::interpretation(Some(&tape), &replacement).unwrap();
    let correction = PreparedPlaybackChange::correction(&tape, &replacement).unwrap();
    let wrong_correction = PreparedPlaybackChange::correction(&replacement, &tape).unwrap();
    let p = event(
        &domain,
        1,
        "proposal",
        RevisionChange::Proposed { delta: &proposed },
    );
    let r = event(
        &domain,
        2,
        "rewrite",
        RevisionChange::Revised {
            replaces: p.reference(),
            delta: &revised,
        },
    );
    let mut journal = PlaybackRevisionJournal::new(
        &domain,
        context(),
        RevisionLimits {
            history_events: 16,
            revisable_units: 1,
        },
        back(&tape),
    )
    .unwrap();
    journal.append(&p, None).unwrap();
    let raw = 0u64.to_le_bytes();
    let reference = ValueRef {
        slot: 0,
        generation: 7,
        byte_len: 8,
    };
    let bytes = StepInputBytes::test_frame([Some(raw.as_slice())], None);
    let mut ready = StepIo::test_frame(
        [Some(reference)],
        [false],
        [Some(contract::MAXIMUM_PCM_BYTES as u32)],
        None,
        contract::REQUIRED_STEP_FUEL,
    );
    assert_eq!(journal.step(&mut ready, &bytes), StepOutcome::Progress);
    StepBack::<1>::step_committed(&mut journal);
    let acknowledgement = ack(&tape, SpeechPlaybackDisposition::Played, 0, 128);
    let proof = journal.acknowledge(&acknowledgement).unwrap().unwrap();
    let commit = event(
        &domain,
        2,
        "commit",
        RevisionChange::Committed {
            revision: p.reference(),
            through: PlaybackEpochCursor::from_played(proof),
        },
    );
    let corrected = event(
        &domain,
        3,
        "corrected",
        RevisionChange::Corrected {
            commit: commit.reference(),
            delta: &correction,
            reason: text("later admitted source"),
        },
    );
    let wrong = event(
        &domain,
        3,
        "foreign",
        RevisionChange::Corrected {
            commit: commit.reference(),
            delta: &wrong_correction,
            reason: text("wrong source"),
        },
    );
    assert_eq!(
        journal.append(&r, Some(back(&replacement))),
        Err(PlaybackRevisionRefusal::QueuedHistory)
    );
    journal.append(&commit, None).unwrap();
    assert_eq!(
        journal.append(&wrong, None),
        Err(PlaybackRevisionRefusal::Source)
    );
    journal.append(&corrected, None).unwrap();
    assert_eq!(
        journal.journal().frontiers().committed.acknowledgement(),
        Some(&acknowledgement)
    );
    assert_eq!(
        correction.correction_data().unwrap().previous(),
        tape.basis()
    );
    assert_eq!(
        correction.correction_data().unwrap().replacement(),
        replacement.basis()
    );
    assert_eq!(journal.producer().unwrap().queued_frames(), 128);
    assert_eq!(journal.producer().unwrap().played_frames(), 128);
    assert_eq!(journal.journal().history().count(), 3);
    assert_eq!(journal.journal().current_proposal(), Some(p.reference()));
}
