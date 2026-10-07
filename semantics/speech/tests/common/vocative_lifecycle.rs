use super::graph;
use conduit_core::revision::*;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    PortId, ValueRef,
};
use conduit_speech::{
    native_playback_back::*, native_playback_contract as contract, playback_basis::*,
    playback_revision::*, semantic::*,
};
pub fn back<'a>(tape: &'a PreparedSpeechPlaybackTape<'a>) -> NativeSpeechPlaybackBack<'a> {
    let plan = graph::plan(tape);
    let gear = plan.fragments[0]
        .placements
        .iter()
        .find(|p| p.kind_id.as_str() == contract::KIND)
        .unwrap();
    NativeSpeechPlaybackBack::prepare::<1>(gear, tape, PortId(0), PortId(0)).unwrap()
}
pub fn ack(
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
fn text(value: &str) -> RevisionText<'_> {
    RevisionText::new(value).unwrap()
}
pub fn context() -> RevisionContext<'static> {
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
pub fn event<'a>(
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

/// One producer owns the queue and played receipt throughout this epoch.
pub fn exercise<'a>(
    tape: &'a PreparedSpeechPlaybackTape<'a>,
    replacement: &'a PreparedSpeechPlaybackTape<'a>,
) {
    let domain = PlaybackRevisionDomain {
        subject: tape.source().utterance_id(),
    };
    let proposed = PreparedPlaybackChange::interpretation(None, tape).unwrap();
    let revised = PreparedPlaybackChange::interpretation(Some(tape), replacement).unwrap();
    let correction = PreparedPlaybackChange::correction(tape, replacement).unwrap();
    assert!(PreparedPlaybackChange::correction(replacement, tape).is_err());
    let foreign_correction = PreparedPlaybackChange::correction(replacement, replacement).unwrap();
    let proposal = event(
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
            revision: proposal.reference(),
            through: PlaybackEpochCursor::interpreted(),
        },
    );
    let premature = event(
        &domain,
        3,
        "premature",
        RevisionChange::Committed {
            revision: proposal.reference(),
            through: PlaybackEpochCursor::interpreted(),
        },
    );
    let rewrite = event(
        &domain,
        3,
        "rewrite",
        RevisionChange::Revised {
            replaces: proposal.reference(),
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
        back(tape),
    )
    .unwrap();
    journal.append(&proposal, None).unwrap();
    journal.append(&stable, None).unwrap();
    assert!(journal.journal().frontiers().stable_through.is_some());
    assert_eq!(
        journal.append(&premature, None),
        Err(PlaybackRevisionRefusal::PlayedEvidence)
    );
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
    assert_eq!(journal.step(&mut blocked, &bytes), StepOutcome::Await);
    let mut ready = StepIo::test_frame(
        [Some(reference)],
        [false],
        [Some(contract::MAXIMUM_PCM_BYTES as u32)],
        None,
        contract::REQUIRED_STEP_FUEL,
    );
    assert_eq!(journal.step(&mut ready, &bytes), StepOutcome::Progress);
    assert_eq!(journal.producer().unwrap().queued_frames(), 0);
    let premature_ack = ack(tape, SpeechPlaybackDisposition::Played, 0, 128);
    assert!(journal.acknowledge(&premature_ack).is_err());
    StepBack::<1>::step_committed(&mut journal);
    assert_eq!(journal.producer().unwrap().queued_frames(), 128);
    let queued_ack = ack(tape, SpeechPlaybackDisposition::Queued, 0, 128);
    assert!(journal.acknowledge(&queued_ack).unwrap().is_none());
    assert_eq!(journal.producer().unwrap().played_frames(), 0);
    assert_eq!(
        journal.append(&rewrite, Some(back(replacement))),
        Err(PlaybackRevisionRefusal::QueuedHistory)
    );
    let foreign_ack = ack(replacement, SpeechPlaybackDisposition::Played, 0, 128);
    assert!(journal.acknowledge(&foreign_ack).is_err());
    let played = ack(tape, SpeechPlaybackDisposition::Played, 0, 128);
    let proof = journal.acknowledge(&played).unwrap().unwrap();
    let committed = event(
        &domain,
        3,
        "commit",
        RevisionChange::Committed {
            revision: proposal.reference(),
            through: PlaybackEpochCursor::from_played(proof),
        },
    );
    let stale = event(
        &domain,
        4,
        "foreign-correction",
        RevisionChange::Corrected {
            commit: committed.reference(),
            delta: &foreign_correction,
            reason: text("wrong immutable source"),
        },
    );
    let corrected = event(
        &domain,
        4,
        "correction",
        RevisionChange::Corrected {
            commit: committed.reference(),
            delta: &correction,
            reason: text("later admitted revision"),
        },
    );
    journal.append(&committed, None).unwrap();
    assert_eq!(
        journal.append(&stale, None),
        Err(PlaybackRevisionRefusal::Source)
    );
    journal.append(&corrected, None).unwrap();
    assert_eq!(
        journal.journal().frontiers().committed.acknowledgement(),
        Some(&played)
    );
    assert_eq!(correction.old().unwrap().basis(), tape.basis());
    assert_eq!(correction.next().unwrap().basis(), replacement.basis());
    StepBack::<1>::cancel(&mut journal);
    let mut later = StepIo::test_frame(
        [Some(reference)],
        [false],
        [Some(contract::MAXIMUM_PCM_BYTES as u32)],
        None,
        contract::REQUIRED_STEP_FUEL,
    );
    assert_eq!(journal.step(&mut later, &bytes), StepOutcome::Complete);
    assert_eq!(journal.producer().unwrap().queued_frames(), 128);
    assert_eq!(journal.producer().unwrap().played_frames(), 128);
}

pub fn scheduler_pressure(tape: &PreparedSpeechPlaybackTape<'_>) -> Vec<u8> {
    use conduit_kernel::scheduler::SchedulerError;
    use std::{cell::Cell, rc::Rc};
    let pause = Rc::new(Cell::new(true));
    let domain = PlaybackRevisionDomain {
        subject: tape.source().utterance_id(),
    };
    let change = PreparedPlaybackChange::interpretation(None, tape).unwrap();
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
        back(tape),
    )
    .unwrap();
    epoch.append(&proposal, None).unwrap();
    let mut scheduler = graph::scheduler_with_epoch(tape, pause.clone(), epoch);
    let frames = |scheduler: &graph::Scheduler<'_>| {
        scheduler
            .drivers()
            .iter()
            .find_map(|driver| match driver {
                graph::Driver::Epoch(epoch) => Some(epoch.producer().unwrap().queued_frames()),
                graph::Driver::Voice(back) => Some(back.queued_frames()),
                _ => None,
            })
            .unwrap()
    };
    for _ in 0..32 {
        scheduler.step().unwrap();
    }
    assert_eq!(frames(&scheduler), 128);
    for _ in 0..32 {
        scheduler.step().unwrap();
    }
    assert_eq!(frames(&scheduler), 128);
    pause.set(false);
    scheduler.run(20000).unwrap();
    let mut renderer = tape.renderer().unwrap();
    let mut expected = Vec::new();
    let mut pcm = [0; 128];
    while !renderer.is_complete() {
        let n = renderer.render(&mut pcm).unwrap();
        for sample in &pcm[..n] {
            expected.extend_from_slice(&sample.to_le_bytes());
        }
    }
    assert_eq!(frames(&scheduler) as usize, expected.len() / 2);
    for driver in scheduler.drivers() {
        if let graph::Driver::Sink { pcm, complete, .. } = driver {
            assert!(*complete);
            assert_eq!(*pcm, expected);
        }
        if let graph::Driver::Epoch(epoch) = driver {
            assert_eq!(epoch.producer().unwrap().played_frames(), 0);
        }
    }
    let mut cancelled = graph::scheduler(tape, Rc::new(Cell::new(true)));
    for _ in 0..32 {
        cancelled.step().unwrap();
    }
    let before = frames(&cancelled);
    cancelled.cancel().unwrap();
    assert_eq!(cancelled.run(20000), Err(SchedulerError::Cancelled));
    assert_eq!(frames(&cancelled), before);
    expected
}

pub fn native_table_refusals(tape: &PreparedSpeechPlaybackTape<'_>) {
    use conduit_plot::rust_binding::BoundedSequence;
    let original = tape.basis().links().occurrences().iter().next().unwrap();
    for bad_index in [
        31,
        (*original.linguistic_index() + 1) % tape.basis().links().linguistic_bases().len() as u64,
    ] {
        let bad = SpeechPlaybackOccurrenceBasis::new(
            *original.event(),
            bad_index,
            original.prosody().clone(),
            original.source().clone(),
            original.token().clone(),
        )
        .unwrap();
        assert!(SpeechPlaybackLinks::new(
            tape.basis().links().linguistic_bases().clone(),
            BoundedSequence::try_from_iter([bad]).unwrap()
        )
        .is_err());
    }
}

pub fn preplay_replan<'a>(
    tape: &'a PreparedSpeechPlaybackTape<'a>,
    replacement: &'a PreparedSpeechPlaybackTape<'a>,
) {
    let domain = PlaybackRevisionDomain {
        subject: tape.source().utterance_id(),
    };
    let change = PreparedPlaybackChange::interpretation(None, tape).unwrap();
    let next = PreparedPlaybackChange::interpretation(Some(tape), replacement).unwrap();
    let withdrawal = PreparedPlaybackChange::withdrawal(replacement).unwrap();
    let proposal = event(
        &domain,
        1,
        "initial",
        RevisionChange::Proposed { delta: &change },
    );
    let stable = event(
        &domain,
        2,
        "stable",
        RevisionChange::Stable {
            revision: proposal.reference(),
            through: PlaybackEpochCursor::interpreted(),
        },
    );
    let revised = event(
        &domain,
        3,
        "revision",
        RevisionChange::Revised {
            replaces: proposal.reference(),
            delta: &next,
        },
    );
    let withdrawn = event(
        &domain,
        4,
        "withdrawal",
        RevisionChange::Withdrawn {
            revision: revised.reference(),
            delta: &withdrawal,
            reason: text("cancelled before any queue commitment"),
        },
    );
    let mut epoch = PlaybackRevisionJournal::new(
        &domain,
        context(),
        RevisionLimits {
            history_events: 16,
            revisable_units: 1,
        },
        back(tape),
    )
    .unwrap();
    epoch.append(&proposal, None).unwrap();
    epoch.append(&stable, None).unwrap();
    epoch.append(&revised, Some(back(replacement))).unwrap();
    assert!(epoch.requires_replan());
    assert!(epoch.journal().frontiers().stable_through.is_none());
    let old = graph::plan(tape);
    let old_gear = old.fragments[0]
        .placements
        .iter()
        .find(|gear| gear.kind_id.as_str() == contract::KIND)
        .unwrap();
    assert!(epoch.rebind_plan::<1>(old_gear).is_err());
    let new = graph::plan(replacement);
    let new_gear = new.fragments[0]
        .placements
        .iter()
        .find(|gear| gear.kind_id.as_str() == contract::KIND)
        .unwrap();
    epoch.rebind_plan::<1>(new_gear).unwrap();
    assert!(!epoch.requires_replan());
    assert_eq!(epoch.producer().unwrap().queued_frames(), 0);
    epoch.append(&withdrawn, None).unwrap();
    assert!(epoch.journal().current_proposal().is_none());
    assert_eq!(
        epoch.journal().frontiers().committed.acknowledgement(),
        None
    );
}
