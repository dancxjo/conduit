//! Generic session publication with actual Source-replayed, independently
//! prepared receipts; no installed epoch owner or fallback controller is used.
// The shared fixture also supplies prosody helpers, unused by this direct test.
#[allow(dead_code)]
#[path = "common/committed_prosody.rs"]
mod fixture;
#[path = "common/analysis_session_probe.rs"]
mod probe;
use conduit_core::revision::*;
use conduit_language::{
    analysis_session::{AnalysisSession, AnalysisSessionRefusal},
    dependency_commitment::PreparedCommittedDependency,
    lexical::prepare_lexical_tape,
    revision::{DependencyDelta, DependencyRevisions, TokenFrontier},
    *,
};
use conduit_plot::rust_binding::BoundedSequence;

fn text(value: &str) -> RevisionText<'_> {
    RevisionText::new(value).unwrap()
}

#[test]
fn distinct_snapshots_survive_receipt_drop_and_atomic_publication_refusals() {
    let execution = fixture::Prepared::new();
    let f = fixture::prosody::fixture("Hello Travis", 1, 0);
    let request0 = fixture::request(&f, 1, false, false);
    let result0 = fixture::result(&execution, &request0);
    let token0 = f
        .lexical
        .tape()
        .tokens()
        .first()
        .unwrap()
        .identity()
        .clone();
    let root = LanguageDependencyArc::new(
        LanguageAnalysisTokenRef::new(f.discourse.fact().analysis().clone(), token0).unwrap(),
        LanguageDependencyHead::Root,
        LanguageDependencyRelation::new(LanguageUniversalDependencyRelation::Root, None).unwrap(),
    )
    .unwrap();
    let receipt0 =
        PreparedCommittedDependency::prepare(&f.lexical, &root, &request0, &result0).unwrap();
    let request1 = LanguageParserBeamCommitRequest::new(
        request0.beam().clone(),
        false,
        LanguageParserRawFrontier::new(request0.beam().basis().clone(), 1, 2).unwrap(),
        false,
        2,
    )
    .unwrap();
    let result1 = fixture::result(&execution, &request1);
    let receipt1 = PreparedCommittedDependency::prepare(
        &f.lexical,
        f.discourse.fact().basis(),
        &request1,
        &result1,
    )
    .unwrap();
    assert_ne!(receipt0.admission(), receipt1.admission());
    assert_ne!(receipt0.arc(), receipt1.arc());
    assert_eq!(*receipt0.commitment().result().frontier().committed(), 1);
    assert_eq!(*receipt1.commitment().result().frontier().committed(), 2);
    let equal_receipt1 =
        PreparedCommittedDependency::prepare(&f.lexical, receipt1.arc(), &request1, &result1)
            .unwrap();
    assert_eq!(equal_receipt1.admission(), receipt1.admission());
    assert_eq!(equal_receipt1.lexical(), receipt1.lexical());
    // Same token identities but different actual Source material cannot publish.
    let foreign = fixture::prosody::fixture("Hello Taylor", 1, 0);
    let foreign_receipt =
        PreparedCommittedDependency::prepare(&foreign.lexical, receipt1.arc(), &request1, &result1)
            .unwrap();
    assert_ne!(
        foreign_receipt.admission().query().source().material(),
        f.lexical.tape().source().material()
    );

    // A different actual replay with the same edge/frontier is not full equality.
    let beam = request1.beam();
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
        request1.frontier().clone(),
        false,
        2,
    )
    .unwrap();
    let alternate_result = fixture::result(&execution, &alternate_request);
    let alternate = PreparedCommittedDependency::prepare(
        &f.lexical,
        receipt1.arc(),
        &alternate_request,
        &alternate_result,
    )
    .unwrap();
    assert_eq!(alternate.arc(), receipt1.arc());
    assert_ne!(alternate.admission(), receipt1.admission());

    // Change only the governor's lexical alternatives, absent from the query.
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
    let changed =
        PreparedCommittedDependency::prepare(&changed_lexical, receipt1.arc(), &request1, &result1)
            .unwrap();
    assert_eq!(changed.admission(), receipt1.admission());
    assert_ne!(changed.lexical(), receipt1.lexical());

    let domain = DependencyRevisions {
        source_text: f.lexical.tape().source().material(),
        tokens: 2,
    };
    let context = RevisionContext {
        stream: text("snapshots"),
        subject: text("utterance"),
        epoch: text("r1"),
        producer: text("source-replay"),
        policy: text("session/1"),
    };
    let reference = |sequence, label| RevisionReference {
        context,
        sequence,
        event: text(label),
    };
    let evidence = [RevisionEvidence {
        source: text("checked-source-fixture"),
        generation: text("r1"),
    }];
    let delta0 = DependencyDelta::Assert(root);
    let delta1 = DependencyDelta::Assert(receipt1.arc().clone());
    let p0 = RevisionEvent::new(
        &domain,
        reference(1, "root"),
        &evidence,
        RevisionChange::Proposed { delta: &delta0 },
    )
    .unwrap();
    let s0 = RevisionEvent::new(
        &domain,
        reference(2, "stable-root"),
        &evidence,
        RevisionChange::Stable {
            revision: p0.reference(),
            through: TokenFrontier(1),
        },
    )
    .unwrap();
    let c0 = RevisionEvent::new(
        &domain,
        reference(3, "commit-root"),
        &evidence,
        RevisionChange::Committed {
            revision: p0.reference(),
            through: TokenFrontier(1),
        },
    )
    .unwrap();
    let p1 = RevisionEvent::new(
        &domain,
        reference(4, "vocative"),
        &evidence,
        RevisionChange::Revised {
            replaces: p0.reference(),
            delta: &delta1,
        },
    )
    .unwrap();
    let s1 = RevisionEvent::new(
        &domain,
        reference(5, "stable-vocative"),
        &evidence,
        RevisionChange::Stable {
            revision: p1.reference(),
            through: TokenFrontier(2),
        },
    )
    .unwrap();
    let c1 = RevisionEvent::new(
        &domain,
        reference(6, "commit-vocative"),
        &evidence,
        RevisionChange::Committed {
            revision: p1.reference(),
            through: TokenFrontier(2),
        },
    )
    .unwrap();
    let bad_sequence = RevisionEvent::new(
        &domain,
        reference(99, "wrong-sequence"),
        &evidence,
        RevisionChange::Committed {
            revision: p1.reference(),
            through: TokenFrontier(2),
        },
    )
    .unwrap();
    let correction = RevisionEvent::new(
        &domain,
        reference(7, "correction"),
        &evidence,
        RevisionChange::Corrected {
            commit: c1.reference(),
            delta: &delta1,
            reason: text("later-evidence"),
        },
    )
    .unwrap();
    let disputed_commit = RevisionEvent::new(
        &domain,
        reference(8, "disputed-commit"),
        &evidence,
        RevisionChange::Committed {
            revision: p1.reference(),
            through: TokenFrontier(2),
        },
    )
    .unwrap();
    let limits = RevisionLimits {
        history_events: 8,
        revisable_units: 2,
    };
    let mut session = AnalysisSession::new(&domain, context, limits).unwrap();
    session.append(&p0, None, false, false).unwrap();
    session.append(&s0, None, false, false).unwrap();

    // Every negative checks the whole observable journal and Current outcome.
    macro_rules! unchanged_refusal {
        ($session:expr, $event:expr, $receipt:expr, $cancelled:expr, $pressure:expr, $pattern:pat, $reference:expr, $current_receipt:expr) => {{
            let before = $session.frontiers();
            let history = $session
                .history()
                .map(|event| event.reference())
                .collect::<Vec<_>>();
            let dispute = $session.publication_dispute();
            let current = $session.current($reference, $current_receipt).is_ok();
            probe::without_heap_activity(|| {
                assert!(matches!(
                    $session.append($event, $receipt, $cancelled, $pressure),
                    Err($pattern)
                ));
                assert_eq!($session.frontiers(), before);
                assert_eq!($session.publication_dispute(), dispute);
                assert!($session
                    .history()
                    .map(|event| event.reference())
                    .eq(history.iter().copied()));
                assert_eq!(
                    $session.current($reference, $current_receipt).is_ok(),
                    current
                );
            });
        }};
    }
    unchanged_refusal!(
        session,
        &c0,
        Some(&receipt0),
        true,
        false,
        AnalysisSessionRefusal::Cancelled,
        p0.reference(),
        &receipt0
    );
    unchanged_refusal!(
        session,
        &c0,
        Some(&receipt0),
        false,
        true,
        AnalysisSessionRefusal::Pressure,
        p0.reference(),
        &receipt0
    );
    unchanged_refusal!(
        session,
        &c0,
        None,
        false,
        false,
        AnalysisSessionRefusal::Commitment,
        p0.reference(),
        &receipt0
    );
    unchanged_refusal!(
        session,
        &c0,
        Some(&receipt1),
        false,
        false,
        AnalysisSessionRefusal::Commitment,
        p0.reference(),
        &receipt0
    );
    probe::without_heap_activity(|| {
        session.append(&c0, Some(&receipt0), false, false).unwrap();
        let current = session.current(p0.reference(), &receipt0).unwrap();
        assert!(core::ptr::eq(current.receipt(), &receipt0));
        assert_eq!(current.frontier(), TokenFrontier(1));
    });
    session.append(&p1, None, false, false).unwrap();
    session.append(&s1, None, false, false).unwrap();
    assert!(session.current(p0.reference(), &receipt0).is_err());
    unchanged_refusal!(
        session,
        &bad_sequence,
        Some(&receipt1),
        false,
        false,
        AnalysisSessionRefusal::Revision(RevisionRefusal::Sequence),
        p1.reference(),
        &receipt1
    );
    unchanged_refusal!(
        session,
        &c1,
        Some(&foreign_receipt),
        false,
        false,
        AnalysisSessionRefusal::Commitment,
        p1.reference(),
        &receipt1
    );
    // The first receipt owns no journal event. Its full destructor is outside
    // observation; only the session now retains its immutable snapshot share.
    drop(receipt0);
    probe::without_heap_activity(|| {
        session.append(&c1, Some(&receipt1), false, false).unwrap();
        let current = session.current(p1.reference(), &receipt1).unwrap();
        assert_eq!(current.frontier(), TokenFrontier(2));
        assert_eq!(current.receipt().admission(), receipt1.admission());
        assert_eq!(current.receipt().lexical(), receipt1.lexical());
        assert!(session.current(p1.reference(), &equal_receipt1).is_ok());
        assert!(session.current(p1.reference(), &foreign_receipt).is_err());
        assert!(session.current(p1.reference(), &alternate).is_err());
        assert!(session.current(p1.reference(), &changed).is_err());
    });
    session.append(&correction, None, false, false).unwrap();
    assert!(matches!(
        session.current(p1.reference(), &receipt1),
        Err(AnalysisSessionRefusal::Corrected)
    ));
    unchanged_refusal!(
        session,
        &disputed_commit,
        Some(&receipt1),
        false,
        false,
        AnalysisSessionRefusal::Corrected,
        p1.reference(),
        &receipt1
    );

    // Fill the admitted history with ordinary valid events. A valid commitment
    // must pass session checks then fail the journal's real HistoryFull bound.
    let mut full = AnalysisSession::new(
        &domain,
        context,
        RevisionLimits {
            history_events: 5,
            revisable_units: 2,
        },
    )
    .unwrap();
    // Independently prepare another receipt rather than aliasing the dropped one.
    let first_again = PreparedCommittedDependency::prepare(
        &f.lexical,
        match &delta0 {
            DependencyDelta::Assert(arc) => arc,
            _ => unreachable!(),
        },
        &request0,
        &result0,
    )
    .unwrap();
    for event in [&p0, &s0] {
        full.append(event, None, false, false).unwrap();
    }
    full.append(&c0, Some(&first_again), false, false).unwrap();
    for event in [&p1, &s1] {
        full.append(event, None, false, false).unwrap();
    }
    drop(first_again);
    unchanged_refusal!(
        full,
        &c1,
        Some(&receipt1),
        false,
        false,
        AnalysisSessionRefusal::Revision(RevisionRefusal::HistoryFull),
        p1.reference(),
        &receipt1
    );
    assert_eq!(full.frontiers().committed, TokenFrontier(1));
    assert_eq!(full.history().count(), 5);
    // All receipt, session, and fixture destruction remains outside observation.
}
