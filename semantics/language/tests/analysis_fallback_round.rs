//! Actual checked Source/Plan/Play, explicitly deterministic fallback only.
//! No model inference, learned accuracy, audio or installed async-ingress claim.
#[path = "common/parser_revision_fixture.rs"]
mod fixture;
#[path = "common/analysis_epoch_runtime.rs"]
mod runtime;
#[path = "common/analysis_session_probe.rs"]
mod session_probe;
use conduit_core::revision::*;
use conduit_language::{
    analysis_epoch_initialization::*,
    analysis_epoch_owner::{fallback_round::*, *},
    dependency_commitment::PreparedCommittedDependency,
    lexical::prepare_lexical_tape,
    parser_rebase_replay::*,
    revision::*,
    *,
};
use conduit_plot::rust_binding::NativeRustBinding;
fn text(s: &str) -> RevisionText<'_> {
    RevisionText::new(s).unwrap()
}
fn snapshot(
    owner: &AnalysisEpochOwner<'_>,
) -> (Vec<u8>, u64, RevisionFrontiers<TokenFrontier>, usize) {
    (
        owner.beam().clone().encode().unwrap(),
        owner.generation(),
        owner.frontiers(),
        owner.history().count(),
    )
}
#[test]
fn actual_fallback_rounds_commit_then_handoff_without_reopening_protected_prefix() {
    let mut runtime = runtime::Runtime::new();
    run_scenario(&mut runtime, false);
    run_scenario(&mut runtime, true);
}
// Borrowed event slots must precede the owner so retained history drops first.
#[allow(clippy::needless_late_init)]
fn run_scenario(runtime: &mut runtime::Runtime, dispute: bool) {
    let old = fixture::revision("answer ", None, 7);
    let next = fixture::revision("answer record ", Some(&old), 7);
    let old_domain = DependencyRevisions {
        source_text: old.material(),
        tokens: 1,
    };
    let next_domain = DependencyRevisions {
        source_text: next.material(),
        tokens: 2,
    };
    let context = RevisionContext {
        stream: text("analysis"),
        subject: text("utterance"),
        epoch: text("1"),
        producer: text("fallback-fixture"),
        policy: text("complete-round-zero-score@1"),
    };
    let reference = |sequence, label| RevisionReference {
        context,
        sequence,
        event: text(label),
    };
    let evidence = [RevisionEvidence {
        source: text("actual-Source-Plan-Play-fallback"),
        generation: text("1"),
    }];
    // Caller-owned stable slots precede the owner; no self-referential arena.
    let delta;
    let proposed;
    let cancelled_stable;
    let pressure_stable;
    let stable;
    let commit;
    let receipt;
    let protected_delta;
    let protected_proposal;
    let correction;
    let closed;
    let corrected_delta;
    let recommit;
    let mut direct;
    let same_epoch_correction;
    let lexical = prepare_lexical_tape(&old, &fixture::profile(), None).unwrap();
    let basis = LanguageParserBasis::new(
        LanguageAnalysisRevisionId::new("analysis/1".into()).unwrap(),
        old.material().revision().clone(),
        old.material().identity().clone(),
    )
    .unwrap();
    let begin = LanguageParserBegin::new(basis.clone(), fixture::dep(), 1).unwrap();
    let observed = LanguageParserNumericState::decode(&runtime.execute(
        "language-parser-initialize",
        &begin.clone().encode().unwrap(),
    ))
    .unwrap();
    let initial = PreparedAnalysisInitializationProgram::prepare()
        .unwrap()
        .replay(lexical, &begin, &observed)
        .unwrap();
    assert_eq!(runtime.source_id.as_str(), initial.source_id());
    let actual_envelope = runtime.execute(
        "language/analysis-initial-envelope",
        &observed.encode().unwrap(),
    );
    assert_eq!(actual_envelope, initial.envelope_output_canonical());
    let limits = RevisionLimits {
        history_events: 16,
        revisable_units: 4,
    };
    let mut owner = AnalysisEpochOwner::initialize(initial, &old_domain, context, limits).unwrap();
    let programs = PreparedFallbackRoundPrograms::prepare().unwrap();
    let mut old_stale_round = None;
    for round in 0..8 {
        let prepared = match owner.prepare_fallback_round_with(&programs, runtime) {
            Ok(p) => p,
            Err(f) => panic!("fallback round {round}: {:?}", f.refusal),
        };
        assert_eq!(
            prepared.profile(),
            "language/complete-round/beam4/zero-score-fallback@1"
        );
        assert!(!prepared.observations().is_empty());
        // One duplicate semantic replay creates a stale value without repeating
        // any actual Source/Plan execution or injecting a replacement beam.
        let stale = if round == 0 {
            Some(
                match owner.prepare_fallback_round(&programs, prepared.observations().to_vec()) {
                    Ok(p) => p,
                    Err(f) => panic!("same exact observed round: {:?}", f.refusal),
                },
            )
        } else {
            None
        };
        match owner.apply_fallback_round(prepared) {
            Ok(()) => (),
            Err(f) => panic!("round apply: {:?}", f.refusal),
        }
        if let Some(stale) = stale {
            let before = snapshot(&owner);
            match owner.apply_fallback_round(stale) {
                Err(f) => {
                    assert!(matches!(f.refusal, FallbackRoundRefusal::Stale));
                    old_stale_round = Some(f.prepared);
                }
                Ok(()) => panic!("stale preparation applied"),
            }
            assert_eq!(snapshot(&owner), before);
        }
    }
    assert_eq!(owner.fallback_rounds().len(), 8);
    // The expected one-token root is a consumer assertion, never an injected
    // owner graph: actual commitment below must replay and correlate it.
    let token = owner.lexical().tape().tokens()[0].identity().clone();
    let arc = LanguageDependencyArc::new(
        LanguageAnalysisTokenRef::new(basis.analysis_revision().clone(), token).unwrap(),
        LanguageDependencyHead::Root,
        LanguageDependencyRelation::new(LanguageUniversalDependencyRelation::Root, None).unwrap(),
    )
    .unwrap();
    delta = DependencyDelta::Assert(arc.clone());
    proposed = RevisionEvent::new(
        &old_domain,
        reference(1, "proposal"),
        &evidence,
        RevisionChange::Proposed { delta: &delta },
    )
    .unwrap();
    owner.append_event(&proposed, false, false).unwrap();
    let prepared = match owner.prepare_fallback_frontier_with(&programs, runtime, true, false) {
        Ok(p) => p,
        Err(f) => panic!("refusal frontier replay: {:?}", f.refusal),
    };
    assert_eq!(*prepared.frontier().stable(), 0);
    let before = snapshot(&owner);
    cancelled_stable = RevisionEvent::new(
        &old_domain,
        reference(2, "cancelled-stable"),
        &evidence,
        RevisionChange::Stable {
            revision: proposed.reference(),
            through: TokenFrontier(0),
        },
    )
    .unwrap();
    assert!(matches!(
        owner.append_fallback_frontier(&cancelled_stable, prepared),
        Err(frontier::FrontierApplyFailure {
            refusal: AnalysisEpochRefusal::Session(
                conduit_language::analysis_session::AnalysisSessionRefusal::Cancelled
            ),
            ..
        })
    ));
    assert_eq!(snapshot(&owner), before);
    let prepared = match owner.prepare_fallback_frontier_with(&programs, runtime, false, true) {
        Ok(p) => p,
        Err(f) => panic!("refusal frontier replay: {:?}", f.refusal),
    };
    assert_eq!(*prepared.frontier().stable(), 0);
    let before = snapshot(&owner);
    pressure_stable = RevisionEvent::new(
        &old_domain,
        reference(2, "pressure-stable"),
        &evidence,
        RevisionChange::Stable {
            revision: proposed.reference(),
            through: TokenFrontier(0),
        },
    )
    .unwrap();
    assert!(matches!(
        owner.append_fallback_frontier(&pressure_stable, prepared),
        Err(frontier::FrontierApplyFailure {
            refusal: AnalysisEpochRefusal::Session(
                conduit_language::analysis_session::AnalysisSessionRefusal::Pressure
            ),
            ..
        })
    ));
    assert_eq!(snapshot(&owner), before);
    let prepared = match owner.prepare_fallback_frontier_with(&programs, runtime, false, false) {
        Ok(p) => p,
        Err(f) => panic!("actual all-survivor frontier: {:?}", f.refusal),
    };
    assert_eq!(
        *prepared.frontier().stable(),
        1,
        "actual surviving Source graphs must agree"
    );
    let frontier = prepared.frontier().clone();
    stable = RevisionEvent::new(
        &old_domain,
        reference(2, "stable"),
        &evidence,
        RevisionChange::Stable {
            revision: proposed.reference(),
            through: TokenFrontier(1),
        },
    )
    .unwrap();
    match owner.append_fallback_frontier(&stable, prepared) {
        Ok(()) => (),
        Err(f) => panic!("stable admission: {:?}", f.refusal),
    }
    let b = owner.beam();
    let beam = LanguageParserAdmittedBeam::new(
        basis,
        b.candidate0().parser().clone(),
        b.candidate1().parser().clone(),
        b.candidate2().parser().clone(),
        b.candidate3().parser().clone(),
    )
    .unwrap();
    let request = LanguageParserBeamCommitRequest::new(beam, false, frontier, false, 1).unwrap();
    let actual = LanguageParserBeamCommitResult::decode(&runtime.execute(
        "language-parser-beam-commit",
        &request.clone().encode().unwrap(),
    ))
    .unwrap();
    assert!(*actual.accepted());
    receipt =
        PreparedCommittedDependency::prepare(owner.lexical(), &arc, &request, &actual).unwrap();
    commit = RevisionEvent::new(
        &old_domain,
        reference(3, "commit"),
        &evidence,
        RevisionChange::Committed {
            revision: proposed.reference(),
            through: TokenFrontier(1),
        },
    )
    .unwrap();
    let choices: [_; 4] = [
        owner.beam().candidate0(),
        owner.beam().candidate1(),
        owner.beam().candidate2(),
        owner.beam().candidate3(),
    ]
    .map(|s| *s.choices());
    owner
        .append_commitment(&commit, &receipt, false, false)
        .unwrap();
    assert_eq!(owner.frontiers().committed, TokenFrontier(1));
    assert_eq!(
        owner
            .current(proposed.reference(), &receipt)
            .unwrap()
            .frontier(),
        TokenFrontier(1)
    );
    for (slot, choice) in [
        owner.beam().candidate0(),
        owner.beam().candidate1(),
        owner.beam().candidate2(),
        owner.beam().candidate3(),
    ]
    .into_iter()
    .zip(choices)
    {
        assert_eq!(slot.choices(), &choice);
        if *slot.parser().active() {
            assert_eq!(*slot.parser().state().committed(), 1)
        }
    }
    let previous = owner.beam().clone();
    let rebase = PreparedParserRebaseProgram::prepare().unwrap();
    let mut replays = Vec::new();
    for prior in [
        previous.candidate0(),
        previous.candidate1(),
        previous.candidate2(),
        previous.candidate3(),
    ] {
        let p = PreparedParserRevisionRebase::prepare(
            prior.parser().state(),
            owner.lexical(),
            &next,
            LanguageAnalysisRevisionId::new("analysis/2".into()).unwrap(),
            fixture::dep(),
        )
        .unwrap();
        let raw = LanguageParserNumericState::decode(&runtime.execute(
            "language-parser-revision-rebase",
            &p.request().clone().encode().unwrap(),
        ))
        .unwrap();
        replays.push(rebase.replay(p, &raw).unwrap());
    }
    let next_context = RevisionContext {
        epoch: text("2"),
        ..context
    };
    if dispute {
        use conduit_language::analysis_session::{AnalysisSession, AnalysisSessionRefusal};
        // Supplied conflicting evidence is retained, not promoted to a checked
        // replacement parse or allowed to rewrite the old committed receipt.
        corrected_delta = DependencyDelta::Assert(
            LanguageDependencyArc::new(
                arc.dependent().clone(),
                arc.governor().clone(),
                LanguageDependencyRelation::new(
                    LanguageUniversalDependencyRelation::Root,
                    Some(LanguageDependencySubtype::new("contested".into()).unwrap()),
                )
                .unwrap(),
            )
            .unwrap(),
        );
        same_epoch_correction = RevisionEvent::new(
            &old_domain,
            reference(4, "correction"),
            &evidence,
            RevisionChange::Corrected {
                commit: commit.reference(),
                delta: &corrected_delta,
                reason: text("later-analysis"),
            },
        )
        .unwrap();
        let before = snapshot(&owner);
        for (cancelled, pressure) in [(true, false), (false, true)] {
            assert!(owner
                .append_event(&same_epoch_correction, cancelled, pressure)
                .is_err());
            assert_eq!(snapshot(&owner), before);
            owner.current(proposed.reference(), &receipt).unwrap();
        }
        owner
            .append_event(&same_epoch_correction, false, false)
            .unwrap();
        assert!(matches!(
            owner.current(proposed.reference(), &receipt),
            Err(AnalysisSessionRefusal::Corrected)
        ));
        assert_eq!(owner.frontiers().committed, TokenFrontier(1));
        assert_eq!(owner.frontiers().stable_through, None);
        assert_eq!(owner.beam(), &previous);
        assert_eq!(owner.history().count(), 4);
        assert_eq!(owner.commitment_receipts().len(), 1);

        // A new commitment cannot refill a disputed session's publication
        // cache, even when the old exact Source receipt remains inspectable.
        direct = AnalysisSession::new(&old_domain, context, limits).unwrap();
        direct.append(&proposed, None, false, false).unwrap();
        direct.append(&stable, None, false, false).unwrap();
        session_probe::append_and_current(&mut direct, &commit, proposed.reference(), &receipt);
        direct
            .append(&same_epoch_correction, None, false, false)
            .unwrap();
        recommit = RevisionEvent::new(
            &old_domain,
            reference(5, "recommit"),
            &evidence,
            RevisionChange::Committed {
                revision: proposed.reference(),
                through: TokenFrontier(1),
            },
        )
        .unwrap();
        assert!(matches!(
            direct.append(&recommit, Some(&receipt), false, false),
            Err(AnalysisSessionRefusal::Corrected)
        ));
        assert_eq!(direct.history().count(), 4);
        assert_eq!(
            direct.publication_dispute(),
            Some(same_epoch_correction.reference())
        );
        let before = snapshot(&owner);
        let failure = match owner.handoff(
            replays.try_into().ok().unwrap(),
            &next_domain,
            next_context,
            limits,
        ) {
            Err(f) => f,
            Ok(_) => panic!("disputed analysis handed off"),
        };
        assert!(matches!(
            failure.refusal,
            AnalysisEpochRefusal::Session(AnalysisSessionRefusal::Corrected)
        ));
        assert_eq!(snapshot(&failure.owner), before);
        assert_eq!(failure.replays.len(), 4);
        runtime.assert_signs();
        return;
    }
    let (mut next_owner, archive) = match owner.handoff(
        replays.try_into().ok().unwrap(),
        &next_domain,
        next_context,
        limits,
    ) {
        Ok(value) => value,
        Err(f) => panic!("committed handoff: {:?}", f.refusal),
    };
    assert_eq!(archive.beam(), &previous);
    assert_eq!(archive.commitment_receipts().len(), 1);
    assert_eq!(archive.frontiers().committed, TokenFrontier(1));
    assert_eq!(next_owner.frontiers().committed, TokenFrontier(1));
    assert_eq!(next_owner.history().count(), 0);
    assert!(next_owner.commitment_receipts().is_empty());
    assert!(next_owner.current(proposed.reference(), &receipt).is_err());
    let new_token = next_owner.lexical().tape().tokens()[0].identity().clone();
    let new_arc = LanguageDependencyArc::new(
        LanguageAnalysisTokenRef::new(
            LanguageAnalysisRevisionId::new("analysis/2".into()).unwrap(),
            new_token,
        )
        .unwrap(),
        LanguageDependencyHead::Root,
        arc.relation().clone(),
    )
    .unwrap();
    protected_delta = DependencyDelta::Assert(new_arc);
    protected_proposal = RevisionEvent::new(
        &next_domain,
        RevisionReference {
            context: next_context,
            sequence: 1,
            event: text("protected-reproposal"),
        },
        &evidence,
        RevisionChange::Proposed {
            delta: &protected_delta,
        },
    )
    .unwrap();
    let before = snapshot(&next_owner);
    assert!(matches!(
        next_owner.append_event(&protected_proposal, false, false),
        Err(AnalysisEpochRefusal::Session(
            conduit_language::analysis_session::AnalysisSessionRefusal::Revision(
                RevisionRefusal::CommittedHistoryRequiresCorrection
            )
        ))
    ));
    correction = RevisionEvent::new(
        &next_domain,
        RevisionReference {
            context: next_context,
            sequence: 1,
            event: text("foreign-old-correction"),
        },
        &evidence,
        RevisionChange::Corrected {
            commit: commit.reference(),
            delta: &protected_delta,
            reason: text("cannot rewrite archived epoch"),
        },
    )
    .unwrap();
    assert!(matches!(
        next_owner.append_event(&correction, false, false),
        Err(AnalysisEpochRefusal::Session(
            conduit_language::analysis_session::AnalysisSessionRefusal::Revision(
                RevisionRefusal::UnknownCommit
            )
        ))
    ));
    assert_eq!(snapshot(&next_owner), before);
    closed = RevisionEvent::new(
        &next_domain,
        RevisionReference {
            context: next_context,
            sequence: 1,
            event: text("closed"),
        },
        &evidence,
        RevisionChange::Closed,
    )
    .unwrap();
    next_owner.append_event(&closed, false, false).unwrap();
    let before = snapshot(&next_owner);
    let operation_count = runtime.operations;
    assert!(matches!(
        next_owner.prepare_fallback_round_with(&programs, runtime),
        Err(FallbackRoundPreparationFailure {
            refusal: FallbackRoundRefusal::Closed,
            ..
        })
    ));
    assert_eq!(
        runtime.operations, operation_count,
        "closed owner executes no new Source operation"
    );
    assert!(matches!(
        next_owner.apply_fallback_round(old_stale_round.take().unwrap()),
        Err(FallbackRoundApplyFailure {
            refusal: FallbackRoundRefusal::Closed,
            ..
        })
    ));
    assert_eq!(snapshot(&next_owner), before);
    runtime.assert_signs();
    eprintln!("fallback-only actual Source operations={}, repeated-prefix replay=0; one duplicate round replay for stale proof; eight finite rounds",runtime.operations);
}
