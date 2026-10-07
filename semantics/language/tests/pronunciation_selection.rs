use conduit_language::{pronunciation_selection::*, *};
use conduit_plot::rust_binding::BoundedSequence;
#[path = "common/pronunciation_fixture.rs"]
mod fixture;
#[test]
fn supplied_profile_selects_exact_ambiguous_occurrence_without_word_inference() {
    for (text, relation, pos) in [
        (
            "a record",
            LanguageUniversalDependencyRelation::Det,
            LanguageLexicalPos::Noun,
        ),
        (
            "we record",
            LanguageUniversalDependencyRelation::Nsubj,
            LanguageLexicalPos::Verb,
        ),
    ] {
        let lexical = fixture::lexical(text);
        let arc = fixture::arc(&lexical, relation);
        let selected = prepare_pronunciation_selection(
            &lexical,
            1,
            &fixture::analysis(),
            &arc,
            &fixture::profile(),
        )
        .unwrap();
        assert_eq!(selected.candidate().pos(), &pos);
        assert_eq!(selected.request().basis(), &arc);
        assert_eq!(selected.request().token().candidates().len(), 2);
        assert_eq!(selected.request().source(), lexical.tape().source());
        assert_eq!(selected.lexical_profile(), lexical.tape().profile());
    }
}
#[test]
fn foreign_stale_unknown_and_duplicate_policy_refuse() {
    let lexical = fixture::lexical("a record");
    let arc = fixture::arc(&lexical, LanguageUniversalDependencyRelation::Det);
    let foreign = fixture::lexical("we record");
    assert!(matches!(
        prepare_pronunciation_selection(
            &foreign,
            1,
            &fixture::analysis(),
            &arc,
            &fixture::profile()
        ),
        Err(PronunciationSelectionRefusal::Arc)
    ));
    assert!(prepare_pronunciation_selection(
        &lexical,
        1,
        &LanguageAnalysisRevisionId::new("stale".into()).unwrap(),
        &arc,
        &fixture::profile()
    )
    .is_err());
    let unknown = fixture::arc(&lexical, LanguageUniversalDependencyRelation::Obj);
    assert!(matches!(
        prepare_pronunciation_selection(
            &lexical,
            1,
            &fixture::analysis(),
            &unknown,
            &fixture::profile()
        ),
        Err(PronunciationSelectionRefusal::Policy)
    ));
    let original = fixture::profile();
    let duplicate = LanguagePronunciationSelectionProfile::new(
        original.identity().clone(),
        original.language().clone(),
        original.provenance().clone(),
        BoundedSequence::try_from_iter([original.rules()[0].clone(), original.rules()[0].clone()])
            .unwrap(),
    )
    .unwrap();
    assert!(matches!(
        prepare_pronunciation_selection(&lexical, 1, &fixture::analysis(), &arc, &duplicate),
        Err(PronunciationSelectionRefusal::Policy)
    ));
}

#[test]
fn duplicate_candidates_and_wrong_target_occurrence_are_not_silently_chosen() {
    let lexical = fixture::lexical("a record");
    let arc = fixture::arc(&lexical, LanguageUniversalDependencyRelation::Det);
    assert!(prepare_pronunciation_selection(
        &lexical,
        0,
        &fixture::analysis(),
        &arc,
        &fixture::profile()
    )
    .is_err());
    let profile = LanguageLexicalProfile::new(
        BoundedSequence::try_from_iter([LanguageLexicalEntry::new(
            BoundedSequence::try_from_iter([
                fixture::candidate(LanguageLexicalPos::Noun),
                fixture::candidate(LanguageLexicalPos::Noun),
            ])
            .unwrap(),
            "record".into(),
        )
        .unwrap()])
        .unwrap(),
        "lexical/duplicate-candidates".into(),
        lexical.tape().profile().language().clone(),
        fixture::provenance(),
    )
    .unwrap();
    let duplicate =
        conduit_language::lexical::prepare_lexical_tape(lexical.tape().source(), &profile, None)
            .unwrap();
    assert!(matches!(
        prepare_pronunciation_selection(
            &duplicate,
            1,
            &fixture::analysis(),
            &arc,
            &fixture::profile()
        ),
        Err(PronunciationSelectionRefusal::Ambiguity)
    ));
}

#[test]
fn selection_schemas_are_admitted_by_installed_language_catalog() {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profiles = conduit_plot::ProfileCatalog::new();
    install_linguistics_catalogs(&mut startup, &mut profiles).unwrap();
    for (name, _) in pronunciation_selection_types() {
        let source =
            format!("plot pronunciation/catalog (\n >> value: {name}\n result: {name} >>\n) = (.)");
        conduit_plot::check_syntax_document(
            &conduit_plot::parse_syntax_document(&source),
            &startup,
        )
        .unwrap();
    }
}
