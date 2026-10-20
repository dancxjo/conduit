//! Initial-owner custody proof only; no learned transitions or installed runtime.
use conduit_core::revision::*;
use conduit_language::{
    analysis_epoch_initialization::*, analysis_epoch_owner::*, lexical::*, revision::*, *,
};
use conduit_plot::{
    rust_binding::{BoundedSequence, NativeRustBinding},
    PortableExpressionProgram,
};
fn text(s: &str) -> RevisionText<'_> {
    RevisionText::new(s).unwrap()
}
fn lexical() -> PreparedLexicalTape {
    let provenance =
        LinguisticDerivationProvenance::deterministic_rule("epoch-fixture".into(), "1".into())
            .unwrap();
    let source = LanguageTextRevision::new(
        LanguageTextFinality::Final,
        LanguageText::new(
            LanguageTextId::new("epoch-source".into()).unwrap(),
            LanguageId::new("language/en".into()).unwrap(),
            LanguageTextRevisionId::new("1".into()).unwrap(),
            "Hello".into(),
        )
        .unwrap(),
        None,
        provenance.clone(),
        0,
        Some(5),
    )
    .unwrap();
    let entry = LanguageLexicalEntry::new(
        BoundedSequence::try_from_iter([LanguageLexicalCandidate::new(
            "hello".into(),
            BoundedSequence::new(),
            LanguageLexicalPos::Interjection,
        )
        .unwrap()])
        .unwrap(),
        "Hello".into(),
    )
    .unwrap();
    let profile = LanguageLexicalProfile::new(
        BoundedSequence::try_from_iter([entry]).unwrap(),
        "epoch-fixture".into(),
        source.material().language().clone(),
        provenance,
    )
    .unwrap();
    prepare_lexical_tape(&source, &profile, None).unwrap()
}
fn request(tape: &PreparedLexicalTape) -> LanguageParserBegin {
    LanguageParserBegin::new(
        LanguageParserBasis::new(
            LanguageAnalysisRevisionId::new("analysis/1".into()).unwrap(),
            tape.tape().source().material().revision().clone(),
            tape.tape().source().material().identity().clone(),
        )
        .unwrap(),
        LanguageParserRelation::new(
            LanguageUniversalDependencyRelation::Dep,
            LanguageParserSubtype::new("".into()).unwrap(),
        )
        .unwrap(),
        1,
    )
    .unwrap()
}
fn observed(request: &LanguageParserBegin) -> LanguageParserNumericState {
    let program = PortableExpressionProgram::from_canonical_hex(include_str!(concat!(
        env!("OUT_DIR"),
        "/analysis_initialize_program.hex"
    )))
    .unwrap();
    LanguageParserNumericState::decode(
        &program
            .evaluate(&request.clone().encode().unwrap())
            .unwrap(),
    )
    .unwrap()
}
#[test]
fn only_exact_uncommitted_source_initialization_seeds_all_four_private_slots() {
    let lexical = lexical();
    let request = request(&lexical);
    let actual = observed(&request);
    let program = PreparedAnalysisInitializationProgram::prepare().unwrap();
    let initial = program.replay(lexical, &request, &actual).unwrap();
    assert_eq!(*initial.beam().candidate0().parser().active(), true);
    for slot in [
        initial.beam().candidate0(),
        initial.beam().candidate1(),
        initial.beam().candidate2(),
        initial.beam().candidate3(),
    ] {
        assert_eq!(*slot.parser().state().committed(), 0);
        assert_eq!(*slot.choices(), [0; 4]);
        assert_eq!(*slot.selected(), 0);
    }
    let source = initial.request().basis().clone();
    let domain_text = LanguageText::new(
        source.text().clone(),
        LanguageId::new("language/en".into()).unwrap(),
        source.source_revision().clone(),
        "Hello".into(),
    )
    .unwrap();
    let domain = DependencyRevisions {
        source_text: &domain_text,
        tokens: 1,
    };
    let context = RevisionContext {
        stream: text("analysis"),
        subject: text("epoch-source"),
        epoch: text("1"),
        producer: text("fixture"),
        policy: text("epoch@1"),
    };
    let proposal = RevisionReference {
        context,
        sequence: 1,
        event: text("untrusted-proposal"),
    };
    let evidence = [RevisionEvidence {
        source: text("caller-supplied-stability"),
        generation: text("1"),
    }];
    let stable = RevisionEvent::new(
        &domain,
        RevisionReference {
            context,
            sequence: 2,
            event: text("untrusted-stable"),
        },
        &evidence,
        RevisionChange::Stable {
            revision: proposal,
            through: TokenFrontier(0),
        },
    )
    .unwrap();
    let mut owner = AnalysisEpochOwner::initialize(
        initial,
        &domain,
        context,
        RevisionLimits {
            history_events: 8,
            revisable_units: 4,
        },
    )
    .unwrap();
    assert!(matches!(
        owner.append_event(&stable, false, false),
        Err(AnalysisEpochRefusal::FrontierCustody)
    ));
    assert_eq!(owner.generation(), 0);
    assert_eq!(owner.frontiers().committed, TokenFrontier(0));
    assert_eq!(owner.history().count(), 0);
    assert!(owner.commitment_receipts().is_empty());
}
#[test]
fn foreign_observation_refuses_without_publishing_owner() {
    let lexical = lexical();
    let request = request(&lexical);
    let actual = observed(&request);
    let changed = LanguageParserNumericState::new(
        actual.basis().clone(),
        1,
        *actual.depth(),
        *actual.heads(),
        actual.relation0().clone(),
        actual.relation1().clone(),
        actual.relation2().clone(),
        actual.relation3().clone(),
        *actual.stack(),
        *actual.token_count(),
        *actual.unread(),
    )
    .unwrap();
    assert!(matches!(
        PreparedAnalysisInitializationProgram::prepare()
            .unwrap()
            .replay(lexical, &request, &changed),
        Err(AnalysisInitializationRefusal::ObservationMismatch)
    ));
}
#[test]
fn consuming_zero_prefix_epoch_handoff_retains_every_slot_and_archives_history_only() {
    check_handoff(false);
}
#[test]
fn closed_stream_cannot_reopen_through_epoch_handoff() {
    check_handoff(true);
}
fn check_handoff(close: bool) {
    use conduit_language::parser_rebase_replay::*;
    let lexical = lexical();
    let request = request(&lexical);
    let actual = observed(&request);
    let old_text = lexical.tape().source().material().clone();
    let next_source = LanguageTextRevision::new(
        LanguageTextFinality::Final,
        LanguageText::new(
            old_text.identity().clone(),
            old_text.language().clone(),
            LanguageTextRevisionId::new("2".into()).unwrap(),
            old_text.text().clone(),
        )
        .unwrap(),
        Some(LanguageTextPriorRevision::new(old_text.revision().clone(), 0).unwrap()),
        lexical.tape().source().provenance().clone(),
        1,
        Some(5),
    )
    .unwrap();
    let initial = PreparedAnalysisInitializationProgram::prepare()
        .unwrap()
        .replay(lexical, &request, &actual)
        .unwrap();
    let old_domain = DependencyRevisions {
        source_text: &old_text,
        tokens: 1,
    };
    let next_domain = DependencyRevisions {
        source_text: next_source.material(),
        tokens: 1,
    };
    let context = RevisionContext {
        stream: text("analysis"),
        subject: text("epoch-source"),
        epoch: text("1"),
        producer: text("fixture"),
        policy: text("epoch@1"),
    };
    let limits = RevisionLimits {
        history_events: 8,
        revisable_units: 4,
    };
    let owner = AnalysisEpochOwner::initialize(initial, &old_domain, context, limits).unwrap();
    let previous = owner.beam().clone();
    let replay_program = PreparedParserRebaseProgram::prepare().unwrap();
    let expression = PortableExpressionProgram::from_canonical_hex(include_str!(concat!(
        env!("OUT_DIR"),
        "/parser_rebase_program.hex"
    )))
    .unwrap();
    let mut receipts = Vec::new();
    for slot in [
        previous.candidate0(),
        previous.candidate1(),
        previous.candidate2(),
        previous.candidate3(),
    ] {
        let p = PreparedParserRevisionRebase::prepare(
            slot.parser().state(),
            owner.lexical(),
            &next_source,
            LanguageAnalysisRevisionId::new("analysis/2".into()).unwrap(),
            request.default_relation().clone(),
        )
        .unwrap();
        let observed = LanguageParserNumericState::decode(
            &expression
                .evaluate(&p.request().clone().encode().unwrap())
                .unwrap(),
        )
        .unwrap();
        receipts.push(replay_program.replay(p, &observed).unwrap());
    }
    let receipts = receipts.try_into().ok().unwrap();
    let next_context = RevisionContext {
        epoch: text("2"),
        ..context
    };
    let foreign_context = RevisionContext {
        stream: text("other-stream"),
        ..next_context
    };
    let failure = match owner.handoff(receipts, &next_domain, foreign_context, limits) {
        Err(f) => f,
        Ok(_) => panic!("cross-stream handoff accepted"),
    };
    assert!(matches!(failure.refusal, AnalysisEpochRefusal::Context));
    assert_eq!(failure.owner.beam(), &previous);
    assert_eq!(failure.owner.generation(), 0);
    assert_eq!(failure.owner.history().count(), 0);
    let mut owner = failure.owner;
    let receipts = failure.replays;
    let closed = RevisionEvent::new(
        &old_domain,
        RevisionReference {
            context,
            sequence: 1,
            event: text("closed"),
        },
        &[],
        RevisionChange::Closed,
    )
    .unwrap();
    if close {
        owner.append_event(&closed, false, false).unwrap();
        let before = owner.frontiers();
        let failure = match owner.handoff(receipts, &next_domain, next_context, limits) {
            Err(failure) => failure,
            Ok(_) => panic!("closed stream reopened"),
        };
        assert!(matches!(failure.refusal, AnalysisEpochRefusal::Closed));
        assert_eq!(failure.owner.beam(), &previous);
        assert_eq!(failure.owner.frontiers(), before);
        assert_eq!(failure.owner.generation(), 1);
        assert_eq!(failure.owner.history().count(), 1);
        assert_eq!(failure.replays.len(), 4);
        return;
    }
    let (next, archive) = match owner.handoff(receipts, &next_domain, next_context, limits) {
        Ok(value) => value,
        Err(_) => panic!("exact handoff refused"),
    };
    assert_eq!(archive.beam(), &previous);
    assert_eq!(archive.generation(), 0);
    assert_eq!(next.generation(), 1);
    assert_eq!(next.rebase_receipts().len(), 4);
    assert_eq!(next.frontiers().committed, TokenFrontier(0));
    assert_eq!(next.history().count(), 0);
    assert!(next.commitment_receipts().is_empty());
    for (old, new) in [
        previous.candidate0(),
        previous.candidate1(),
        previous.candidate2(),
        previous.candidate3(),
    ]
    .into_iter()
    .zip([
        next.beam().candidate0(),
        next.beam().candidate1(),
        next.beam().candidate2(),
        next.beam().candidate3(),
    ]) {
        assert_eq!(old.parser().identity(), new.parser().identity());
        assert_eq!(old.parser().active(), new.parser().active());
        assert_eq!(
            new.parser().state().basis().source_revision(),
            next_source.material().revision()
        );
    }
}
