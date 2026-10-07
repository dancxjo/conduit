//! Supplied native forest fixtures validate Source policy, not learned accuracy.
use conduit_core::{StructuredFieldValue, StructuredInfoTypeShape, StructuredInfoValue};
use conduit_language::{
    parser_window8::{self, lexical::*, *},
    *,
};
use conduit_plot::rust_binding::{validate_native_invariants, BoundedSequence, NativeRustBinding};
use conduit_plot::{
    check_syntax_document, parse_syntax_document, CheckedNativeType, ProfileCatalog, StartupCatalog,
};
fn record(ty: &CheckedNativeType, fields: Vec<(&str, StructuredInfoValue)>) -> StructuredInfoValue {
    StructuredInfoValue::record(
        ty.value_type.clone(),
        fields
            .into_iter()
            .map(|(n, v)| StructuredFieldValue::new(n, v).unwrap())
            .collect(),
    )
    .unwrap()
}
fn scalar(ty: &CheckedNativeType, name: &str, value: u64) -> StructuredInfoValue {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.value_type.shape() else {
        panic!("record")
    };
    let field = fields.iter().find(|field| field.name() == name).unwrap();
    StructuredInfoValue::leaf(field.value_type().clone(), value.to_le_bytes().to_vec()).unwrap()
}
fn relation(base: LanguageUniversalDependencyRelation) -> LanguageParserRelation {
    LanguageParserRelation::new(base, LanguageParserSubtype::new("".into()).unwrap()).unwrap()
}
#[test]
fn reviewed_early_dependencies_refuse_provisional_subject_and_unclosed_root() {
    let mut startup = StartupCatalog::new();
    conduit_language::install_linguistics_catalogs(&mut startup, &mut ProfileCatalog::new())
        .unwrap();
    let source = [
        include_str!("../parser_window8_facts.conduit"),
        include_str!("../parser_window8_dependency_facts.conduit"),
    ]
    .join("\n");
    eprintln!("window8 dependency facts: Source check start");
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
    let ty = |name: &str| {
        checked
            .native_types
            .iter()
            .find(|ty| ty.name == name)
            .unwrap()
    };
    let provenance = LinguisticDerivationProvenance::deterministic_rule(
        "window8/dependency-fixture".into(),
        "reviewed-early-det-case-voc@1".into(),
    )
    .unwrap();
    let revision = |finality| {
        LanguageTextRevision::new(
            finality,
            LanguageText::new(
                LanguageTextId::new("window8/dependency".into()).unwrap(),
                LanguageId::new("language/en".into()).unwrap(),
                LanguageTextRevisionId::new("window8/dependency/r0".into()).unwrap(),
                "record record ".into(),
            )
            .unwrap(),
            None,
            provenance.clone(),
            0,
            Some(13),
        )
        .unwrap()
    };
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
        "window8/dependency-profile".into(),
        LanguageId::new("language/en".into()).unwrap(),
        provenance.clone(),
    )
    .unwrap();
    let partial = revision(LanguageTextFinality::Partial);
    let basis = LanguageParserBasis::new(
        LanguageAnalysisRevisionId::new("window8/dependency-analysis".into()).unwrap(),
        partial.material().revision().clone(),
        partial.material().identity().clone(),
    )
    .unwrap();
    let mut state = parser_window8::initialize_window8(
        &LanguageParserWindow8Begin::new(
            basis.clone(),
            relation(LanguageUniversalDependencyRelation::Dep),
            2,
        )
        .unwrap(),
    )
    .unwrap();
    for (action, base) in [
        (
            LanguageParserAction::Shift,
            LanguageUniversalDependencyRelation::Dep,
        ),
        (
            LanguageParserAction::LeftArc,
            LanguageUniversalDependencyRelation::Det,
        ),
        (
            LanguageParserAction::RightArc,
            LanguageUniversalDependencyRelation::Root,
        ),
    ] {
        let step = prepare_window8_step(&state, &basis, action, &relation(base)).unwrap();
        assert!(step.accepted());
        state = step.next().clone();
    }
    let changed = |base| {
        let s = state.state();
        prepare_window8_state(
            &LanguageParserWindow8RawState::new(
                s.basis().clone(),
                *s.committed(),
                *s.depth(),
                *s.heads(),
                relation(base),
                s.relation1().clone(),
                s.relation2().clone(),
                s.relation3().clone(),
                s.relation4().clone(),
                s.relation5().clone(),
                s.relation6().clone(),
                s.relation7().clone(),
                *s.stack(),
                *s.token_count(),
                *s.unread(),
            )
            .unwrap(),
        )
        .unwrap()
    };
    let subject = changed(LanguageUniversalDependencyRelation::Nsubj);
    let snapshot = |finality, first: &PreparedWindow8State, second: &PreparedWindow8State| {
        let tape =
            conduit_language::lexical::prepare_lexical_tape(&revision(finality), &profile, None)
                .unwrap();
        let lexical = prepare_window8_lexical(&tape).unwrap();
        let hypothesis = |proof: &PreparedWindow8State| {
            let native = ty("LanguageParserWindow8CheckedHypothesis");
            let raw = LanguageParserWindow8RawHypothesis::new(
                true,
                [0; 8],
                0,
                0,
                2,
                proof.state().clone(),
            )
            .unwrap();
            let value = record(
                native,
                vec![
                    ("hypothesis", raw.into_structured().unwrap()),
                    ("proof", proof.proof().clone().into_structured().unwrap()),
                ],
            );
            validate_native_invariants(&value, &native.invariants).unwrap();
            value
        };
        let native = ty("LanguageParserWindow8Snapshot");
        let value = record(
            native,
            vec![
                (
                    "lexical",
                    lexical.lexical().clone().into_structured().unwrap(),
                ),
                ("basis", basis.clone().into_structured().unwrap()),
                ("candidate0", hypothesis(first)),
                ("candidate1", hypothesis(second)),
                ("candidate2", hypothesis(first)),
                ("candidate3", hypothesis(first)),
            ],
        );
        validate_native_invariants(&value, &native.invariants).unwrap();
        value
    };
    let fact = |finality,
                first: &PreparedWindow8State,
                second: &PreparedWindow8State,
                dependent,
                head,
                base,
                dependent_end,
                head_end| {
        let query_ty = ty("LanguageParserWindow8FactQuery");
        let query = record(
            query_ty,
            vec![
                ("snapshot", snapshot(finality, first, second)),
                ("dependent", scalar(query_ty, "dependent", dependent)),
            ],
        );
        validate_native_invariants(&query, &query_ty.invariants).unwrap();
        let context_ty = ty("LanguageParserWindow8DependencyFactContext");
        let context = record(
            context_ty,
            vec![
                ("query", query),
                ("head", scalar(context_ty, "head", head)),
                ("relation", relation(base).into_structured().unwrap()),
                (
                    "dependent_end",
                    scalar(context_ty, "dependent_end", dependent_end),
                ),
                ("head_end", scalar(context_ty, "head_end", head_end)),
            ],
        );
        validate_native_invariants(&context, &context_ty.invariants)?;
        let fact_ty = ty("LanguageParserWindow8StableDependencyFact");
        validate_native_invariants(
            &record(fact_ty, vec![("context", context)]),
            &fact_ty.invariants,
        )
    };
    assert!(fact(
        LanguageTextFinality::Partial,
        &state,
        &state,
        0,
        1,
        LanguageUniversalDependencyRelation::Det,
        6,
        13
    )
    .is_ok());
    assert!(fact(
        LanguageTextFinality::Partial,
        &subject,
        &subject,
        0,
        1,
        LanguageUniversalDependencyRelation::Nsubj,
        6,
        13
    )
    .is_err());
    assert!(fact(
        LanguageTextFinality::Final,
        &subject,
        &subject,
        0,
        1,
        LanguageUniversalDependencyRelation::Nsubj,
        6,
        13
    )
    .is_ok());
    assert!(fact(
        LanguageTextFinality::Partial,
        &state,
        &state,
        1,
        8,
        LanguageUniversalDependencyRelation::Root,
        13,
        0
    )
    .is_err());
    assert!(fact(
        LanguageTextFinality::Final,
        &state,
        &state,
        1,
        8,
        LanguageUniversalDependencyRelation::Root,
        13,
        0
    )
    .is_ok());
    assert!(fact(
        LanguageTextFinality::Partial,
        &state,
        &subject,
        0,
        1,
        LanguageUniversalDependencyRelation::Det,
        6,
        13
    )
    .is_err());
    assert!(fact(
        LanguageTextFinality::Partial,
        &state,
        &state,
        0,
        1,
        LanguageUniversalDependencyRelation::Det,
        5,
        13
    )
    .is_err());
}
