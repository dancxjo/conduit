use super::*;

#[test]
fn finite_region_scope_order_and_prior_identity_refuse_atomically() {
    let domain = Tracking;
    let first = delta(0, 2, 1);
    let too_far = delta(2, 5, 2);
    let next = delta(2, 3, 2);
    let proposed = event(&domain, 1, "r1", RevisionChange::Proposed { delta: &first });
    let wide = event(
        &domain,
        2,
        "wide",
        RevisionChange::Revised {
            replaces: proposed.reference(),
            delta: &too_far,
        },
    );
    let wrong_prior = event(
        &domain,
        2,
        "wrong-prior",
        RevisionChange::Revised {
            replaces: reference(1, "different-r1"),
            delta: &next,
        },
    );
    let wrong_sequence = event(
        &domain,
        3,
        "gap",
        RevisionChange::Revised {
            replaces: proposed.reference(),
            delta: &next,
        },
    );
    let duplicate = event(
        &domain,
        2,
        "r1",
        RevisionChange::Revised {
            replaces: proposed.reference(),
            delta: &next,
        },
    );
    let mut wrong_context = context();
    wrong_context.policy = text("different-policy@1");
    let foreign = RevisionEvent::new(
        &domain,
        RevisionReference {
            context: wrong_context,
            sequence: 2,
            event: text("foreign"),
        },
        &EVIDENCE,
        RevisionChange::Revised {
            replaces: proposed.reference(),
            delta: &next,
        },
    )
    .unwrap();
    let mut journal = RevisionJournal::new(&domain, context(), Frame(0), limits()).unwrap();
    journal.append(&proposed).unwrap();
    let before = journal.frontiers();
    for (item, error) in [
        (&wide, RevisionRefusal::RevisableRegionBound),
        (&wrong_prior, RevisionRefusal::StaleRevision),
        (&wrong_sequence, RevisionRefusal::Sequence),
        (&duplicate, RevisionRefusal::DuplicateIdentity),
        (&foreign, RevisionRefusal::Scope),
    ] {
        assert_eq!(journal.append(item), Err(error));
        assert_eq!(journal.frontiers(), before);
        assert_eq!(journal.history().count(), 1);
    }
    let accepted = event(
        &domain,
        2,
        "r2",
        RevisionChange::Revised {
            replaces: proposed.reference(),
            delta: &next,
        },
    );
    journal.append(&accepted).unwrap();
    assert_eq!(journal.current_proposal(), Some(accepted.reference()));
    assert_eq!(
        journal.truncate_prefix(2),
        Err(RevisionRefusal::TruncationWouldEraseCurrentRevision)
    );
}

#[test]
fn invalid_domain_values_evidence_and_limits_are_admission_failures() {
    let domain = Tracking;
    assert!(RevisionText::new("").is_err());
    assert!(RevisionText::new(&"x".repeat(MAX_REVISION_TEXT_BYTES + 1)).is_err());
    let malformed = delta(2, 1, 1);
    assert!(matches!(
        RevisionEvent::new(
            &domain,
            reference(1, "r1"),
            &EVIDENCE,
            RevisionChange::Proposed { delta: &malformed }
        ),
        Err(RevisionRefusal::Domain)
    ));
    let too_large = delta(0, 33, 1);
    assert!(matches!(
        RevisionEvent::new(
            &domain,
            reference(1, "r1"),
            &EVIDENCE,
            RevisionChange::Proposed { delta: &too_large }
        ),
        Err(RevisionRefusal::Domain)
    ));
    let first = delta(0, 1, 1);
    assert!(matches!(
        RevisionEvent::new(
            &domain,
            reference(1, "r1"),
            &[],
            RevisionChange::Proposed { delta: &first }
        ),
        Err(RevisionRefusal::EvidenceBound)
    ));
    let duplicates = [EVIDENCE[0], EVIDENCE[0]];
    assert!(matches!(
        RevisionEvent::new(
            &domain,
            reference(1, "r1"),
            &duplicates,
            RevisionChange::Proposed { delta: &first }
        ),
        Err(RevisionRefusal::DuplicateEvidence)
    ));
    for count in [0, MAX_REVISION_HISTORY + 1] {
        assert!(matches!(
            RevisionJournal::new(
                &domain,
                context(),
                Frame(0),
                RevisionLimits {
                    history_events: count,
                    ..limits()
                }
            ),
            Err(RevisionRefusal::Limits)
        ));
    }
}

#[test]
fn corrections_require_exact_retained_commit_and_cannot_cross_its_frontier() {
    let domain = Tracking;
    let first = delta(0, 3, 1);
    let inside = delta(0, 1, 2);
    let outside = delta(1, 3, 2);
    let proposed = event(&domain, 1, "r1", RevisionChange::Proposed { delta: &first });
    let commit = event(
        &domain,
        2,
        "commit/r1",
        RevisionChange::Committed {
            revision: proposed.reference(),
            through: Frame(2),
        },
    );
    let unknown = event(
        &domain,
        3,
        "unknown",
        RevisionChange::Corrected {
            commit: reference(2, "other-commit"),
            delta: &inside,
            reason: text("evidence"),
        },
    );
    let not_commit = event(
        &domain,
        3,
        "not-commit",
        RevisionChange::Corrected {
            commit: proposed.reference(),
            delta: &inside,
            reason: text("evidence"),
        },
    );
    let crossing = event(
        &domain,
        3,
        "crossing",
        RevisionChange::Corrected {
            commit: commit.reference(),
            delta: &outside,
            reason: text("evidence"),
        },
    );
    let backward = event(
        &domain,
        3,
        "backward",
        RevisionChange::Committed {
            revision: proposed.reference(),
            through: Frame(1),
        },
    );
    let mut journal = RevisionJournal::new(&domain, context(), Frame(0), limits()).unwrap();
    journal.append(&proposed).unwrap();
    journal.append(&commit).unwrap();
    assert_eq!(
        journal.append(&unknown),
        Err(RevisionRefusal::UnknownCommit)
    );
    assert_eq!(
        journal.append(&not_commit),
        Err(RevisionRefusal::UnknownCommit)
    );
    assert_eq!(
        journal.append(&crossing),
        Err(RevisionRefusal::CorrectionOutsideCommit)
    );
    assert_eq!(journal.append(&backward), Err(RevisionRefusal::Frontier));
    assert_eq!(journal.history().count(), 2);
    assert_eq!(journal.frontiers().committed, Frame(2));
}

#[test]
fn independent_pipeline_stages_do_not_share_commitment_or_delivery() {
    let domain = Tracking;
    let first = delta(0, 2, 1);
    let proposed = event(&domain, 1, "r1", RevisionChange::Proposed { delta: &first });
    let commit = event(
        &domain,
        2,
        "commit/r1",
        RevisionChange::Committed {
            revision: proposed.reference(),
            through: Frame(2),
        },
    );
    let mut interpretation = RevisionJournal::new(&domain, context(), Frame(0), limits()).unwrap();
    let mut intent_context = context();
    intent_context.stream = text("control/intent-stage");
    let intent = RevisionJournal::new(&domain, intent_context, Frame(0), limits()).unwrap();
    interpretation.append(&proposed).unwrap();
    interpretation.append(&commit).unwrap();
    assert_eq!(interpretation.frontiers().committed, Frame(2));
    assert_eq!(intent.frontiers().committed, Frame(0));
    assert!(!interpretation.is_closed(), "commitment is not closure");
    // No device receipt, Host Call or effect capability is created by either journal.
}
