//! Exact Source candidate projection retains the full early lexical fact.
use conduit_language::{
    parser_window8::{self, lexical::*},
    *,
};
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};

#[test]
fn source_projects_each_admitted_early_lexical_candidate_without_an_arc() {
    let provenance = LinguisticDerivationProvenance::deterministic_rule(
        "window8/fact-fixture".into(),
        "profile/3".into(),
    )
    .unwrap();
    let source = LanguageTextRevision::new(
        LanguageTextFinality::Partial,
        LanguageText::new(
            LanguageTextId::new("window8/fact".into()).unwrap(),
            LanguageId::new("language/en".into()).unwrap(),
            LanguageTextRevisionId::new("window8/fact/r0".into()).unwrap(),
            "record ".into(),
        )
        .unwrap(),
        None,
        provenance.clone(),
        0,
        Some(6),
    )
    .unwrap();
    let candidates = BoundedSequence::try_from_iter(
        [LanguageLexicalPos::Noun, LanguageLexicalPos::Verb]
            .into_iter()
            .map(|pos| {
                LanguageLexicalCandidate::new("record".into(), BoundedSequence::new(), pos).unwrap()
            }),
    )
    .unwrap();
    let profile = LanguageLexicalProfile::new(
        BoundedSequence::try_from_iter([
            LanguageLexicalEntry::new(candidates, "record".into()).unwrap()
        ])
        .unwrap(),
        "window8/fact-profile".into(),
        LanguageId::new("language/en".into()).unwrap(),
        provenance,
    )
    .unwrap();
    let tape = conduit_language::lexical::prepare_lexical_tape(&source, &profile, None).unwrap();
    let lexical = prepare_window8_lexical(&tape).unwrap();
    let basis = LanguageParserBasis::new(
        LanguageAnalysisRevisionId::new("window8/fact-analysis".into()).unwrap(),
        source.material().revision().clone(),
        source.material().identity().clone(),
    )
    .unwrap();
    let relation = LanguageParserRelation::new(
        LanguageUniversalDependencyRelation::Dep,
        LanguageParserSubtype::new("".into()).unwrap(),
    )
    .unwrap();
    let state = parser_window8::initialize_window8(
        &LanguageParserWindow8Begin::new(basis.clone(), relation, 1).unwrap(),
    )
    .unwrap();
    let hypothesis = |choice: u64| {
        let mut choices = [0; 8];
        choices[0] = choice;
        LanguageParserWindow8CheckedHypothesis::new(
            LanguageParserWindow8RawHypothesis::new(
                true,
                choices,
                choice,
                0,
                1,
                state.state().clone(),
            )
            .unwrap(),
            state.proof().clone(),
        )
        .unwrap()
    };
    let snapshot = |second: u64,
                    lexical: &LanguageParserWindow8Lexical,
                    offered_basis: &LanguageParserBasis| {
        LanguageParserWindow8Snapshot::new(
            offered_basis.clone(),
            hypothesis(second),
            hypothesis(second),
            hypothesis(second),
            hypothesis(second),
            lexical.clone(),
        )
    };
    let admit = |snapshot| {
        LanguageParserWindow8StableLexicalFact::new(
            LanguageParserWindow8FactQuery::new(0, snapshot).unwrap(),
        )
    };
    let mut startup = conduit_plot::StartupCatalog::new();
    conduit_language::install_linguistics_catalogs(
        &mut startup,
        &mut conduit_plot::ProfileCatalog::new(),
    )
    .unwrap();
    let checked = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(include_str!(
            "../parser_window8_lexical_selection.conduit"
        )),
        &startup,
    )
    .unwrap();
    let expanded = conduit_plot::expand_canonical_plot_for_authoring(
        &checked,
        "language-window8-stable-lexical-candidate",
        &conduit_plot::ProfileCatalog::new(),
    )
    .unwrap();
    assert_eq!(expanded.expanded.gears.len(), 1);
    let [configuration] = expanded.expanded.gears[0].configuration.as_slice() else {
        panic!("one exact Source program")
    };
    let conduit_core::ConfigurationValue::Text(hex) = &configuration.value else {
        panic!("encoded Source program")
    };
    let program = conduit_plot::PortableExpressionProgram::from_canonical_hex(hex).unwrap();
    for choice in 0..2 {
        let fact = admit(snapshot(choice, lexical.lexical(), &basis).unwrap()).unwrap();
        let bytes = program.evaluate(&fact.clone().encode().unwrap()).unwrap();
        let candidate = LanguageLexicalCandidate::decode(&bytes).unwrap();
        let prepared =
            conduit_language::stable_lexical_selection::prepare_stable_lexical_selection(
                &tape, &fact,
            )
            .unwrap();
        assert!(std::ptr::eq(prepared.fact(), &fact));
        assert!(std::ptr::eq(prepared.lexical(), &tape));
        assert_eq!(prepared.candidate(), &candidate);
        let foreign_profile = LanguageLexicalProfile::new(
            profile.entries().clone(),
            "foreign/lexical-profile".into(),
            profile.language().clone(),
            profile.provenance().clone(),
        )
        .unwrap();
        let foreign_tape =
            conduit_language::lexical::prepare_lexical_tape(&source, &foreign_profile, None)
                .unwrap();
        assert!(matches!(conduit_language::stable_lexical_selection::prepare_stable_lexical_selection(&foreign_tape, &fact),
            Err(conduit_language::stable_lexical_selection::StableLexicalSelectionRefusal::LexicalTape)));

        assert_eq!(
            &candidate,
            tape.tape()
                .tokens()
                .iter()
                .next()
                .unwrap()
                .candidates()
                .iter()
                .nth(choice as usize)
                .unwrap()
        );
        assert_eq!(
            fact.query()
                .snapshot()
                .candidate0()
                .hypothesis()
                .state()
                .heads()[0],
            9
        );
        assert_eq!(
            *fact.query().snapshot().lexical().tape().source().finality(),
            LanguageTextFinality::Partial
        );
    }
}
