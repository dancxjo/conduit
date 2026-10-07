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
    playback_basis::*, playback_revision::*, semantic::*,
};
fn text(value: &str) -> RevisionText<'_> {
    RevisionText::new(value).unwrap()
}
#[test]
fn duplicate_producer_provenance_cannot_substitute_for_owned_played_assertion() {
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
    let tape = prepare_speech_playback_tape(
        &realized,
        &pitch,
        &[PlaybackLinguisticBinding {
            event: 0,
            admitted: &linguistic,
        }],
        7,
    )
    .unwrap();
    let plan = graph::plan(&tape);
    let gear = plan.fragments[0]
        .placements
        .iter()
        .find(|gear| gear.kind_id.as_str() == contract::KIND)
        .unwrap();
    let domain = PlaybackRevisionDomain {
        subject: fixture.source.utterance_id(),
    };
    let context = RevisionContext {
        stream: text("speech"),
        subject: text("utterance"),
        epoch: text("epoch/1"),
        producer: text("owner"),
        policy: text("played@1"),
    };
    let evidence = [RevisionEvidence {
        source: text("fixture"),
        generation: text("1"),
    }];
    let delta = PreparedPlaybackChange::interpretation(None, &tape).unwrap();
    let proposal = RevisionEvent::new(
        &domain,
        RevisionReference {
            context,
            sequence: 1,
            event: text("proposal"),
        },
        &evidence,
        RevisionChange::Proposed { delta: &delta },
    )
    .unwrap();
    let producer =
        NativeSpeechPlaybackBack::prepare::<1>(gear, &tape, PortId(0), PortId(0)).unwrap();
    let mut owned = PlaybackRevisionJournal::new(
        &domain,
        context,
        RevisionLimits {
            history_events: 8,
            revisable_units: 1,
        },
        producer,
    )
    .unwrap();
    owned.append(&proposal, None).unwrap();
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
    assert_eq!(owned.step(&mut ready, &bytes), StepOutcome::Progress);
    StepBack::<1>::step_committed(&mut owned);
    let acknowledgement = |note: &str| {
        SpeechPlaybackAcknowledgement::new(
            tape.basis().clone(),
            SpeechPlaybackDisposition::Played,
            0,
            SpeechEvidenceProvenance::new(note.into(), SpeechEvidenceSource::Manual, None).unwrap(),
            128,
        )
        .unwrap()
    };
    let owned_ack = acknowledgement("owned effect");
    let proof = owned.acknowledge(&owned_ack).unwrap().unwrap();
    let mut duplicate =
        NativeSpeechPlaybackBack::prepare::<1>(gear, &tape, PortId(0), PortId(0)).unwrap();
    let mut ready = StepIo::test_frame(
        [Some(reference)],
        [false],
        [Some(contract::MAXIMUM_PCM_BYTES as u32)],
        None,
        contract::REQUIRED_STEP_FUEL,
    );
    assert_eq!(duplicate.step(&mut ready, &bytes), StepOutcome::Progress);
    StepBack::<1>::step_committed(&mut duplicate);
    let foreign_ack = acknowledgement("different effect assertion");
    let foreign_proof = duplicate.acknowledge(&foreign_ack).unwrap().unwrap();
    let wrong = RevisionEvent::new(
        &domain,
        RevisionReference {
            context,
            sequence: 2,
            event: text("foreign-commit"),
        },
        &evidence,
        RevisionChange::Committed {
            revision: proposal.reference(),
            through: PlaybackEpochCursor::from_played(foreign_proof),
        },
    )
    .unwrap();
    let accepted = RevisionEvent::new(
        &domain,
        RevisionReference {
            context,
            sequence: 2,
            event: text("owned-commit"),
        },
        &evidence,
        RevisionChange::Committed {
            revision: proposal.reference(),
            through: PlaybackEpochCursor::from_played(proof),
        },
    )
    .unwrap();
    assert_eq!(
        owned.append(&wrong, None),
        Err(PlaybackRevisionRefusal::PlayedEvidence)
    );
    assert_eq!(owned.journal().history().count(), 1);
    owned.append(&accepted, None).unwrap();
    assert_eq!(
        owned.journal().frontiers().committed.acknowledgement(),
        Some(&owned_ack)
    );
}
