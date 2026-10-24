//! Actual nonzero commitment and consuming resource handoff; no invented beam.
#![cfg(target_has_atomic = "ptr")]
#[path = "common/resource_epoch_kernel.rs"]
mod kernel;
#[path = "common/resource_lexicon.rs"]
mod resource;
use conduit_core::revision::*;
use conduit_language::analysis_epoch_owner::fallback_round::PreparedFallbackRoundPrograms;
use conduit_language::{
    analysis_epoch_initialization::*, analysis_epoch_owner::*,
    dependency_commitment::PreparedCommittedDependency, resource_lexicon_snapshot::*,
    resource_parser_rebase::*, revision::*, *,
};
use conduit_plot::rust_binding::NativeRustBinding;
fn text(value: &str) -> RevisionText<'_> {
    RevisionText::new(value).unwrap()
}
fn dep() -> LanguageParserRelation {
    LanguageParserRelation::new(
        LanguageUniversalDependencyRelation::Dep,
        LanguageParserSubtype::new("".into()).unwrap(),
    )
    .unwrap()
}
fn revised(previous: &LanguageTextRevision, value: &str, stable: u32) -> LanguageTextRevision {
    LanguageTextRevision::new(
        LanguageTextFinality::Partial,
        LanguageText::new(
            previous.material().identity().clone(),
            previous.material().language().clone(),
            LanguageTextRevisionId::new(format!("{}", previous.sequence() + 2)).unwrap(),
            value.into(),
        )
        .unwrap(),
        Some(
            LanguageTextPriorRevision::new(
                previous.material().revision().clone(),
                *previous.sequence(),
            )
            .unwrap(),
        ),
        previous.provenance().clone(),
        previous.sequence() + 1,
        Some(stable),
    )
    .unwrap()
}
fn snapshot(
    owner: &AnalysisEpochOwner<'_>,
) -> (Vec<u8>, u64, RevisionFrontiers<TokenFrontier>, usize, usize) {
    (
        owner.beam().clone().encode().unwrap(),
        owner.generation(),
        owner.frontiers(),
        owner.history().count(),
        owner.resource_rebase_receipts().len(),
    )
}
fn replay_snapshot(runtime: &mut kernel::Runtime, value: &PreparedResourceLexicalSnapshot<'_>) {
    for binding in value.bindings() {
        let actual = runtime.execute_exact(
            "language/resource-lexical-binding",
            binding.input_canonical(),
            &[include_str!(concat!(
                env!("OUT_DIR"),
                "/resource_lexical_binding.hex"
            ))],
        );
        assert_eq!(actual, binding.output_canonical());
        assert!(conduit_core::InfoBool::decode(&actual).unwrap().get());
        assert!(std::ptr::eq(binding.row().owner(), value.lexicon()));
    }
}
fn replay_handoff(
    runtime: &mut kernel::Runtime,
    owner: &AnalysisEpochOwner<'_>,
    archive: &ArchivedAnalysisEpoch<'_>,
) {
    let before = [
        archive.beam().candidate0(),
        archive.beam().candidate1(),
        archive.beam().candidate2(),
        archive.beam().candidate3(),
    ];
    let after = [
        owner.beam().candidate0(),
        owner.beam().candidate1(),
        owner.beam().candidate2(),
        owner.beam().candidate3(),
    ];
    assert_eq!(owner.resource_rebase_receipts().len(), 4);
    for ((prior, next), receipt) in before
        .into_iter()
        .zip(after)
        .zip(owner.resource_rebase_receipts())
    {
        assert_eq!(receipt.source_id(), runtime.source_id.as_str());
        assert_eq!(receipt.request().previous(), prior.parser().state());
        assert_eq!(
            receipt.request().previous_lexical().tape(),
            archive.lexical().tape()
        );
        assert_eq!(receipt.request().next().tape(), owner.lexical().tape());
        let adapted = runtime.execute_exact(
            "language-resource-parser-rebase-adapter",
            receipt.input_canonical(),
            &[include_str!(concat!(
                env!("OUT_DIR"),
                "/resource_parser_rebase_adapter.hex"
            ))],
        );
        assert_eq!(adapted, receipt.adapted_canonical());
        let output = runtime.execute_exact(
            "language-parser-rebase-numeric",
            &adapted,
            &[include_str!(concat!(
                env!("OUT_DIR"),
                "/parser_rebase_program.hex"
            ))],
        );
        assert_eq!(output, receipt.output_canonical());
        assert_eq!(
            LanguageParserNumericState::decode(&output).unwrap(),
            *receipt.raw_result()
        );
        assert_eq!(next.parser().state(), receipt.result());
        assert_eq!(next.parser().identity(), prior.parser().identity());
        assert_eq!(next.parser().active(), prior.parser().active());
        assert_eq!(
            next.parser().state().committed(),
            prior.parser().state().committed()
        );
        let protected = *prior.parser().state().committed() as usize;
        assert_eq!(&next.choices()[..protected], &prior.choices()[..protected]);
        assert!(next.choices()[protected..].iter().all(|c| *c == 0));
        assert_eq!(
            &next.parser().state().heads()[..protected],
            &prior.parser().state().heads()[..protected]
        );
        if *prior.parser().active() {
            assert_eq!(protected, 1);
            assert_eq!(next.selected(), next.parser().state().unread());
        }
        for (old, new) in [
            prior.parser().state().relation0(),
            prior.parser().state().relation1(),
            prior.parser().state().relation2(),
            prior.parser().state().relation3(),
        ]
        .into_iter()
        .zip([
            next.parser().state().relation0(),
            next.parser().state().relation1(),
            next.parser().state().relation2(),
            next.parser().state().relation3(),
        ])
        .take(protected)
        {
            assert_eq!(old, new);
        }
    }
    assert_eq!(owner.frontiers().committed, TokenFrontier(1));
    assert_eq!(archive.frontiers().committed, TokenFrontier(1));
}
#[test]
#[expect(
    clippy::needless_late_init,
    reason = "Borrowed event and receipt backing must predate and outlive the consuming owner"
)]
#[ignore = "requires explicit existing private pinned TRAIN path; actual resource owner and ordinary kernel, no model"]
fn actual_committed_resource_owner_rebases_all_four_slots_without_reopening_prefix() {
    let lexicon = resource::pinned_owner();
    let other = resource::pinned_owner();
    let old_source = resource::source("answer ", LanguageTextFinality::Partial);
    let partial_source = revised(&old_source, "answer rec", 7);
    let next_source = revised(&partial_source, "answer record ", 14);
    let program = PreparedResourceSnapshotProgram::prepare().unwrap();
    let old = PreparedResourceLexicalSnapshot::prepare(&old_source, &lexicon, &program).unwrap();
    let mut partial =
        PreparedResourceLexicalSnapshot::prepare_revision(&partial_source, &old, &program).unwrap();
    let next = PreparedResourceLexicalSnapshot::prepare_revision(&next_source, &partial, &program)
        .unwrap();
    let foreign_old =
        PreparedResourceLexicalSnapshot::prepare(&old_source, &other, &program).unwrap();
    let foreign =
        PreparedResourceLexicalSnapshot::prepare_revision(&partial_source, &foreign_old, &program)
            .unwrap();
    let old_domain = DependencyRevisions {
        source_text: old_source.material(),
        tokens: 1,
    };
    let partial_domain = DependencyRevisions {
        source_text: partial_source.material(),
        tokens: 2,
    };
    let next_domain = DependencyRevisions {
        source_text: next_source.material(),
        tokens: 2,
    };
    let context = RevisionContext {
        stream: text("resource"),
        subject: text("actual-owner"),
        epoch: text("1"),
        producer: text("kernel"),
        policy: text("complete-round-zero-score@1"),
    };
    let reference = |sequence, label| RevisionReference {
        context,
        sequence,
        event: text(label),
    };
    let evidence = [RevisionEvidence {
        source: text("actual-resource-Source-Plan-Play"),
        generation: text("1"),
    }];
    // Borrowed event/receipt backing predates the consuming owner.
    let delta;
    let proposal;
    let stable;
    let commitment_event;
    let commitment;
    let mut runtime = kernel::Runtime::new();
    replay_snapshot(&mut runtime, &old);
    replay_snapshot(&mut runtime, &partial);
    replay_snapshot(&mut runtime, &next);
    let basis = LanguageParserBasis::new(
        LanguageAnalysisRevisionId::new("analysis/1".into()).unwrap(),
        old_source.material().revision().clone(),
        old_source.material().identity().clone(),
    )
    .unwrap();
    let begin = LanguageParserBegin::new(basis.clone(), dep(), 1).unwrap();
    let observed = LanguageParserNumericState::decode(&runtime.execute(
        "language-parser-initialize",
        &begin.clone().encode().unwrap(),
    ))
    .unwrap();
    let initial = PreparedAnalysisInitializationProgram::prepare()
        .unwrap()
        .replay(
            conduit_language::lexical::prepare_lexical_tape(
                &old_source,
                old.lexical().tape().profile(),
                None,
            )
            .unwrap(),
            &begin,
            &observed,
        )
        .unwrap();
    assert_eq!(initial.source_id(), runtime.source_id.as_str());
    assert_eq!(
        runtime.execute(
            "language/analysis-initial-envelope",
            &observed.encode().unwrap()
        ),
        initial.envelope_output_canonical()
    );
    let limits = RevisionLimits {
        history_events: 16,
        revisable_units: 4,
    };
    let mut owner =
        AnalysisEpochOwner::initialize_resource(initial, old, &old_domain, context, limits)
            .unwrap();
    let fallback = PreparedFallbackRoundPrograms::prepare().unwrap();
    for _ in 0..8 {
        let round = owner
            .prepare_fallback_round_with(&fallback, &mut runtime)
            .unwrap_or_else(|f| panic!("actual round: {:?}", f.refusal));
        owner
            .apply_fallback_round(round)
            .unwrap_or_else(|f| panic!("round apply: {:?}", f.refusal));
    }
    let arc = LanguageDependencyArc::new(
        LanguageAnalysisTokenRef::new(
            basis.analysis_revision().clone(),
            owner.lexical().tape().tokens()[0].identity().clone(),
        )
        .unwrap(),
        LanguageDependencyHead::Root,
        LanguageDependencyRelation::new(LanguageUniversalDependencyRelation::Root, None).unwrap(),
    )
    .unwrap();
    delta = DependencyDelta::Assert(arc.clone());
    proposal = RevisionEvent::new(
        &old_domain,
        reference(1, "proposal"),
        &evidence,
        RevisionChange::Proposed { delta: &delta },
    )
    .unwrap();
    owner.append_event(&proposal, false, false).unwrap();
    let frontier = owner
        .prepare_fallback_frontier_with(&fallback, &mut runtime, false, false)
        .unwrap_or_else(|f| panic!("frontier: {:?}", f.refusal));
    assert_eq!(*frontier.frontier().stable(), 1);
    let frontier_value = frontier.frontier().clone();
    stable = RevisionEvent::new(
        &old_domain,
        reference(2, "stable"),
        &evidence,
        RevisionChange::Stable {
            revision: proposal.reference(),
            through: TokenFrontier(1),
        },
    )
    .unwrap();
    owner
        .append_fallback_frontier(&stable, frontier)
        .unwrap_or_else(|f| panic!("stable: {:?}", f.refusal));
    let b = owner.beam();
    let beam = LanguageParserAdmittedBeam::new(
        basis,
        b.candidate0().parser().clone(),
        b.candidate1().parser().clone(),
        b.candidate2().parser().clone(),
        b.candidate3().parser().clone(),
    )
    .unwrap();
    let request =
        LanguageParserBeamCommitRequest::new(beam, false, frontier_value, false, 1).unwrap();
    let actual = LanguageParserBeamCommitResult::decode(&runtime.execute(
        "language-parser-beam-commit",
        &request.clone().encode().unwrap(),
    ))
    .unwrap();
    assert!(*actual.accepted());
    commitment =
        PreparedCommittedDependency::prepare(owner.lexical(), &arc, &request, &actual).unwrap();
    commitment_event = RevisionEvent::new(
        &old_domain,
        reference(3, "commit"),
        &evidence,
        RevisionChange::Committed {
            revision: proposal.reference(),
            through: TokenFrontier(1),
        },
    )
    .unwrap();
    owner
        .append_commitment(&commitment_event, &commitment, false, false)
        .unwrap();
    assert_eq!(owner.frontiers().committed, TokenFrontier(1));
    let active = [
        owner.beam().candidate0(),
        owner.beam().candidate1(),
        owner.beam().candidate2(),
        owner.beam().candidate3(),
    ]
    .into_iter()
    .filter(|s| *s.parser().active())
    .count();
    assert!(active > 0 && active <= 4);
    println!("actual resource survivors active={active} slots=4 committed=1");
    // This profile admits exact TRAIN surfaces from the unchanged scalar-run
    // tokenizer. Alternative language-specific/explicit segmenter requests are
    // not modeled. An unsupported boundary surface in the uncommitted suffix
    // must refuse, rather than silently join words or invoke another tokenizer.
    let before = snapshot(&owner);
    assert!(lexicon.lookup("\u{2060}").unwrap().is_none());
    let unsupported_boundary = revised(&old_source, "answer record\u{2060}answer ", 7);
    assert!(matches!(
        PreparedResourceLexicalSnapshot::prepare_revision(
            &unsupported_boundary,
            owner.resource_snapshot().unwrap(),
            &program,
        ),
        Err(ResourceSnapshotRefusal::Absent { ordinal: 2 })
    ));
    assert_eq!(snapshot(&owner), before);
    assert_eq!(owner.frontiers().committed, TokenFrontier(1));
    assert!(std::ptr::eq(
        owner.resource_snapshot().unwrap().lexicon(),
        &lexicon
    ));
    let rebase = PreparedResourceParserRebaseProgram::prepare().unwrap();
    // Cancellation and pressure return the same consuming owners unchanged.
    for (cancelled, pressure) in [(true, false), (false, true)] {
        let before = snapshot(&owner);
        let failure = owner
            .handoff_resource(
                partial,
                &rebase,
                ResourceEpochTarget {
                    analysis: LanguageAnalysisRevisionId::new("analysis/2".into()).unwrap(),
                    default_relation: dep(),
                    domain: &partial_domain,
                    context: RevisionContext {
                        epoch: text("2"),
                        ..context
                    },
                    limits,
                },
                cancelled,
                pressure,
            )
            .err()
            .expect("terminal refusal");
        assert!(matches!(
            (&failure.refusal, cancelled),
            (
                AnalysisEpochRefusal::Session(
                    conduit_language::analysis_session::AnalysisSessionRefusal::Cancelled
                ),
                true
            ) | (
                AnalysisEpochRefusal::Session(
                    conduit_language::analysis_session::AnalysisSessionRefusal::Pressure
                ),
                false
            )
        ));
        assert_eq!(snapshot(&failure.owner), before);
        owner = failure.owner;
        partial = failure.next;
    }
    let before = snapshot(&owner);
    let failure = owner
        .handoff_resource(
            foreign,
            &rebase,
            ResourceEpochTarget {
                analysis: LanguageAnalysisRevisionId::new("analysis/2".into()).unwrap(),
                default_relation: dep(),
                domain: &partial_domain,
                context: RevisionContext {
                    epoch: text("2"),
                    ..context
                },
                limits,
            },
            false,
            false,
        )
        .err()
        .expect("foreign resource refuses");
    assert!(matches!(
        failure.refusal,
        AnalysisEpochRefusal::ResourceRebase(ResourceParserRebaseRefusal::Resource)
    ));
    assert_eq!(snapshot(&failure.owner), before);
    owner = failure.owner;
    let before = snapshot(&owner);
    let failure = owner
        .handoff_resource(
            partial,
            &rebase,
            ResourceEpochTarget {
                analysis: LanguageAnalysisRevisionId::new("analysis/2".into()).unwrap(),
                default_relation: dep(),
                domain: &partial_domain,
                context,
                limits,
            },
            false,
            false,
        )
        .err()
        .expect("same epoch refuses");
    assert!(matches!(failure.refusal, AnalysisEpochRefusal::Context));
    assert_eq!(snapshot(&failure.owner), before);
    owner = failure.owner;
    partial = failure.next;
    // A real zero-capacity retained journal must refuse after valid Source
    // rebase preparation, without publishing a new epoch or consuming custody.
    let before = snapshot(&owner);
    let failure = owner
        .handoff_resource(
            partial,
            &rebase,
            ResourceEpochTarget {
                analysis: LanguageAnalysisRevisionId::new("analysis/2".into()).unwrap(),
                default_relation: dep(),
                domain: &partial_domain,
                context: RevisionContext {
                    epoch: text("2"),
                    ..context
                },
                limits: RevisionLimits {
                    history_events: 0,
                    ..limits
                },
            },
            false,
            false,
        )
        .err()
        .expect("zero retained-history capacity refuses");
    assert!(matches!(
        failure.refusal,
        AnalysisEpochRefusal::Revision(RevisionRefusal::Limits)
    ));
    assert_eq!(snapshot(&failure.owner), before);
    assert!(std::ptr::eq(
        failure.owner.resource_snapshot().unwrap().lexicon(),
        &lexicon
    ));
    assert_eq!(failure.next.lexical().tape(), next.previous_tape().unwrap());
    owner = failure.owner;
    partial = failure.next;
    let (owner, initial_archive) = owner
        .handoff_resource(
            partial,
            &rebase,
            ResourceEpochTarget {
                analysis: LanguageAnalysisRevisionId::new("analysis/2".into()).unwrap(),
                default_relation: dep(),
                domain: &partial_domain,
                context: RevisionContext {
                    epoch: text("2"),
                    ..context
                },
                limits,
            },
            false,
            false,
        )
        .unwrap_or_else(|f| panic!("partial handoff: {:?}", f.refusal));
    assert_eq!(initial_archive.commitment_receipts().len(), 1);
    assert_eq!(owner.lexical().tape().tokens()[1].candidates().len(), 0);
    replay_handoff(&mut runtime, &owner, &initial_archive);
    assert!(owner.current(proposal.reference(), &commitment).is_err());
    let (owner, archive) = owner
        .handoff_resource(
            next,
            &rebase,
            ResourceEpochTarget {
                analysis: LanguageAnalysisRevisionId::new("analysis/3".into()).unwrap(),
                default_relation: dep(),
                domain: &next_domain,
                context: RevisionContext {
                    epoch: text("3"),
                    ..context
                },
                limits,
            },
            false,
            false,
        )
        .unwrap_or_else(|f| panic!("complete handoff: {:?}", f.refusal));
    replay_handoff(&mut runtime, &owner, &archive);
    assert_eq!(owner.resource_snapshot().unwrap().bindings().len(), 2);
    assert_eq!(
        owner.lexical().tape().tokens()[0].candidates(),
        archive.lexical().tape().tokens()[0].candidates()
    );
    assert!(std::ptr::eq(
        owner.resource_snapshot().unwrap().lexicon(),
        &lexicon
    ));
    assert_eq!(initial_archive.commitment_receipts().len(), 1);
    assert_eq!(initial_archive.frontiers().committed, TokenFrontier(1));
    assert!(runtime.operations > 16);
}
