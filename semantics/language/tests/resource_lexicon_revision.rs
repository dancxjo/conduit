#![cfg(target_has_atomic = "ptr")]
#[path = "common/resource_lexicon.rs"]
mod fixture;
use conduit_language::{resource_lexicon_snapshot::*, *};
use fixture::{pinned_owner, source};

fn revised_source(
    previous: &LanguageTextRevision,
    text: &str,
    stable: u32,
) -> LanguageTextRevision {
    LanguageTextRevision::new(
        LanguageTextFinality::Partial,
        LanguageText::new(
            previous.material().identity().clone(),
            previous.material().language().clone(),
            LanguageTextRevisionId::new(format!("{}", previous.sequence() + 2)).unwrap(),
            text.into(),
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
fn dep() -> LanguageParserRelation {
    LanguageParserRelation::new(
        LanguageUniversalDependencyRelation::Dep,
        LanguageParserSubtype::new("".into()).unwrap(),
    )
    .unwrap()
}
#[test]
#[ignore = "requires explicit existing private pinned TRAIN path; actual Source revision and all-four epoch handoff, no model"]
fn actual_resource_revision_extends_profile_and_rebases_all_four_through_same_epoch_owner() {
    use conduit_core::revision::*;
    use conduit_language::{
        analysis_epoch_initialization::*, analysis_epoch_owner::*, lexical::*,
        resource_parser_rebase::*, revision::*,
    };
    use conduit_plot::{rust_binding::NativeRustBinding, PortableExpressionProgram};
    let resource = pinned_owner();
    let snapshot_program = PreparedResourceSnapshotProgram::prepare().unwrap();
    for surface in ["answer", "record"] {
        assert_eq!(
            resource.lookup(surface).unwrap().unwrap().pos_codes().len(),
            2
        );
    }
    let old_source = source("answer ", LanguageTextFinality::Partial);
    let partial_source = revised_source(&old_source, "answer rec", 7);
    let next_source = revised_source(&partial_source, "answer record ", 14);
    let old = PreparedResourceLexicalSnapshot::prepare(&old_source, &resource, &snapshot_program)
        .unwrap();
    let partial =
        PreparedResourceLexicalSnapshot::prepare_revision(&partial_source, &old, &snapshot_program)
            .unwrap();
    assert_eq!(partial.bindings().len(), 1);
    assert!(partial.lexical().tape().tokens()[1].candidates().is_empty());
    assert!(matches!(
        partial.lexical().tape().tokens()[1].completeness(),
        LanguageLexicalCompleteness::TrailingPartial
    ));
    let next = PreparedResourceLexicalSnapshot::prepare_revision(
        &next_source,
        &partial,
        &snapshot_program,
    )
    .unwrap();
    assert_eq!(next.bindings().len(), 2);
    assert_eq!(next.lexical().tape().profile().entries().len(), 2);
    assert_ne!(
        next.lexical().tape().profile(),
        partial.lexical().tape().profile()
    );
    assert_eq!(
        next.lexical().tape().tokens()[0]
            .prior_occurrence()
            .as_ref(),
        Some(partial.lexical().tape().tokens()[0].identity())
    );
    assert_eq!(
        next.lexical().tape().tokens()[0].candidates(),
        partial.lexical().tape().tokens()[0].candidates()
    );
    // Legacy route still refuses rotating the full active profile.
    assert!(matches!(
        prepare_lexical_tape(
            &next_source,
            next.lexical().tape().profile(),
            Some(partial.lexical())
        ),
        Err(LexicalRefusal::Profile)
    ));
    // Initialize directly on the partial epoch: its trailing token has no POS.
    let lexical = prepare_lexical_tape(&old_source, old.lexical().tape().profile(), None).unwrap();
    let begin = LanguageParserBegin::new(
        LanguageParserBasis::new(
            LanguageAnalysisRevisionId::new("analysis/1".into()).unwrap(),
            old_source.material().revision().clone(),
            old_source.material().identity().clone(),
        )
        .unwrap(),
        dep(),
        1,
    )
    .unwrap();
    let init_expression = PortableExpressionProgram::from_canonical_hex(include_str!(concat!(
        env!("OUT_DIR"),
        "/analysis_initialize_program.hex"
    )))
    .unwrap();
    let observed = LanguageParserNumericState::decode(
        &init_expression
            .evaluate(&begin.clone().encode().unwrap())
            .unwrap(),
    )
    .unwrap();
    let initial = PreparedAnalysisInitializationProgram::prepare()
        .unwrap()
        .replay(lexical, &begin, &observed)
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
        stream: RevisionText::new("resource-stream").unwrap(),
        subject: RevisionText::new("resource-subject").unwrap(),
        epoch: RevisionText::new("1").unwrap(),
        producer: RevisionText::new("fixture").unwrap(),
        policy: RevisionText::new("resource@1").unwrap(),
    };
    let limits = RevisionLimits {
        history_events: 8,
        revisable_units: 4,
    };
    let owner = AnalysisEpochOwner::initialize_resource(initial, old, &old_domain, context, limits)
        .unwrap();
    let prior_beam = owner.beam().clone();
    let program = PreparedResourceParserRebaseProgram::prepare().unwrap();
    // Same-epoch context refuses transactionally before any next publication.
    let failed = owner
        .handoff_resource(
            partial,
            &program,
            ResourceEpochTarget {
                analysis: LanguageAnalysisRevisionId::new("analysis/2".into()).unwrap(),
                default_relation: dep(),
                domain: &partial_domain,
                context: context,
                limits: limits,
            },
        )
        .err()
        .expect("same context refusal");
    assert!(matches!(failed.refusal, AnalysisEpochRefusal::Context));
    assert_eq!(failed.owner.beam(), &prior_beam);
    assert!(failed.owner.resource_rebase_receipts().is_empty());
    let owner = failed.owner;
    let partial = failed.next;
    let (owner, archived) = owner
        .handoff_resource(
            partial,
            &program,
            ResourceEpochTarget {
                analysis: LanguageAnalysisRevisionId::new("analysis/2".into()).unwrap(),
                default_relation: dep(),
                domain: &partial_domain,
                context: RevisionContext {
                    epoch: RevisionText::new("2").unwrap(),
                    ..context
                },
                limits: limits,
            },
        )
        .unwrap_or_else(|_| panic!("partial handoff"));
    assert_eq!(archived.beam(), &prior_beam);
    assert_eq!(owner.resource_rebase_receipts().len(), 4);
    assert_eq!(*owner.beam().candidate0().parser().state().token_count(), 1);
    let prior_beam = owner.beam().clone();
    let (owner, archived) = owner
        .handoff_resource(
            next,
            &program,
            ResourceEpochTarget {
                analysis: LanguageAnalysisRevisionId::new("analysis/3".into()).unwrap(),
                default_relation: dep(),
                domain: &next_domain,
                context: RevisionContext {
                    epoch: RevisionText::new("3").unwrap(),
                    ..context
                },
                limits: limits,
            },
        )
        .unwrap_or_else(|_| panic!("complete handoff"));
    for (receipt, prior) in owner.resource_rebase_receipts().iter().zip([
        prior_beam.candidate0(),
        prior_beam.candidate1(),
        prior_beam.candidate2(),
        prior_beam.candidate3(),
    ]) {
        assert_eq!(receipt.request().previous(), prior.parser().state());
        assert_eq!(
            receipt.request().previous_lexical().tape(),
            archived.lexical().tape()
        );
        assert_eq!(receipt.request().next().tape(), owner.lexical().tape());
        assert_eq!(*receipt.result().token_count(), 2);
        assert!(!receipt.input_canonical().is_empty());
        assert!(!receipt.adapted_canonical().is_empty());
        assert_eq!(
            receipt.output_canonical(),
            receipt.raw_result().clone().encode().unwrap()
        );
    }
    assert_eq!(owner.frontiers().committed.0, 0);
    assert_eq!(
        *owner.beam().candidate0().parser().identity(),
        *prior_beam.candidate0().parser().identity()
    );
    assert_eq!(
        owner.lexical().tape().tokens()[0].candidates(),
        archived.lexical().tape().tokens()[0].candidates()
    );
    assert!(std::ptr::eq(
        owner.resource_snapshot().unwrap().lexicon(),
        &resource
    ));
}

#[test]
#[ignore = "requires explicit existing private pinned TRAIN path; checked committed candidate and resource custody refusals"]
fn resource_rebase_preserves_committed_candidates_and_refuses_foreign_or_absent_lineage() {
    use conduit_language::resource_parser_rebase::*;
    let resource = pinned_owner();
    let other_resource = pinned_owner();
    let snapshot_program = PreparedResourceSnapshotProgram::prepare().unwrap();
    let old_source = source("answer ", LanguageTextFinality::Partial);
    let next_source = revised_source(&old_source, "answer record ", 14);
    let old = PreparedResourceLexicalSnapshot::prepare(&old_source, &resource, &snapshot_program)
        .unwrap();
    let next =
        PreparedResourceLexicalSnapshot::prepare_revision(&next_source, &old, &snapshot_program)
            .unwrap();
    let basis = LanguageParserBasis::new(
        LanguageAnalysisRevisionId::new("analysis/1".into()).unwrap(),
        old_source.material().revision().clone(),
        old_source.material().identity().clone(),
    )
    .unwrap();
    let root = LanguageParserRelation::new(
        LanguageUniversalDependencyRelation::Root,
        LanguageParserSubtype::new("".into()).unwrap(),
    )
    .unwrap();
    // Explicit semantic fixture: this is not an installed commitment receipt.
    let committed = LanguageParserState::new(
        basis,
        1,
        2,
        [4, 5, 5, 5, 4],
        root.clone(),
        dep(),
        dep(),
        dep(),
        [4, 0, 4, 4, 4],
        1,
        1,
    )
    .unwrap();
    let program = PreparedResourceParserRebaseProgram::prepare().unwrap();
    let replay = program
        .replay(
            &committed,
            &old,
            &next,
            LanguageAnalysisRevisionId::new("analysis/2".into()).unwrap(),
            dep(),
        )
        .unwrap();
    assert_eq!(*replay.result().committed(), 1);
    assert_eq!(*replay.result().unread(), 1);
    assert_eq!(replay.result().heads(), &[4, 5, 5, 5, 4]);
    assert_eq!(replay.result().relation0(), &root);
    assert_eq!(
        replay.request().previous_lexical().tape().tokens()[0].candidates(),
        replay.request().next().tape().tokens()[0].candidates()
    );
    let foreign_old =
        PreparedResourceLexicalSnapshot::prepare(&old_source, &other_resource, &snapshot_program)
            .unwrap();
    let foreign_next = PreparedResourceLexicalSnapshot::prepare_revision(
        &next_source,
        &foreign_old,
        &snapshot_program,
    )
    .unwrap();
    assert!(matches!(
        program.replay(
            &committed,
            &old,
            &foreign_next,
            LanguageAnalysisRevisionId::new("analysis/2".into()).unwrap(),
            dep()
        ),
        Err(ResourceParserRebaseRefusal::Resource)
    ));
    assert!(matches!(
        program.replay(
            &committed,
            &old,
            &old,
            LanguageAnalysisRevisionId::new("analysis/2".into()).unwrap(),
            dep()
        ),
        Err(ResourceParserRebaseRefusal::Lineage)
    ));
    assert!(PreparedResourceLexicalSnapshot::prepare_revision(
        &old_source,
        &old,
        &snapshot_program
    )
    .is_err());
    assert!(PreparedResourceLexicalSnapshot::prepare_revision(
        &revised_source(&old_source, "record ", 7),
        &old,
        &snapshot_program
    )
    .is_err());
    assert!(PreparedResourceLexicalSnapshot::prepare_revision(
        &revised_source(&old_source, "answer answer answer answer answer ", 35),
        &old,
        &snapshot_program
    )
    .is_err());
    assert!(program
        .replay(
            &committed,
            &old,
            &next,
            committed.basis().analysis_revision().clone(),
            dep()
        )
        .is_err());
}
