#![cfg(feature = "semantic-bindings")]
use conduit_core::revision::*;
use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::{revision::*, semantic::*};
fn text(value: &str) -> RevisionText<'_> {
    RevisionText::new(value).unwrap()
}
// Domain reduction follows explicit scalar replacement ranges, never a diff.
fn view(events: &[&RevisionEvent<'_, ListeningRevisions<'_>>]) -> String {
    let mut current = String::new();
    for event in events {
        let delta = match event.change() {
            RevisionChange::Proposed { delta }
            | RevisionChange::Revised { delta, .. }
            | RevisionChange::Corrected { delta, .. }
            | RevisionChange::Withdrawn { delta, .. } => delta,
            _ => continue,
        };
        match delta {
            ListeningDelta::Partial(value) => current.clone_from(value.text()),
            ListeningDelta::Replacement(value) | ListeningDelta::Correction(value) => {
                let mut chars: Vec<_> = current.chars().collect();
                chars.splice(
                    *value.replaces().start() as usize..*value.replaces().end() as usize,
                    value.text().chars(),
                );
                current = chars.into_iter().collect();
            }
            ListeningDelta::Withdrawal { range, .. } => {
                let mut chars: Vec<_> = current.chars().collect();
                chars.drain(*range.start() as usize..*range.end() as usize);
                current = chars.into_iter().collect();
            }
        }
        assert!(current.len() <= 4096);
    }
    current
}
#[test]
fn native_asr_keeps_scalar_ranges_committed_text_and_delivery_separate() {
    let segment = ListeningSegmentId::new("segment/1".into()).unwrap();
    let domain = ListeningRevisions { segment: &segment };
    let context = RevisionContext {
        stream: text("listening/session/1"),
        subject: text("segment/1"),
        epoch: text("history/1"),
        producer: text("asr/back@1"),
        policy: text("asr/scalar-window@1"),
    };
    let evidence = [RevisionEvidence {
        source: text("audio/1"),
        generation: text("audio/digest/17"),
    }];
    let reference = |sequence, label| RevisionReference {
        context,
        sequence,
        event: text(label),
    };
    let partial = ListeningDelta::Partial(
        AsrPartialHypothesis::new(
            None,
            ListeningTextRole::Recognition,
            segment.clone(),
            "café?".into(),
        )
        .unwrap(),
    );
    let replacement = ListeningDelta::Replacement(
        AsrRevisedHypothesis::new(
            None,
            LanguageTextRange::new(5, 4).unwrap(),
            ListeningTextRole::Recognition,
            segment.clone(),
            "!".into(),
        )
        .unwrap(),
    );
    let corrected = ListeningDelta::Correction(
        AsrRevisedHypothesis::new(
            None,
            LanguageTextRange::new(4, 3).unwrap(),
            ListeningTextRole::Recognition,
            segment.clone(),
            "e".into(),
        )
        .unwrap(),
    );
    let r1 = RevisionEvent::new(
        &domain,
        reference(1, "partial"),
        &evidence,
        RevisionChange::Proposed { delta: &partial },
    )
    .unwrap();
    let r2 = RevisionEvent::new(
        &domain,
        reference(2, "replace"),
        &evidence,
        RevisionChange::Revised {
            replaces: r1.reference(),
            delta: &replacement,
        },
    )
    .unwrap();
    let native_commit = AsrCommittedSegment::new(
        None,
        None,
        ListeningTextRole::Recognition,
        segment.clone(),
        None,
        "café!".into(),
        BoundedSequence::new(),
    )
    .unwrap();
    let current = view(&[&r1, &r2]);
    assert_eq!(current, "café!");
    let through = domain.committed_frontier(&native_commit, &current).unwrap();
    assert_eq!(through, ScalarFrontier(5)); // Six bytes, five scalars.
    assert_eq!(
        domain.committed_frontier(&native_commit, "silently changed"),
        Err(RevisionRefusal::Domain)
    );
    let commit = RevisionEvent::new(
        &domain,
        reference(3, "commit"),
        &evidence,
        RevisionChange::Committed {
            revision: r2.reference(),
            through,
        },
    )
    .unwrap();
    // This replacement fits within the committed high-water range, but would
    // shift all later scalar positions if admitted as a correction.
    let shifts_epoch = ListeningDelta::Correction(
        AsrRevisedHypothesis::new(
            None,
            LanguageTextRange::new(2, 1).unwrap(),
            ListeningTextRole::Recognition,
            segment.clone(),
            "two".into(),
        )
        .unwrap(),
    );
    assert!(matches!(
        RevisionEvent::new(
            &domain,
            reference(4, "shifts-epoch"),
            &evidence,
            RevisionChange::Corrected {
                commit: commit.reference(),
                delta: &shifts_epoch,
                reason: text("length-changing reanalysis")
            }
        ),
        Err(RevisionRefusal::Domain)
    ));
    let correction = RevisionEvent::new(
        &domain,
        reference(4, "correction"),
        &evidence,
        RevisionChange::Corrected {
            commit: commit.reference(),
            delta: &corrected,
            reason: text("late acoustic evidence"),
        },
    )
    .unwrap();
    let limits = RevisionLimits {
        history_events: 8,
        revisable_units: 8,
    };
    let history = [&r1, &r2, &commit, &correction];
    let journal =
        RevisionJournal::replay(&domain, context, ScalarFrontier(0), limits, &history, 0).unwrap();
    let replay =
        RevisionJournal::replay(&domain, context, ScalarFrontier(0), limits, &history, 0).unwrap();
    assert_eq!(journal.frontiers(), replay.frontiers());
    assert_eq!(view(&journal.history().collect::<Vec<_>>()), "cafe!");
    assert_eq!(
        view(&journal.history().collect::<Vec<_>>()),
        view(&replay.history().collect::<Vec<_>>())
    );
    assert_eq!(native_commit.text(), "café!");
    assert!(!journal.is_closed()); // Commitment is not closure or played audio.
    assert!(!domain.validate_delta(RevisionDeltaRole::Revision, &corrected));
}

#[test]
fn listening_mapping_refuses_wrong_subject_role_and_unbounded_scalar_ranges() {
    let segment = ListeningSegmentId::new("segment/1".into()).unwrap();
    let other = ListeningSegmentId::new("segment/2".into()).unwrap();
    let domain = ListeningRevisions { segment: &segment };
    let wrong_subject = ListeningDelta::Partial(
        AsrPartialHypothesis::new(None, ListeningTextRole::Recognition, other, "hi".into())
            .unwrap(),
    );
    let wrong_role = ListeningDelta::Partial(
        AsrPartialHypothesis::new(
            None,
            ListeningTextRole::Generation,
            segment.clone(),
            "hi".into(),
        )
        .unwrap(),
    );
    assert!(!domain.validate_delta(RevisionDeltaRole::Proposal, &wrong_subject));
    assert!(!domain.validate_delta(RevisionDeltaRole::Proposal, &wrong_role));
    let replacement = ListeningDelta::Replacement(
        AsrRevisedHypothesis::new(
            None,
            LanguageTextRange::new(4097, 4096).unwrap(),
            ListeningTextRole::Recognition,
            segment.clone(),
            "x".into(),
        )
        .unwrap(),
    );
    assert!(!domain.validate_cursor(domain.region(&replacement).1));
}
