//! Generated Native custody for an early lexical choice before dependency closure.
use conduit_language::{
    parser_window8::{self, lexical::*},
    *,
};
use conduit_plot::rust_binding::BoundedSequence;

#[test]
fn generated_lexical_fact_admits_agreement_and_refuses_changed_basis_choice_or_prefix() {
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
            hypothesis(0),
            hypothesis(second),
            hypothesis(0),
            hypothesis(0),
            lexical.clone(),
        )
    };
    let admit = |snapshot| {
        LanguageParserWindow8StableLexicalFact::new(
            LanguageParserWindow8FactQuery::new(0, snapshot).unwrap(),
        )
    };
    let agreed = admit(snapshot(0, lexical.lexical(), &basis).unwrap()).unwrap();
    assert_eq!(*agreed.query().dependent(), 0);
    {
        use conduit_plot::rust_binding::{
            NativeBindingRefusal, NativeRustBinding, PreparedNativeFamilyLimits,
        };
        let mut prepared = conduit_language::prepared_stable_lexical_fact::PreparedStableLexicalFactAdmission::prepare(
            PreparedNativeFamilyLimits {
                maximum_types: 64,
                maximum_laws_per_type: 64,
                maximum_input_bytes: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES,
                maximum_retained_bytes: 256 * 1024 * 1024,
                maximum_preparation_peak_bytes: 512 * 1024 * 1024,
                maximum_conversion_requested_bytes: 1024 * 1024 * 1024,
            },
        ).unwrap();
        let canonical = agreed.clone().encode().unwrap();
        assert_eq!(prepared.decode(&canonical).unwrap(), agreed);
        let structured = agreed.clone().into_structured().unwrap();
        assert_eq!(prepared.convert_structured(&structured).unwrap(), agreed);
        assert_eq!(prepared.storage_receipt().types, 35);
        let changed_query =
            LanguageParserWindow8FactQuery::new(0, snapshot(1, lexical.lexical(), &basis).unwrap())
                .unwrap();
        let invalid = conduit_core::StructuredInfoValue::record(
            LanguageParserWindow8StableLexicalFact::semantic_type().unwrap(),
            vec![conduit_core::StructuredFieldValue::new(
                "query",
                changed_query.into_structured().unwrap(),
            )
            .unwrap()],
        )
        .unwrap();
        let reference = LanguageParserWindow8StableLexicalFact::from_structured(invalid.clone());
        assert_eq!(
            reference.as_ref().err(),
            Some(&NativeBindingRefusal::ViolatedInvariant { index: 3 })
        );
        assert_eq!(prepared.convert_structured(&invalid), reference);

        assert!(prepared.decode(&canonical[..canonical.len() - 1]).is_err());
        assert!(prepared
            .decode(&agreed.query().clone().encode().unwrap())
            .is_err());
    }

    assert!(admit(snapshot(1, lexical.lexical(), &basis).unwrap()).is_err());
    let foreign_basis = LanguageParserBasis::new(
        LanguageAnalysisRevisionId::new("window8/foreign-analysis".into()).unwrap(),
        basis.source_revision().clone(),
        basis.text().clone(),
    )
    .unwrap();
    assert!(snapshot(0, lexical.lexical(), &foreign_basis).is_err());
    let unstable_source = LanguageTextRevision::new(
        *source.finality(),
        source.material().clone(),
        None,
        source.provenance().clone(),
        0,
        Some(0),
    )
    .unwrap();
    let unstable_tape =
        conduit_language::lexical::prepare_lexical_tape(&unstable_source, &profile, None).unwrap();
    let unstable = prepare_window8_lexical(&unstable_tape).unwrap();
    assert!(admit(snapshot(0, unstable.lexical(), &basis).unwrap()).is_err());
}
