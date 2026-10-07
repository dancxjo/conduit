//! External oracle rows are TRAIN-only references. Native Source must derive
//! every feature and graph mutation independently; this is no learned decode.
use conduit_language::{
    parser_window8::{self, lexical::*},
    *,
};
use conduit_plot::rust_binding::BoundedSequence;
use serde_json::Value;
fn provenance() -> LinguisticDerivationProvenance {
    LinguisticDerivationProvenance::deterministic_rule(
        "window8/reference-parity".into(),
        "reviewed/3".into(),
    )
    .unwrap()
}
fn profile() -> LanguageLexicalProfile {
    let data: Value = serde_json::from_str(include_str!(
        "../training/ewt_joint_v3_window8/reviewed_lexical_alternatives.json"
    ))
    .unwrap();
    let pos = [
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
    ];
    let entries = data.as_object().unwrap().iter().map(|(word, codes)| {
        let candidates = codes.as_array().unwrap().iter().map(|code| {
            LanguageLexicalCandidate::new(
                word.clone(),
                BoundedSequence::new(),
                pos[code.as_u64().unwrap() as usize],
            )
            .unwrap()
        });
        LanguageLexicalEntry::new(
            BoundedSequence::try_from_iter(candidates).unwrap(),
            word.clone(),
        )
        .unwrap()
    });
    LanguageLexicalProfile::new(
        BoundedSequence::try_from_iter(entries).unwrap(),
        "window8/reviewed-parity".into(),
        LanguageId::new("language/en".into()).unwrap(),
        provenance(),
    )
    .unwrap()
}
fn state_matches(actual: &LanguageParserWindow8RawState, expected: &Value) {
    assert_eq!(*actual.unread(), expected["unread"].as_u64().unwrap());
    let stack = expected["stack"].as_array().unwrap();
    assert_eq!(*actual.depth(), stack.len() as u64);
    for (ordinal, value) in stack.iter().enumerate() {
        assert_eq!(actual.stack()[ordinal], value.as_u64().unwrap());
    }
    for (ordinal, value) in expected["heads"].as_array().unwrap().iter().enumerate() {
        assert_eq!(actual.heads()[ordinal], value.as_u64().unwrap());
    }
    let relations = [
        actual.relation0(),
        actual.relation1(),
        actual.relation2(),
        actual.relation3(),
        actual.relation4(),
        actual.relation5(),
        actual.relation6(),
        actual.relation7(),
    ];
    for (ordinal, value) in expected["relations"].as_array().unwrap().iter().enumerate() {
        assert_eq!(
            format!("{:?}", relations[ordinal].base()).to_lowercase(),
            value.as_str().unwrap().split(':').next().unwrap()
        );
        assert_eq!(relations[ordinal].subtype().get().as_str(), "");
    }
}
#[test]
fn eight_train_reference_oracles_have_native_source_feature_and_transition_parity() {
    let rows: Value = serde_json::from_str(include_str!(
        "../training/ewt_joint_v3_window8/oracle_reference.json"
    ))
    .unwrap();
    let profile = profile();
    let profile_codes: Value = serde_json::from_str(include_str!(
        "../training/ewt_joint_v3_window8/reviewed_lexical_alternatives.json"
    ))
    .unwrap();
    for row in rows.as_array().unwrap() {
        let id = row["id"].as_str().unwrap();
        let source = LanguageTextRevision::new(
            LanguageTextFinality::Final,
            LanguageText::new(
                LanguageTextId::new(format!("window8/{id}")).unwrap(),
                LanguageId::new("language/en".into()).unwrap(),
                LanguageTextRevisionId::new(format!("window8/{id}/r0")).unwrap(),
                row["text"].as_str().unwrap().into(),
            )
            .unwrap(),
            None,
            provenance(),
            0,
            None,
        )
        .unwrap();
        let tape =
            conduit_language::lexical::prepare_lexical_tape(&source, &profile, None).unwrap();
        let lexical = prepare_window8_lexical(&tape).unwrap();
        assert_eq!(
            *lexical.lexical().token_count(),
            row["forms"].as_array().unwrap().len() as u64
        );
        let basis = LanguageParserBasis::new(
            LanguageAnalysisRevisionId::new(format!("window8/{id}/reference")).unwrap(),
            source.material().revision().clone(),
            source.material().identity().clone(),
        )
        .unwrap();
        let default = LanguageParserRelation::new(
            LanguageUniversalDependencyRelation::Dep,
            LanguageParserSubtype::new("".into()).unwrap(),
        )
        .unwrap();
        let mut state = parser_window8::initialize_window8(
            &LanguageParserWindow8Begin::new(
                basis.clone(),
                default.clone(),
                *lexical.lexical().token_count(),
            )
            .unwrap(),
        )
        .unwrap();
        let mut choices = [0; 8];
        for (ordinal, form) in row["forms"].as_array().unwrap().iter().enumerate() {
            let codes = profile_codes[form.as_str().unwrap()].as_array().unwrap();
            choices[ordinal] = codes
                .iter()
                .position(|code| code == &row["pos"][ordinal])
                .unwrap() as u64;
        }
        for reference in row["steps"].as_array().unwrap() {
            state_matches(state.state(), &reference["before"]);
            let features = prepare_window8_features(&state, &lexical, &basis, choices).unwrap();
            let expected: Vec<_> = reference["indices"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_u64().unwrap())
                .collect();
            assert_eq!(
                features.features().raw().indices().as_slice(),
                expected,
                "{id}"
            );
            let class =
                parser_window8::window8_class(reference["code"].as_u64().unwrap(), &default)
                    .unwrap();
            let step = parser_window8::prepare_window8_step(
                &state,
                &basis,
                *class.action(),
                class.relation(),
            )
            .unwrap();
            assert!(step.accepted(), "{id}");
            state = step.next().clone();
            state_matches(state.state(), &reference["after"]);
        }
        assert!(parser_window8::window8_complete(&state).unwrap(), "{id}");
        assert_eq!(*state.state().committed(), 0);
    }
    assert!(parser_window8::window8_class(
        76,
        &LanguageParserRelation::new(
            LanguageUniversalDependencyRelation::Dep,
            LanguageParserSubtype::new("".into()).unwrap()
        )
        .unwrap()
    )
    .is_err());
}
