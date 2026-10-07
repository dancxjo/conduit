//! Raw search ranking and finite class ABI do not establish linguistic truth.
use conduit_language::{parser_window8::*, *};
fn initial() -> PreparedWindow8State {
    let basis = LanguageParserBasis::new(
        LanguageAnalysisRevisionId::new("window8/search".into()).unwrap(),
        LanguageTextRevisionId::new("window8/r0".into()).unwrap(),
        LanguageTextId::new("window8/text".into()).unwrap(),
    )
    .unwrap();
    let relation = LanguageParserRelation::new(
        LanguageUniversalDependencyRelation::Dep,
        LanguageParserSubtype::new("".into()).unwrap(),
    )
    .unwrap();
    initialize_window8(&LanguageParserWindow8Begin::new(basis, relation, 1).unwrap()).unwrap()
}
#[test]
fn source_ranks_whole_raw_bodies_and_ties_by_explicit_identity() {
    let state = initial();
    let candidate = |identity, score, active| {
        LanguageParserWindow8RawHypothesis::new(
            active,
            [identity; 8],
            identity,
            score,
            0,
            state.state().clone(),
        )
        .unwrap()
    };
    let beam = LanguageParserWindow8RawBeam::new(
        candidate(9, 20, true),
        candidate(3, 20, true),
        candidate(1, 100, false),
        candidate(7, -30, true),
    )
    .unwrap();
    let ranked = window8_rank(beam).unwrap();
    assert_eq!(*ranked.candidate0().identity(), 3);
    assert_eq!(*ranked.candidate1().identity(), 9);
    assert_eq!(*ranked.candidate2().identity(), 7);
    assert!(!ranked.candidate3().active());
    assert_eq!(ranked.candidate0().choices(), &[3; 8]);
    let best = candidate(11, 30, true);
    let merged = window8_merge(ranked, best.clone()).unwrap();
    assert_eq!(merged.candidate0(), &best);
    assert_eq!(*merged.candidate3().identity(), 7);
    let source = window8_class(73, state.state().relation0()).unwrap();
    assert_eq!(source.action(), &LanguageParserAction::RightArc);
    assert_eq!(
        source.relation().base(),
        &LanguageUniversalDependencyRelation::Root
    );
    assert!(window8_class(76, state.state().relation0()).is_err());
}
#[test]
fn source_accumulates_bounded_step_scores_without_admitting_raw_outcome() {
    let prior = initial();
    let basis = prior.state().basis();
    let class = window8_class(73, prior.state().relation0()).unwrap();
    let proposal =
        prepare_window8_proposal(&prior, basis, *class.action(), class.relation()).unwrap();
    assert!(proposal.proposal().accepted());
    let query =
        LanguageParserWindow8RawAdvance::new([0; 8], 1, proposal.proposal().clone(), -12, 1, 30)
            .unwrap();
    let raw = window8_score_advance(query).unwrap();
    assert_eq!(*raw.score(), 18);
    assert_eq!(raw.state(), proposal.proposal().state());
    assert!(prepare_window8_state(raw.state()).is_ok());
    assert_eq!(*prior.state().unread(), 0);
    assert!(LanguageParserWindow8RawAdvance::new(
        [0; 8],
        1,
        proposal.proposal().clone(),
        32000001,
        1,
        0
    )
    .is_err());
    assert!(LanguageParserWindow8RawAdvance::new(
        [0; 8],
        1,
        proposal.proposal().clone(),
        0,
        1,
        1000001
    )
    .is_err());
}
#[test]
fn all_model_classes_match_ordinary_and_prepared_nominal_projection() {
    use conduit_plot::rust_binding::NativeRustBinding;
    fn prepared<I: NativeRustBinding, O: NativeRustBinding>(input: I, source: &str) -> O {
        let program = conduit_plot::PortableExpressionProgram::from_canonical_hex(source).unwrap();
        let mut evaluator =
            conduit_plot::PreparedPortableExpressionEvaluator::new(&program).unwrap();
        let encoded = input.encode().unwrap();
        O::decode(evaluator.evaluate(&encoded).unwrap()).unwrap()
    }
    let state = initial();
    for code in 0..76 {
        let query =
            LanguageParserWindow8ClassQuery::new(code, state.state().relation0().clone()).unwrap();
        let index: LanguageParserWindow8RawClassIndex = prepared(
            query,
            include_str!(concat!(env!("OUT_DIR"), "/window8_class_index.hex")),
        );
        let relations: LanguageParserWindow8RawClassRelations = prepared(
            index,
            include_str!(concat!(env!("OUT_DIR"), "/window8_class_relations.hex")),
        );
        let actual: LanguageParserWindow8RawClass = prepared(
            relations,
            include_str!(concat!(env!("OUT_DIR"), "/window8_class_relation.hex")),
        );
        assert_eq!(
            actual,
            window8_class(code, state.state().relation0()).unwrap(),
            "class {code}"
        );
    }
    let subtype = LanguageParserRelation::new(
        LanguageUniversalDependencyRelation::Dep,
        LanguageParserSubtype::new("poss".into()).unwrap(),
    )
    .unwrap();
    assert!(LanguageParserWindow8ClassQuery::new(0, subtype).is_err());
}
#[test]
fn private_source_context_preserves_all_classes_and_stale_refusals() {
    use conduit_plot::rust_binding::NativeRustBinding;
    let prior = initial();
    let stale = LanguageParserBasis::new(
        LanguageAnalysisRevisionId::new("window8/foreign-analysis".into()).unwrap(),
        prior.state().basis().source_revision().clone(),
        prior.state().basis().text().clone(),
    )
    .unwrap();
    for basis in [prior.state().basis(), &stale] {
        let cached = prepare_window8_context(&prior, basis).unwrap();
        let request = LanguageParserWindow8RawRequest::new(
            LanguageParserAction::RightArc,
            basis.clone(),
            prior.state().relation0().clone(),
            prior.proof().clone(),
            prior.proof().ancestry()[0].clone(),
        )
        .unwrap();
        let program = conduit_plot::PortableExpressionProgram::from_canonical_hex(include_str!(
            concat!(env!("OUT_DIR"), "/window8_move_context.hex")
        ))
        .unwrap();
        let seed = LanguageParserWindow8RawContext::decode(
            &program.evaluate(&request.encode().unwrap()).unwrap(),
        )
        .unwrap();
        let class_program = conduit_plot::PortableExpressionProgram::from_canonical_hex(
            include_str!(concat!(env!("OUT_DIR"), "/window8_class_context.hex")),
        )
        .unwrap();
        let mut evaluator =
            conduit_plot::PreparedPortableExpressionEvaluator::new(&class_program).unwrap();
        assert_eq!(cached.prior().proof(), prior.proof());
        for code in 0..76 {
            let class = window8_class(code, prior.state().relation0()).unwrap();
            let encoded = LanguageParserWindow8RawClassContext::new(class.clone(), seed.clone())
                .unwrap()
                .encode()
                .unwrap();
            assert_eq!(
                evaluator.evaluate(&encoded).unwrap(),
                class_program.evaluate(&encoded).unwrap(),
                "context class {code}"
            );
            let ordinary =
                prepare_window8_proposal(&prior, basis, *class.action(), class.relation()).unwrap();
            let actual = cached.propose(&class).unwrap();
            assert_eq!(actual.prior(), ordinary.prior());
            assert_eq!(actual.proposal(), ordinary.proposal(), "class {code}");
            if basis == &stale {
                assert!(!actual.proposal().accepted());
            }
        }
    }
}
