//! Explicit played acknowledgements remain distinct from rendered scheduler PCM.
use super::lifecycle;
use conduit_core::revision::*;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    ValueRef,
};
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_speech::{
    native_playback_contract as contract, playback_basis::*, playback_revision::*, semantic::*,
};
pub fn commitment<'a>(tape: &'a PreparedSpeechPlaybackTape<'a>) -> serde_json::Value {
    let domain = PlaybackRevisionDomain {
        subject: tape.source().utterance_id(),
    };
    let proposed = PreparedPlaybackChange::interpretation(None, tape).unwrap();
    let proposal = lifecycle::event(
        &domain,
        1,
        "learned/proposal",
        RevisionChange::Proposed { delta: &proposed },
    );
    let stable = lifecycle::event(
        &domain,
        2,
        "learned/stable",
        RevisionChange::Stable {
            revision: proposal.reference(),
            through: PlaybackEpochCursor::interpreted(),
        },
    );
    let premature = lifecycle::event(
        &domain,
        3,
        "learned/premature",
        RevisionChange::Committed {
            revision: proposal.reference(),
            through: PlaybackEpochCursor::interpreted(),
        },
    );
    let mut journal = PlaybackRevisionJournal::new(
        &domain,
        lifecycle::context(),
        RevisionLimits {
            history_events: 8,
            revisable_units: 1,
        },
        lifecycle::back(tape),
    )
    .unwrap();
    journal.append(&proposal, None).unwrap();
    journal.append(&stable, None).unwrap();
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
    let mut ready = StepIo::test_frame(
        [Some(reference)],
        [false],
        [Some(contract::MAXIMUM_PCM_BYTES as u32)],
        None,
        contract::REQUIRED_STEP_FUEL,
    );
    assert_eq!(journal.step(&mut ready, &bytes), StepOutcome::Progress);
    let played = lifecycle::ack(tape, SpeechPlaybackDisposition::Played, 0, 128);
    assert!(journal.acknowledge(&played).is_err());
    StepBack::<1>::step_committed(&mut journal);
    let queued = lifecycle::ack(tape, SpeechPlaybackDisposition::Queued, 0, 128);
    assert!(journal.acknowledge(&queued).unwrap().is_none());
    assert_eq!(journal.producer().unwrap().played_frames(), 0);
    let played_receipt = played.clone().encode().unwrap();
    let queued_receipt = queued.clone().encode().unwrap();
    let played = journal.acknowledge(&played).unwrap().unwrap();
    let committed = lifecycle::event(
        &domain,
        3,
        "learned/commit",
        RevisionChange::Committed {
            revision: proposal.reference(),
            through: PlaybackEpochCursor::from_played(played),
        },
    );
    journal.append(&committed, None).unwrap();
    let withdrawn = PreparedPlaybackChange::withdrawal(tape).unwrap();
    let withdrawal = lifecycle::event(
        &domain,
        4,
        "learned/withdraw-after-played",
        RevisionChange::Withdrawn {
            revision: proposal.reference(),
            delta: &withdrawn,
            reason: RevisionText::new("later withdrawal").unwrap(),
        },
    );
    assert_eq!(
        journal.append(&withdrawal, None),
        Err(PlaybackRevisionRefusal::QueuedHistory)
    );
    assert!(journal
        .journal()
        .frontiers()
        .committed
        .acknowledgement()
        .is_some());
    assert_eq!(journal.producer().unwrap().played_frames(), 128);
    serde_json::json!({"queued_acknowledgement_bytes":queued_receipt,"played_acknowledgement_bytes":played_receipt,"stable_does_not_commit":true,"withdraw_after_played_refused":true,"premature_commit_refused":true,"queued_frames":128,"played_frames":128,"played_provenance":"manual-effect-owner-fixture"})
}
