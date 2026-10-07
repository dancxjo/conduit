use conduit_language::{
    parser_window8::{self, lexical::*},
    *,
};
use conduit_plot::rust_binding::BoundedSequence;
fn provenance() -> LinguisticDerivationProvenance {
    LinguisticDerivationProvenance::deterministic_rule("window8/fixture".into(), "profile/3".into())
        .unwrap()
}
fn revision(text: &str, finality: LanguageTextFinality) -> LanguageTextRevision {
    LanguageTextRevision::new(
        finality,
        LanguageText::new(
            LanguageTextId::new("window8/text".into()).unwrap(),
            LanguageId::new("language/en".into()).unwrap(),
            LanguageTextRevisionId::new("window8/r0".into()).unwrap(),
            text.into(),
        )
        .unwrap(),
        None,
        provenance(),
        0,
        None,
    )
    .unwrap()
}
fn profile() -> LanguageLexicalProfile {
    let candidates = BoundedSequence::try_from_iter(
        [LanguageLexicalPos::Noun, LanguageLexicalPos::Verb]
            .into_iter()
            .map(|pos| {
                LanguageLexicalCandidate::new("record".into(), BoundedSequence::new(), pos).unwrap()
            }),
    )
    .unwrap();
    LanguageLexicalProfile::new(
        BoundedSequence::try_from_iter([
            LanguageLexicalEntry::new(candidates, "record".into()).unwrap()
        ])
        .unwrap(),
        "window8/lexical".into(),
        LanguageId::new("language/en".into()).unwrap(),
        provenance(),
    )
    .unwrap()
}
fn basis(source: &LanguageTextRevision) -> LanguageParserBasis {
    LanguageParserBasis::new(
        LanguageAnalysisRevisionId::new("window8/analysis".into()).unwrap(),
        source.material().revision().clone(),
        source.material().identity().clone(),
    )
    .unwrap()
}
fn relation() -> LanguageParserRelation {
    LanguageParserRelation::new(
        LanguageUniversalDependencyRelation::Dep,
        LanguageParserSubtype::new("".into()).unwrap(),
    )
    .unwrap()
}
#[test]
fn source_codes_and_425_features_preserve_alternatives_and_available_future_only() {
    let source = revision(&["record"; 8].join(" "), LanguageTextFinality::Final);
    let full = conduit_language::lexical::prepare_lexical_tape(&source, &profile(), None).unwrap();
    let lexical = prepare_window8_lexical(&full).unwrap();
    assert_eq!(lexical.lexical().tape(), full.tape());
    assert_eq!(*lexical.lexical().token_count(), 8);
    for (ordinal, token) in lexical.projection().tokens().iter().enumerate() {
        assert_eq!(*token.ordinal(), ordinal as u64);
        assert_eq!(token.codes(), &[7, 15, 17, 17]);
        assert_eq!(*token.count(), 2);
    }
    let exact_basis = basis(&source);
    let state = parser_window8::initialize_window8(
        &LanguageParserWindow8Begin::new(exact_basis.clone(), relation(), 8).unwrap(),
    )
    .unwrap();
    let first = prepare_window8_features(&state, &lexical, &exact_basis, [0; 8]).unwrap();
    let mut expected = vec![17, 25, 349, 361, 370, 379, 381, 390];
    expected.extend((0..17).map(|code| 391 + 2 * code + u64::from(code == 7 || code == 15)));
    assert_eq!(first.features().raw().indices().as_slice(), expected);
    let mut future_choices = [0; 8];
    future_choices[1] = 1;
    let future = prepare_window8_features(&state, &lexical, &exact_basis, future_choices).unwrap();
    assert_eq!(first.features(), future.features());
    let choice = LanguageParserWindow8ChoiceQuery::new(first.query().clone(), [0; 8], 0).unwrap();
    assert_eq!(
        *parser_window8::window8_choice_frontier(choice)
            .unwrap()
            .count(),
        1
    );
    assert!(LanguageParserWindow8ChoiceQuery::new(future.query().clone(), [0; 8], 0).is_err());

    let mut current_choices = [0; 8];
    current_choices[0] = 1;
    let current =
        prepare_window8_features(&state, &lexical, &exact_basis, current_choices).unwrap();
    assert_eq!(&current.features().raw().indices()[..3], &[17, 33, 357]);
    assert!(LanguageParserWindow8ChoiceQuery::new(current.query().clone(), [0; 8], 1).is_err());

    let foreign = LanguageParserBasis::new(
        LanguageAnalysisRevisionId::new("foreign".into()).unwrap(),
        exact_basis.source_revision().clone(),
        exact_basis.text().clone(),
    )
    .unwrap();
    assert!(prepare_window8_features(&state, &lexical, &foreign, [0; 8]).is_err());
    let mut impossible = [0; 8];
    impossible[0] = 2;
    assert!(prepare_window8_features(&state, &lexical, &exact_basis, impossible).is_err());
}
#[test]
fn trailing_partial_is_available_wait_not_a_silent_pos_choice() {
    let source = revision("record record re", LanguageTextFinality::Partial);
    let full = conduit_language::lexical::prepare_lexical_tape(&source, &profile(), None).unwrap();
    let lexical = prepare_window8_lexical(&full).unwrap();
    assert_eq!(*lexical.lexical().token_count(), 2);
    assert_eq!(lexical.projection().tokens()[2].codes(), &[17; 4]);
    assert_eq!(*lexical.projection().tokens()[2].count(), 0);
    for text in [["record"; 9].join(" "), "record unknown".into()] {
        let source = revision(&text, LanguageTextFinality::Final);
        let full =
            conduit_language::lexical::prepare_lexical_tape(&source, &profile(), None).unwrap();
        assert!(prepare_window8_lexical(&full).is_err());
    }
}
