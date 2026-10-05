//! Lifecycle and correlation proof over frame cursors, never text offsets.
#[path = "revision/common.rs"]
mod common;
use common::*;
use conduit_core::revision::*;

#[test]
fn tracking_revises_stability_commits_and_corrects_without_rewriting_history() {
    let domain = Tracking;
    let first = delta(0, 2, 1);
    let revised = delta(0, 2, 2);
    let correction = delta(0, 1, 3);
    let proposal = event(&domain, 1, "r1", RevisionChange::Proposed { delta: &first });
    let stable = event(
        &domain,
        2,
        "stable/r1",
        RevisionChange::Stable {
            revision: proposal.reference(),
            through: Frame(2),
        },
    );
    let revision = event(
        &domain,
        3,
        "r2",
        RevisionChange::Revised {
            replaces: proposal.reference(),
            delta: &revised,
        },
    );
    let commit = event(
        &domain,
        4,
        "commit/r2",
        RevisionChange::Committed {
            revision: revision.reference(),
            through: Frame(2),
        },
    );
    let behind = event(
        &domain,
        5,
        "forbidden-rewrite",
        RevisionChange::Revised {
            replaces: revision.reference(),
            delta: &correction,
        },
    );
    let corrected = event(
        &domain,
        5,
        "correction/r2",
        RevisionChange::Corrected {
            commit: commit.reference(),
            delta: &correction,
            reason: text("later-observation"),
        },
    );
    let closed = event(&domain, 6, "closed", RevisionChange::Closed);
    let late = event(
        &domain,
        7,
        "late-correction",
        RevisionChange::Corrected {
            commit: commit.reference(),
            delta: &correction,
            reason: text("human-review"),
        },
    );
    let after_close = event(
        &domain,
        7,
        "late-revision",
        RevisionChange::Revised {
            replaces: revision.reference(),
            delta: &revised,
        },
    );
    let mut journal = RevisionJournal::new(&domain, context(), Frame(0), limits()).unwrap();
    journal.append(&proposal).unwrap();
    journal.append(&stable).unwrap();
    assert_eq!(
        journal.frontiers().committed,
        Frame(0),
        "stability never commits"
    );
    journal.append(&revision).unwrap();
    assert_eq!(
        journal.frontiers().stable_through,
        None,
        "stability can be explicitly revised"
    );
    journal.append(&commit).unwrap();
    assert_eq!(
        journal.append(&behind),
        Err(RevisionRefusal::CommittedHistoryRequiresCorrection)
    );
    assert_eq!(journal.history().count(), 4);
    journal.append(&corrected).unwrap();
    journal.append(&closed).unwrap();
    assert_eq!(journal.append(&after_close), Err(RevisionRefusal::Closed));
    journal.append(&late).unwrap();
    assert!(
        journal.is_closed(),
        "a correction does not reopen ordinary revision"
    );
    assert_eq!(journal.frontiers().committed, Frame(2));
    let history: Vec<_> = journal.history().collect();
    assert!(
        matches!(history[2].change(), RevisionChange::Revised { delta, .. } if delta.class == Some(2))
    );
    assert_eq!(history[3].reference(), commit.reference());
    assert_eq!(reduce(&history)[..2], [Some(3), Some(2)]);
    let replay = RevisionJournal::replay(&domain, context(), Frame(0), limits(), &history).unwrap();
    assert_eq!(replay.frontiers(), journal.frontiers());
    assert_eq!(replay.current_proposal(), journal.current_proposal());
    assert_eq!(replay.is_closed(), journal.is_closed());
    assert_eq!(
        reduce(&replay.history().collect::<Vec<_>>()),
        reduce(&history)
    );
}

#[test]
fn withdrawal_truncation_and_history_pressure_are_explicit() {
    let domain = Tracking;
    let first = delta(0, 1, 1);
    let removed = TrackDelta {
        first: Frame(0),
        through: Frame(1),
        class: None,
    };
    let next = delta(0, 1, 2);
    let proposed = event(&domain, 1, "r1", RevisionChange::Proposed { delta: &first });
    let withdrawn = event(
        &domain,
        2,
        "withdraw/r1",
        RevisionChange::Withdrawn {
            revision: proposed.reference(),
            delta: &removed,
            reason: text("occluded"),
        },
    );
    let fresh = event(&domain, 3, "r2", RevisionChange::Proposed { delta: &next });
    let committed = event(
        &domain,
        4,
        "commit/r2",
        RevisionChange::Committed {
            revision: fresh.reference(),
            through: Frame(1),
        },
    );
    let closed = event(&domain, 5, "closed", RevisionChange::Closed);
    let small = RevisionLimits {
        history_events: 4,
        ..limits()
    };
    let mut journal = RevisionJournal::new(&domain, context(), Frame(0), small).unwrap();
    for item in [&proposed, &withdrawn, &fresh, &committed] {
        journal.append(item).unwrap();
    }
    assert_eq!(journal.append(&closed), Err(RevisionRefusal::HistoryFull));
    assert_eq!(journal.truncated_events(), 0);
    assert_eq!(
        journal.truncate_prefix(2),
        Err(RevisionRefusal::TruncationWouldEraseCommittedHistory)
    );
    let mut journal = RevisionJournal::new(&domain, context(), Frame(0), small).unwrap();
    for item in [&proposed, &withdrawn, &fresh] {
        journal.append(item).unwrap();
    }
    assert_eq!(
        journal.truncate_prefix(3),
        Err(RevisionRefusal::TruncationWouldEraseCurrentRevision)
    );
    journal.truncate_prefix(2).unwrap();
    assert_eq!(journal.truncated_events(), 2);
    assert_eq!(journal.history().count(), 1);
    journal.append(&committed).unwrap();
    journal.append(&closed).unwrap();
    let tail: Vec<_> = journal.history().collect();
    assert!(matches!(
        RevisionJournal::replay(&domain, context(), Frame(0), small, &tail),
        Err(RevisionRefusal::TruncatedHistory)
    ));
    assert_eq!(
        journal.truncate_prefix(2),
        Err(RevisionRefusal::TruncationWouldEraseCurrentRevision)
    );
    let mut fully_committed = RevisionJournal::new(&domain, context(), Frame(0), small).unwrap();
    let initial_commit = event(
        &domain,
        2,
        "commit/r1",
        RevisionChange::Committed {
            revision: proposed.reference(),
            through: Frame(1),
        },
    );
    let initial_withdrawal = event(
        &domain,
        3,
        "withdraw-committed",
        RevisionChange::Withdrawn {
            revision: proposed.reference(),
            delta: &removed,
            reason: text("erase"),
        },
    );
    fully_committed.append(&proposed).unwrap();
    fully_committed.append(&initial_commit).unwrap();
    assert_eq!(
        fully_committed.append(&initial_withdrawal),
        Err(RevisionRefusal::CommittedHistoryRequiresCorrection)
    );
    assert_eq!(
        fully_committed.truncate_prefix(2),
        Err(RevisionRefusal::TruncationWouldEraseCurrentRevision)
    );
}

#[path = "revision/admission.rs"]
mod admission;
