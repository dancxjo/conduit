//! Garden-path consumer seam for #4907; supplied arcs are not parser evidence.
use conduit_core::revision::*;
use conduit_language::{revision::*, *};
fn text(value: &str) -> RevisionText<'_> {
    RevisionText::new(value).unwrap()
}
fn arc(revision: &str, relation: LanguageUniversalDependencyRelation) -> DependencyDelta {
    arc_between(revision, relation, 1, 2)
}
fn arc_between(
    revision: &str,
    relation: LanguageUniversalDependencyRelation,
    dependent: u64,
    governor: u64,
) -> DependencyDelta {
    let token = |ordinal| {
        LanguageAnalysisTokenRef::new(
            LanguageAnalysisRevisionId::new(revision.into()).unwrap(),
            LinguisticTokenIdentity::new(
                ordinal,
                LanguageTextId::new("utterance/3".into()).unwrap(),
                LanguageTextRevisionId::new("source/3".into()).unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
    };
    let head = token(governor);
    DependencyDelta::Assert(
        LanguageDependencyArc::new(
            token(dependent),
            LanguageDependencyHead::token(head.revision().clone(), head.token().clone()).unwrap(),
            LanguageDependencyRelation::new(relation, None).unwrap(),
        )
        .unwrap(),
    )
}
fn view<'a>(
    events: &[&'a RevisionEvent<'a, DependencyRevisions<'a>>],
) -> Option<&'a LanguageDependencyArc> {
    let mut current = None;
    for event in events {
        let delta = match event.change() {
            RevisionChange::Proposed { delta }
            | RevisionChange::Revised { delta, .. }
            | RevisionChange::Corrected { delta, .. }
            | RevisionChange::Withdrawn { delta, .. } => delta,
            _ => continue,
        };
        current = match delta {
            DependencyDelta::Assert(arc) => Some(arc),
            DependencyDelta::Withdraw(_) => None,
        };
    }
    current
}
#[test]
fn garden_path_revises_stabilizes_commits_and_corrects_without_erasure() {
    let source = LanguageText::new(
        LanguageTextId::new("utterance/3".into()).unwrap(),
        LanguageId::new("language/english".into()).unwrap(),
        LanguageTextRevisionId::new("source/3".into()).unwrap(),
        "The old man the boats".into(),
    )
    .unwrap();
    let domain = DependencyRevisions {
        source_text: &source,
        tokens: 5,
    };
    let context = RevisionContext {
        stream: text("analysis/session/1"),
        subject: text("token/old"),
        epoch: text("utterance/3"),
        producer: text("parser/fixture@1"),
        policy: text("language/tail-window@1"),
    };
    let evidence = [RevisionEvidence {
        source: text("utterance"),
        generation: text("utterance/3"),
    }];
    let reference = |sequence, label| RevisionReference {
        context,
        sequence,
        event: text(label),
    };
    let early = arc("analysis/1", LanguageUniversalDependencyRelation::amod());
    let later = arc("analysis/2", LanguageUniversalDependencyRelation::nsubj());
    let repair = arc(
        "analysis/3",
        LanguageUniversalDependencyRelation::vocative(),
    );
    let r1 = RevisionEvent::new(
        &domain,
        reference(1, "r1"),
        &evidence,
        RevisionChange::Proposed { delta: &early },
    )
    .unwrap();
    let r2 = RevisionEvent::new(
        &domain,
        reference(2, "r2"),
        &evidence,
        RevisionChange::Revised {
            replaces: r1.reference(),
            delta: &later,
        },
    )
    .unwrap();
    let stable = RevisionEvent::new(
        &domain,
        reference(3, "stable"),
        &evidence,
        RevisionChange::Stable {
            revision: r2.reference(),
            through: TokenFrontier(2),
        },
    )
    .unwrap();
    let committed = RevisionEvent::new(
        &domain,
        reference(4, "commit"),
        &evidence,
        RevisionChange::Committed {
            revision: r2.reference(),
            through: TokenFrontier(2),
        },
    )
    .unwrap();
    let ordinary = RevisionEvent::new(
        &domain,
        reference(5, "ordinary"),
        &evidence,
        RevisionChange::Revised {
            replaces: r2.reference(),
            delta: &repair,
        },
    )
    .unwrap();
    let correction = RevisionEvent::new(
        &domain,
        reference(5, "repair"),
        &evidence,
        RevisionChange::Corrected {
            commit: committed.reference(),
            delta: &repair,
            reason: text("later external reanalysis"),
        },
    )
    .unwrap();
    let closed = RevisionEvent::new(
        &domain,
        reference(6, "closed"),
        &evidence,
        RevisionChange::Closed,
    )
    .unwrap();
    let limits = RevisionLimits {
        history_events: 8,
        revisable_units: 4,
    };
    let mut journal = RevisionJournal::new(&domain, context, TokenFrontier(1), limits).unwrap();
    for event in [&r1, &r2, &stable, &committed] {
        journal.append(event).unwrap();
    }
    assert_eq!(
        journal.append(&ordinary),
        Err(RevisionRefusal::CommittedHistoryRequiresCorrection)
    );
    journal.append(&correction).unwrap();
    assert_eq!(journal.frontiers().stable_through, None);
    assert_eq!(journal.frontiers().committed, TokenFrontier(2));
    journal.append(&closed).unwrap();
    let history: Vec<_> = journal.history().collect();
    let replay = RevisionJournal::replay(
        &domain,
        context,
        TokenFrontier(1),
        limits,
        &history,
        journal.truncated_events(),
    )
    .unwrap();
    assert_eq!(journal.frontiers(), replay.frontiers());
    assert_eq!(journal.current_proposal(), replay.current_proposal());
    assert!(replay.is_closed());
    assert_eq!(view(&history), view(&replay.history().collect::<Vec<_>>()));
    assert!(
        matches!(history[1].change(), RevisionChange::Revised { delta: DependencyDelta::Assert(arc), .. } if arc.dependent().revision() == &LanguageAnalysisRevisionId::new("analysis/2".into()).unwrap())
    );
    assert_eq!(
        journal.truncate_prefix(1),
        Err(RevisionRefusal::TruncationWouldEraseCommittedHistory)
    );
    let wrong = DependencyDelta::Withdraw(
        LinguisticTokenIdentity::new(
            1,
            LanguageTextId::new("other/text".into()).unwrap(),
            LanguageTextRevisionId::new("source/3".into()).unwrap(),
        )
        .unwrap(),
    );
    assert!(!domain.validate_delta(RevisionDeltaRole::Withdrawal, &wrong));
}

#[test]
fn dependency_observes_governor_without_marking_it_as_changed() {
    let source = LanguageText::new(
        LanguageTextId::new("utterance/3".into()).unwrap(),
        LanguageId::new("language/english".into()).unwrap(),
        LanguageTextRevisionId::new("source/3".into()).unwrap(),
        "a record today".into(),
    )
    .unwrap();
    let domain = DependencyRevisions {
        source_text: &source,
        tokens: 3,
    };
    let forward = arc_between("analysis/a", LanguageUniversalDependencyRelation::Det, 0, 1);
    let backward = arc_between("analysis/a", LanguageUniversalDependencyRelation::Dep, 2, 0);
    assert_eq!(
        domain.region(&forward),
        (TokenFrontier(0), TokenFrontier(2))
    );
    assert_eq!(
        domain.region(&backward),
        (TokenFrontier(2), TokenFrontier(3))
    );
    let context = RevisionContext {
        stream: text("analysis"),
        subject: text("dependency"),
        epoch: text("source/3"),
        producer: text("supplied-graph"),
        policy: text("bounded-dependency@1"),
    };
    let evidence = [RevisionEvidence {
        source: text("fixture"),
        generation: text("1"),
    }];
    let reference = |sequence, event| RevisionReference {
        context,
        sequence,
        event: text(event),
    };
    let proposed = RevisionEvent::new(
        &domain,
        reference(1, "forward"),
        &evidence,
        RevisionChange::Proposed { delta: &forward },
    )
    .unwrap();
    let stable = RevisionEvent::new(
        &domain,
        reference(2, "stable"),
        &evidence,
        RevisionChange::Stable {
            revision: proposed.reference(),
            through: TokenFrontier(2),
        },
    )
    .unwrap();
    let mut journal = RevisionJournal::new(
        &domain,
        context,
        TokenFrontier(0),
        RevisionLimits {
            history_events: 4,
            revisable_units: 2,
        },
    )
    .unwrap();
    journal.append(&proposed).unwrap();
    assert_eq!(journal.frontiers().observed_through, TokenFrontier(2));
    assert_eq!(journal.frontiers().committed, TokenFrontier(0));
    journal.append(&stable).unwrap();
    assert_eq!(journal.frontiers().committed, TokenFrontier(0));
    let mut tight = RevisionJournal::new(
        &domain,
        context,
        TokenFrontier(0),
        RevisionLimits {
            history_events: 4,
            revisable_units: 1,
        },
    )
    .unwrap();
    assert_eq!(
        tight.append(&proposed),
        Err(RevisionRefusal::RevisableRegionBound)
    );
    assert_eq!(tight.history().count(), 0);
    assert_eq!(tight.frontiers().observed_through, TokenFrontier(0));
    let next = RevisionEvent::new(
        &domain,
        reference(1, "backward"),
        &evidence,
        RevisionChange::Proposed { delta: &backward },
    )
    .unwrap();
    let mut protected = RevisionJournal::new(
        &domain,
        context,
        TokenFrontier(2),
        RevisionLimits {
            history_events: 4,
            revisable_units: 1,
        },
    )
    .unwrap();
    assert_eq!(
        protected.append(&proposed),
        Err(RevisionRefusal::CommittedHistoryRequiresCorrection)
    );
    protected.append(&next).unwrap();
    assert_eq!(protected.frontiers().observed_through, TokenFrontier(3));
    assert_eq!(protected.frontiers().committed, TokenFrontier(2));
}
