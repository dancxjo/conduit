use conduit_core::*;
use conduit_language::{lexical::prepare_lexical_tape, *};
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
#[path = "common/parser_fixture.rs"]
mod fixture;
#[path = "common/parser_kernel.rs"]
mod parser_kernel;
use fixture::{field, field_type, number, record, replace};

fn source() -> String {
    [
        fixture::parser_source(),
        include_str!("../text_revision.conduit").into(),
        include_str!("../lexical.conduit").into(),
        include_str!("../parser_joint.conduit").into(),
    ]
    .join("\n")
}
fn lexical() -> LanguageParserJointLexical {
    lexical_with_pos(&[LanguageLexicalPos::Noun, LanguageLexicalPos::Verb])
}
fn lexical_with_pos(pos: &[LanguageLexicalPos]) -> LanguageParserJointLexical {
    let provenance = LinguisticDerivationProvenance::deterministic_rule(
        "fixture/joint".into(),
        "profile/1".into(),
    )
    .unwrap();
    let language = LanguageId::new("language/en".into()).unwrap();
    let source = LanguageTextRevision::new(
        LanguageTextFinality::Final,
        LanguageText::new(
            LanguageTextId::new("utterance".into()).unwrap(),
            language.clone(),
            LanguageTextRevisionId::new("source/1".into()).unwrap(),
            "record".into(),
        )
        .unwrap(),
        None,
        provenance.clone(),
        0,
        None,
    )
    .unwrap();
    let entries = BoundedSequence::try_from_iter([LanguageLexicalEntry::new(
        BoundedSequence::try_from_iter(pos.iter().cloned().map(|pos| {
            LanguageLexicalCandidate::new("record".into(), BoundedSequence::new(), pos).unwrap()
        }))
        .unwrap(),
        "record".into(),
    )
    .unwrap()])
    .unwrap();
    let profile =
        LanguageLexicalProfile::new(entries, "fixture/joint".into(), language, provenance).unwrap();
    let tape = prepare_lexical_tape(&source, &profile, None).unwrap();
    LanguageParserJointLexical::new(tape.tape().clone(), 1).unwrap()
}
fn state() -> LanguageParserState {
    let basis = LanguageParserBasis::new(
        LanguageAnalysisRevisionId::new("analysis/1".into()).unwrap(),
        LanguageTextRevisionId::new("source/1".into()).unwrap(),
        LanguageTextId::new("utterance".into()).unwrap(),
    )
    .unwrap();
    let relation = LanguageParserRelation::new(
        LanguageUniversalDependencyRelation::Dep,
        LanguageParserSubtype::new("".into()).unwrap(),
    )
    .unwrap();
    LanguageParserState::new(
        basis,
        0,
        1,
        [5, 5, 5, 5, 4],
        relation.clone(),
        relation.clone(),
        relation.clone(),
        relation,
        [4, 4, 4, 4, 4],
        1,
        0,
    )
    .unwrap()
}
#[test]
fn current_lexical_choices_are_admitted_and_projected_by_actual_source_graph() {
    let lexical = lexical();
    let state = state();
    let mut execution = parser_kernel::Execution::prepare(source(), "language-parser-joint-pos");
    execution.kernel.start().unwrap();
    for (invocation, choice, expected) in [(0, 0, 7), (1, 1, 15)] {
        let query = LanguageParserJointChoiceQuery::new(
            [choice, 0, 0, 0],
            0,
            invocation,
            lexical.clone(),
            state.clone(),
        )
        .unwrap();
        let output = execution.transact(invocation, &query.into_structured().unwrap());
        let native = LanguageParserJointPosContext::from_structured(output).unwrap();
        assert_eq!(native.pos(), &[expected, 16, 16, 16]);
        assert_eq!(*native.query().invocation(), invocation);
        assert_eq!(native.query().state().basis(), state.basis());
    }
    // Official UPOS order is a test expectation; the source owns conversion.
    for (code, pos) in [
        LanguageLexicalPos::Adjective,
        LanguageLexicalPos::Adposition,
        LanguageLexicalPos::Adverb,
        LanguageLexicalPos::Auxiliary,
        LanguageLexicalPos::CoordinatingConjunction,
        LanguageLexicalPos::Determiner,
        LanguageLexicalPos::Interjection,
        LanguageLexicalPos::Noun,
        LanguageLexicalPos::Numeral,
        LanguageLexicalPos::Particle,
        LanguageLexicalPos::Pronoun,
        LanguageLexicalPos::ProperNoun,
        LanguageLexicalPos::Punctuation,
        LanguageLexicalPos::SubordinatingConjunction,
        LanguageLexicalPos::Symbol,
        LanguageLexicalPos::Verb,
        LanguageLexicalPos::Other,
    ]
    .into_iter()
    .enumerate()
    {
        let invocation = code as u64 + 2;
        let query = LanguageParserJointChoiceQuery::new(
            [0, 0, 0, 0],
            0,
            invocation,
            lexical_with_pos(&[pos]),
            state.clone(),
        )
        .unwrap();
        let output = execution.transact(invocation, &query.into_structured().unwrap());
        assert_eq!(
            LanguageParserJointPosContext::from_structured(output)
                .unwrap()
                .pos(),
            &[code as u64, 16, 16, 16]
        );
    }
    assert!(LanguageParserJointChoiceQuery::new(
        [2, 0, 0, 0],
        0,
        2,
        lexical.clone(),
        state.clone()
    )
    .is_err());
    assert!(LanguageParserJointChoiceQuery::new(
        [0, 0, 0, 0],
        1,
        2,
        lexical.clone(),
        state.clone()
    )
    .is_err());
    let query = LanguageParserJointChoiceQuery::new([0, 0, 0, 0], 0, 2, lexical, state)
        .unwrap()
        .into_structured()
        .unwrap();
    let prior = field(&query, "state");
    let forged = replace(
        &query,
        "state",
        replace(
            prior,
            "committed",
            number(field_type(prior.value_type(), "committed"), 1),
        ),
    );
    assert!(LanguageParserJointChoiceQuery::from_structured(forged).is_err());
}
#[test]
fn every_active_beam_choice_and_nested_state_require_recursive_admission() {
    let lexical = lexical();
    let good_lexical = lexical.clone().into_structured().unwrap();
    let tape = field(&good_lexical, "tape");
    let tokens = field(tape, "tokens");
    let StructuredInfoValueShape::Collection(items) = tokens.shape() else {
        panic!("tokens")
    };
    let token = &items[0];
    let span = field(token, "span");
    for invalid_span in [
        replace(
            span,
            "text_revision",
            fixture::text(field_type(span.value_type(), "text_revision"), "foreign"),
        ),
        replace(
            span,
            "basis",
            fixture::unit_variant(field_type(span.value_type(), "basis"), "utf8_byte"),
        ),
    ] {
        let invalid_token = replace(token, "span", invalid_span);
        let invalid_tokens =
            StructuredInfoValue::sequence(tokens.value_type().clone(), vec![invalid_token])
                .unwrap();
        let invalid_tape = replace(tape, "tokens", invalid_tokens);
        assert!(LanguageParserJointLexical::from_structured(replace(
            &good_lexical,
            "tape",
            invalid_tape
        ))
        .is_err());
    }
    let incomplete = replace(
        token,
        "completeness",
        fixture::unit_variant(
            field_type(token.value_type(), "completeness"),
            "trailing_partial",
        ),
    );
    let incomplete =
        StructuredInfoValue::sequence(tokens.value_type().clone(), vec![incomplete]).unwrap();
    assert!(LanguageParserJointLexical::from_structured(replace(
        &good_lexical,
        "tape",
        replace(tape, "tokens", incomplete)
    ))
    .is_err());
    let profile = field(tape, "profile");
    let foreign = replace(
        profile,
        "language",
        fixture::text(field_type(profile.value_type(), "language"), "language/fr"),
    );
    assert!(LanguageParserJointLexical::from_structured(replace(
        &good_lexical,
        "tape",
        replace(tape, "profile", foreign)
    ))
    .is_err());

    let state = state();
    let parser = LanguageParserHypothesis::new(true, 1, 0, state.clone()).unwrap();
    let candidate = LanguageParserJointHypothesis::new([0, 0, 0, 0], parser).unwrap();
    let beam = LanguageParserJointBeam::new(
        state.basis().clone(),
        candidate.clone(),
        candidate.clone(),
        candidate.clone(),
        candidate,
        0,
        0,
        lexical,
    )
    .unwrap();
    let good = beam.into_structured().unwrap();
    assert!(LanguageParserJointBeam::from_structured(good.clone()).is_ok());
    for slot in ["candidate0", "candidate1", "candidate2", "candidate3"] {
        let candidate = field(&good, slot);
        let choices = field(candidate, "choices");
        let StructuredInfoTypeShape::Collection { element, .. } = choices.value_type().shape()
        else {
            panic!("choice collection")
        };
        let invalid = StructuredInfoValue::collection(
            choices.value_type().clone(),
            [2, 0, 0, 0]
                .into_iter()
                .map(|n| number(element, n))
                .collect(),
        )
        .unwrap();
        assert!(LanguageParserJointBeam::from_structured(replace(
            &good,
            slot,
            replace(candidate, "choices", invalid)
        ))
        .is_err());
        let parser = field(candidate, "parser");
        let state = field(parser, "state");
        let invalid = replace(
            state,
            "committed",
            number(field_type(state.value_type(), "committed"), 1),
        );
        assert!(LanguageParserJointBeam::from_structured(replace(
            &good,
            slot,
            replace(candidate, "parser", replace(parser, "state", invalid))
        ))
        .is_err());
    }
    let basis = field(&good, "basis");
    let foreign = record(
        basis.value_type(),
        vec![
            ("text", field(basis, "text").clone()),
            ("source_revision", field(basis, "source_revision").clone()),
            (
                "analysis_revision",
                fixture::text(
                    field_type(basis.value_type(), "analysis_revision"),
                    "foreign",
                ),
            ),
        ],
    );
    assert!(LanguageParserJointBeam::from_structured(replace(&good, "basis", foreign)).is_err());
}

#[test]
fn source_merge_keeps_four_ranked_states_and_their_lexical_choices() {
    // Fixture scores prove source rank/coupling only, not learned accuracy.
    let lexical = lexical();
    let state = state();
    let inactive = LanguageParserJointHypothesis::new(
        [0, 0, 0, 0],
        LanguageParserHypothesis::new(false, 0, 0, state.clone()).unwrap(),
    )
    .unwrap();
    let mut beam = LanguageParserJointRawBeam::new(
        inactive.clone(),
        inactive.clone(),
        inactive.clone(),
        inactive,
    )
    .unwrap();
    let mut execution = parser_kernel::Execution::prepare(source(), "language-parser-joint-merge");
    execution.kernel.start().unwrap();
    for (i, score) in [10, 30, 20, 40, 25, 40].into_iter().enumerate() {
        let id = i as u64 + 1;
        let proposal = LanguageParserJointHypothesis::new(
            [id % 2, 0, 0, 0],
            LanguageParserHypothesis::new(true, id, score, state.clone()).unwrap(),
        )
        .unwrap();
        let input = LanguageParserJointMerge::new(beam, proposal).unwrap();
        beam = LanguageParserJointRawBeam::from_structured(
            execution.transact(i as u64, &input.into_structured().unwrap()),
        )
        .unwrap();
        LanguageParserJointBeam::new(
            state.basis().clone(),
            beam.candidate0().clone(),
            beam.candidate1().clone(),
            beam.candidate2().clone(),
            beam.candidate3().clone(),
            0,
            id,
            lexical.clone(),
        )
        .unwrap();
    }
    let candidates = [
        beam.candidate0(),
        beam.candidate1(),
        beam.candidate2(),
        beam.candidate3(),
    ];
    assert_eq!(candidates.map(|c| *c.parser().identity()), [4, 6, 2, 5]);
    for c in candidates {
        assert_eq!(c.choices()[0], *c.parser().identity() % 2);
    }
}
