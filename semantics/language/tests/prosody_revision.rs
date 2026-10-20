#[path = "common/committed_prosody.rs"]
mod fixture;
use conduit_core::revision::*;
use conduit_language::{
    analysis_session::AnalysisSession,
    dependency_commitment::PreparedCommittedDependency,
    lexical::prepare_lexical_tape,
    prosody::prepare_fallback_prosody,
    prosody_commitment::*,
    prosody_revision::*,
    revision::TokenFrontier,
    revision::{DependencyDelta, DependencyRevisions},
    LanguageLexicalCandidate, LanguageLexicalEntry, LanguageLexicalPos, LanguageLexicalProfile,
    LanguageParserAdmittedBeam, LanguageParserBeamCommitRequest, LanguageParserHypothesis,
};
use conduit_plot::rust_binding::BoundedSequence;
fn text(value: &str) -> RevisionText<'_> {
    RevisionText::new(value).unwrap()
}
#[test]
fn current_prosody_uses_shared_ledger_and_retains_corrections_after_commit() {
    let execution = fixture::Prepared::new();
    let f = fixture::prosody::fixture("Hello Travis", 1, 0);
    let request = fixture::request(&f, 2, false, false);
    let result = fixture::result(&execution, &request);
    let rich = PreparedCommittedProsody::prepare(
        &f.lexical,
        1,
        f.discourse.fact(),
        &fixture::symbolic(&f.profile),
        &request,
        &result,
    )
    .unwrap();
    let fallback = prepare_fallback_prosody(&f.lexical, 1, &f.profile).unwrap();
    let rich_delta = ProsodyPlan::CommittedDiscourse(&rich);
    let fallback_delta = ProsodyPlan::HeuristicFallback(&fallback);
    let domain = ProsodyRevisions::new(f.lexical.tape().source(), 2).unwrap();
    let context = RevisionContext {
        stream: text("prosody"),
        subject: text("utterance"),
        epoch: text("r1"),
        producer: text("checked-profile"),
        policy: text("prosody/1"),
    };
    let reference = |sequence, label| RevisionReference {
        context,
        sequence,
        event: text(label),
    };
    let evidence = [RevisionEvidence {
        source: text("supplied-graph-fixture"),
        generation: text("r1"),
    }];
    let proposed = RevisionEvent::new(
        &domain,
        reference(1, "rich"),
        &evidence,
        RevisionChange::Proposed { delta: &rich_delta },
    )
    .unwrap();
    let revised = RevisionEvent::new(
        &domain,
        reference(2, "heuristic"),
        &evidence,
        RevisionChange::Revised {
            replaces: proposed.reference(),
            delta: &fallback_delta,
        },
    )
    .unwrap();
    let stable = RevisionEvent::new(
        &domain,
        reference(3, "stable"),
        &evidence,
        RevisionChange::Stable {
            revision: revised.reference(),
            through: TokenFrontier(2),
        },
    )
    .unwrap();
    let commit = RevisionEvent::new(
        &domain,
        reference(4, "commit"),
        &evidence,
        RevisionChange::Committed {
            revision: revised.reference(),
            through: TokenFrontier(2),
        },
    )
    .unwrap();
    let rewrite = RevisionEvent::new(
        &domain,
        reference(5, "rewrite"),
        &evidence,
        RevisionChange::Revised {
            replaces: revised.reference(),
            delta: &rich_delta,
        },
    )
    .unwrap();
    let correction = RevisionEvent::new(
        &domain,
        reference(5, "correction"),
        &evidence,
        RevisionChange::Corrected {
            commit: commit.reference(),
            delta: &rich_delta,
            reason: text("later-analysis"),
        },
    )
    .unwrap();
    let analysis_domain = DependencyRevisions {
        source_text: f.lexical.tape().source().material(),
        tokens: 2,
    };
    let analysis_delta = DependencyDelta::Assert(f.discourse.fact().basis().clone());
    let analysis_proposal = RevisionEvent::new(
        &analysis_domain,
        reference(1, "analysis"),
        &evidence,
        RevisionChange::Proposed {
            delta: &analysis_delta,
        },
    )
    .unwrap();
    let analysis_stable = RevisionEvent::new(
        &analysis_domain,
        reference(2, "analysis-stable"),
        &evidence,
        RevisionChange::Stable {
            revision: analysis_proposal.reference(),
            through: TokenFrontier(2),
        },
    )
    .unwrap();
    let analysis_commit = RevisionEvent::new(
        &analysis_domain,
        reference(3, "analysis-commit"),
        &evidence,
        RevisionChange::Committed {
            revision: analysis_proposal.reference(),
            through: TokenFrontier(2),
        },
    )
    .unwrap();
    let mut analysis = AnalysisSession::new(
        &analysis_domain,
        context,
        RevisionLimits {
            history_events: 16,
            revisable_units: 4,
        },
    )
    .unwrap();
    analysis
        .append(&analysis_proposal, None, false, false)
        .unwrap();
    analysis
        .append(&analysis_stable, None, false, false)
        .unwrap();
    assert!(analysis
        .append(&analysis_commit, None, false, false)
        .is_err());
    analysis
        .append(&analysis_commit, Some(rich.dependency()), false, false)
        .unwrap();
    assert!(analysis
        .current(analysis_commit.reference(), rich.dependency())
        .is_err());
    // A different actual replay can retain the same arc/source/frontier while
    // changing the proposing hypothesis identity. It is not this session's
    // committed receipt and cannot be swapped in as current authority.
    let beam = request.beam();
    let alternate_beam = LanguageParserAdmittedBeam::new(
        beam.basis().clone(),
        LanguageParserHypothesis::new(
            true,
            999,
            *beam.candidate0().score(),
            beam.candidate0().state().clone(),
        )
        .unwrap(),
        beam.candidate1().clone(),
        beam.candidate2().clone(),
        beam.candidate3().clone(),
    )
    .unwrap();
    let alternate_request = LanguageParserBeamCommitRequest::new(
        alternate_beam,
        false,
        request.frontier().clone(),
        false,
        *request.through(),
    )
    .unwrap();
    let alternate_result = fixture::result(&execution, &alternate_request);
    let alternate = PreparedCommittedProsody::prepare(
        &f.lexical,
        1,
        f.discourse.fact(),
        &fixture::symbolic(&f.profile),
        &alternate_request,
        &alternate_result,
    )
    .unwrap();
    assert_eq!(alternate.dependency().arc(), rich.dependency().arc());
    assert_eq!(
        alternate.commitment().result().frontier(),
        rich.commitment().result().frontier()
    );
    assert!(analysis
        .current(analysis_proposal.reference(), alternate.dependency())
        .is_err());
    // The dependent Native query does not carry the governor's alternatives.
    // Equal source/arc/admission must not launder a different lexical snapshot.
    let prior_profile = f.lexical.tape().profile();
    let governor_entry = LanguageLexicalEntry::new(
        BoundedSequence::try_from_iter([LanguageLexicalCandidate::new(
            "hello".into(),
            BoundedSequence::new(),
            LanguageLexicalPos::Noun,
        )
        .unwrap()])
        .unwrap(),
        "Hello".into(),
    )
    .unwrap();
    let changed_profile = LanguageLexicalProfile::new(
        BoundedSequence::try_from_iter(
            prior_profile
                .entries()
                .iter()
                .cloned()
                .chain([governor_entry]),
        )
        .unwrap(),
        prior_profile.identity().clone(),
        prior_profile.language().clone(),
        prior_profile.provenance().clone(),
    )
    .unwrap();
    let changed_lexical =
        prepare_lexical_tape(f.lexical.tape().source(), &changed_profile, None).unwrap();
    let changed_receipt = PreparedCommittedDependency::prepare(
        &changed_lexical,
        f.discourse.fact().basis(),
        &request,
        &result,
    )
    .unwrap();
    assert_eq!(changed_receipt.admission(), rich.dependency().admission());
    assert_ne!(changed_receipt.lexical(), rich.dependency().lexical());
    assert!(analysis
        .current(analysis_proposal.reference(), &changed_receipt)
        .is_err());
    let authority = analysis
        .current(analysis_proposal.reference(), rich.dependency())
        .unwrap();
    let mut session = ProsodySession::new(
        &domain,
        context,
        RevisionLimits {
            history_events: 16,
            revisable_units: 4,
        },
    )
    .unwrap();
    let rich_stable = RevisionEvent::new(
        &domain,
        reference(2, "rich-stable"),
        &evidence,
        RevisionChange::Stable {
            revision: proposed.reference(),
            through: TokenFrontier(2),
        },
    )
    .unwrap();
    let rich_commit = RevisionEvent::new(
        &domain,
        reference(3, "rich-commit"),
        &evidence,
        RevisionChange::Committed {
            revision: proposed.reference(),
            through: TokenFrontier(2),
        },
    )
    .unwrap();
    let rich_correction = RevisionEvent::new(
        &domain,
        reference(4, "rich-correction"),
        &evidence,
        RevisionChange::Corrected {
            commit: rich_commit.reference(),
            delta: &fallback_delta,
            reason: text("later-analysis"),
        },
    )
    .unwrap();
    let changed_rich = PreparedCommittedProsody::prepare(
        &changed_lexical,
        1,
        f.discourse.fact(),
        &fixture::symbolic(&f.profile),
        &request,
        &result,
    )
    .unwrap();
    assert_eq!(
        changed_rich.dependency().admission(),
        rich.dependency().admission()
    );
    let changed_delta = ProsodyPlan::CommittedDiscourse(&changed_rich);
    let changed_proposed = RevisionEvent::new(
        &domain,
        reference(1, "changed-governor-profile"),
        &evidence,
        RevisionChange::Proposed {
            delta: &changed_delta,
        },
    )
    .unwrap();
    let mut rich_session = ProsodySession::new(
        &domain,
        context,
        RevisionLimits {
            history_events: 16,
            revisable_units: 4,
        },
    )
    .unwrap();
    let unchanged_frontiers = rich_session.frontiers();
    assert!(rich_session
        .append_current(&changed_proposed, &authority, false, false)
        .is_err());
    assert_eq!(rich_session.history().count(), 0);
    assert_eq!(rich_session.frontiers(), unchanged_frontiers);
    rich_session
        .append_current(&proposed, &authority, false, false)
        .unwrap();
    assert!(rich_session.append(&rich_stable, false, false).is_err());
    rich_session
        .append_current(&rich_stable, &authority, false, false)
        .unwrap();
    assert!(rich_session.append(&rich_commit, false, false).is_err());
    rich_session
        .append_current(&rich_commit, &authority, false, false)
        .unwrap();
    let rich_frontier = rich_session.frontiers();
    rich_session.append(&rich_correction, false, false).unwrap();
    assert_eq!(rich_session.frontiers().committed, rich_frontier.committed);
    assert_eq!(
        rich_session.frontiers().observed_through,
        rich_frontier.observed_through
    );
    // A correction preserves committed output, but invalidates prior stability.
    assert_eq!(rich_session.frontiers().stable_through, None);
    rich_session.require_current(proposed.reference()).unwrap();
    assert!(session.append(&proposed, false, true).is_err());
    assert!(session.append(&proposed, true, false).is_err());
    assert_eq!(session.history().count(), 0);
    assert!(session.append(&proposed, false, false).is_err());
    session
        .append_current(&proposed, &authority, false, false)
        .unwrap();
    session.require_current(proposed.reference()).unwrap();
    session.append(&revised, false, false).unwrap();
    assert!(session.require_current(proposed.reference()).is_err());
    assert!(session.append(&commit, false, false).is_err());
    session.append(&stable, false, false).unwrap();
    session.append(&commit, false, false).unwrap();
    assert!(session.append(&rewrite, false, false).is_err());
    let frontier = session.frontiers();
    session.append(&correction, false, false).unwrap();
    assert_eq!(session.frontiers().committed, frontier.committed);
    assert_eq!(
        session.frontiers().observed_through,
        frontier.observed_through
    );
    assert_eq!(session.frontiers().stable_through, None);
    assert_eq!(session.history().count(), 5);
    // Correction evidence cannot replace the retained current committed plan.
    session.require_current(revised.reference()).unwrap();
    assert!(matches!(
        session.history().last().unwrap().change(),
        RevisionChange::Corrected { .. }
    ));
}

#[test]
fn successive_token_plans_preserve_committed_intervals_and_correction_history() {
    let f = fixture::prosody::fixture("Hello Travis", 1, 0);
    let first = prepare_fallback_prosody(&f.lexical, 0, &f.profile).unwrap();
    let second = prepare_fallback_prosody(&f.lexical, 1, &f.profile).unwrap();
    let first_delta = ProsodyPlan::HeuristicFallback(&first);
    let second_delta = ProsodyPlan::HeuristicFallback(&second);
    let domain = ProsodyRevisions::new(f.lexical.tape().source(), 2).unwrap();
    let context = RevisionContext {
        stream: text("streaming-prosody"),
        subject: text("utterance"),
        epoch: text("r1"),
        producer: text("heuristic-profile"),
        policy: text("prosody/1"),
    };
    let reference = |sequence, label| RevisionReference {
        context,
        sequence,
        event: text(label),
    };
    let evidence = [RevisionEvidence {
        source: text("explicit-heuristic-fixture"),
        generation: text("r1"),
    }];
    let p0 = RevisionEvent::new(
        &domain,
        reference(1, "token0"),
        &evidence,
        RevisionChange::Proposed {
            delta: &first_delta,
        },
    )
    .unwrap();
    let s0 = RevisionEvent::new(
        &domain,
        reference(2, "stable0"),
        &evidence,
        RevisionChange::Stable {
            revision: p0.reference(),
            through: TokenFrontier(1),
        },
    )
    .unwrap();
    let c0 = RevisionEvent::new(
        &domain,
        reference(3, "commit0"),
        &evidence,
        RevisionChange::Committed {
            revision: p0.reference(),
            through: TokenFrontier(1),
        },
    )
    .unwrap();
    let correction0 = RevisionEvent::new(
        &domain,
        reference(4, "correction0"),
        &evidence,
        RevisionChange::Corrected {
            commit: c0.reference(),
            delta: &first_delta,
            reason: text("later-analysis"),
        },
    )
    .unwrap();
    let p1 = RevisionEvent::new(
        &domain,
        reference(5, "token1"),
        &evidence,
        RevisionChange::Revised {
            replaces: p0.reference(),
            delta: &second_delta,
        },
    )
    .unwrap();
    let s1 = RevisionEvent::new(
        &domain,
        reference(6, "stable1"),
        &evidence,
        RevisionChange::Stable {
            revision: p1.reference(),
            through: TokenFrontier(2),
        },
    )
    .unwrap();
    let c1 = RevisionEvent::new(
        &domain,
        reference(7, "commit1"),
        &evidence,
        RevisionChange::Committed {
            revision: p1.reference(),
            through: TokenFrontier(2),
        },
    )
    .unwrap();
    let rewrite0 = RevisionEvent::new(
        &domain,
        reference(8, "rewrite0"),
        &evidence,
        RevisionChange::Revised {
            replaces: p1.reference(),
            delta: &first_delta,
        },
    )
    .unwrap();
    let mut session = ProsodySession::new(
        &domain,
        context,
        RevisionLimits {
            history_events: 16,
            revisable_units: 4,
        },
    )
    .unwrap();
    for event in [&p0, &s0, &c0, &correction0, &p1, &s1, &c1] {
        session.append(event, false, false).unwrap();
    }
    assert_eq!(session.frontiers().committed, TokenFrontier(2));
    assert!(session.append(&rewrite0, false, false).is_err());
    assert_eq!(session.history().count(), 7);
    assert!(session.history().any(|e| matches!(e.change(),RevisionChange::Corrected { commit,.. } if *commit == c0.reference())));
    session.require_current(p1.reference()).unwrap();
}
