#[path = "common/prosody.rs"]
mod fixture;
use conduit_language::{prosody::*, *};
#[test]
fn exact_vocative_bases_select_profile_data_in_all_three_positions() {
    for (text, dependent, head) in [
        ("Travis hello friend", 0, 1),
        ("hello Travis friend", 1, 0),
        ("hello friend Travis", 2, 0),
    ] {
        let fixture = fixture::fixture(text, dependent, head);
        let prepared = prepare_rich_prosody(
            &fixture.lexical,
            dependent as usize,
            fixture.discourse.fact(),
            &fixture.profile,
        )
        .unwrap();
        assert_eq!(prepared.accepted().choice(), fixture.profile.vocative());
        assert_eq!(
            prepared.requested().token().identity(),
            fixture.discourse.fact().basis().dependent().token()
        );
        assert_eq!(
            prepared.requested().lexical_profile(),
            fixture.lexical.tape().profile()
        );
        assert_eq!(prepared.requested().token().candidates().len(), 1);
        let wrong = LanguageRichProsodyAccepted::new(
            fixture.profile.fallback().clone(),
            prepared.requested().clone(),
        );
        assert!(wrong.is_err());
        let fallback =
            prepare_fallback_prosody(&fixture.lexical, dependent as usize, &fixture.profile)
                .unwrap();
        assert_eq!(fallback.accepted().choice(), fixture.profile.fallback());
        assert_ne!(fallback.accepted().choice(), prepared.accepted().choice());
    }
}
#[test]
fn punctuation_is_not_the_rich_selection_basis_and_foreign_occurrences_refuse() {
    let fixture = fixture::fixture("Hello, Travis.", 2, 0);
    let prepared = prepare_rich_prosody(
        &fixture.lexical,
        2,
        fixture.discourse.fact(),
        &fixture.profile,
    )
    .unwrap();
    assert_eq!(prepared.accepted().choice(), fixture.profile.vocative());
    assert!(matches!(
        prepare_rich_prosody(
            &fixture.lexical,
            0,
            fixture.discourse.fact(),
            &fixture.profile
        ),
        Err(ProsodyRefusal::Native(_))
    ));
    assert!(matches!(
        prepare_rich_prosody(
            &fixture.lexical,
            128,
            fixture.discourse.fact(),
            &fixture.profile
        ),
        Err(ProsodyRefusal::Token)
    ));
}
#[test]
fn symbolic_pitch_intent_is_rate_independent_and_accepted_by_language() {
    let fixture = fixture::fixture("Hello Travis", 1, 0);
    let rising = LanguageProsodyProfile::new(
        fixture.profile.fallback().clone(),
        "prosody/rising".into(),
        fixture.profile.language().clone(),
        fixture::provenance(),
        fixture::choice(
            LanguageProsodyBoundary::MinorPhrase,
            LanguageProsodyProminence::Prominent,
            LanguageProsodyPitch::Rising,
        ),
    )
    .unwrap();
    let prepared =
        prepare_rich_prosody(&fixture.lexical, 1, fixture.discourse.fact(), &rising).unwrap();
    assert!(matches!(
        prepared.accepted().choice().pitch(),
        LanguageProsodyPitch::Rising
    ));
}
#[test]
fn prosody_schemas_install_without_changing_identity_family() {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    install_linguistics_catalogs(&mut startup, &mut profile).unwrap();
    for (name, _) in prosody_types() {
        let source = format!(
            "plot language/prosody-fixture (\n >> value: {name}\n result: {name} >>\n) = (.)"
        );
        conduit_plot::check_syntax_document(
            &conduit_plot::parse_syntax_document(&source),
            &startup,
        )
        .unwrap();
    }
}

#[test]
fn punctuation_and_uncompleted_words_are_not_admitted_as_word_prosody() {
    let fixture = fixture::fixture("Hello, Travis.", 2, 0);
    assert!(matches!(
        prepare_fallback_prosody(&fixture.lexical, 1, &fixture.profile),
        Err(ProsodyRefusal::Native(_))
    ));
    let old = fixture.lexical.tape().source();
    let partial = LanguageTextRevision::new(
        LanguageTextFinality::Partial,
        LanguageText::new(
            old.material().identity().clone(),
            old.material().language().clone(),
            LanguageTextRevisionId::new("partial".into()).unwrap(),
            "Hello Trav".into(),
        )
        .unwrap(),
        None,
        fixture::provenance(),
        0,
        None,
    )
    .unwrap();
    let lexical = conduit_language::lexical::prepare_lexical_tape(
        &partial,
        fixture.lexical.tape().profile(),
        None,
    )
    .unwrap();
    assert!(matches!(
        prepare_fallback_prosody(&lexical, 1, &fixture.profile),
        Err(ProsodyRefusal::Native(_))
    ));
}

#[test]
fn same_identifier_with_different_source_material_and_foreign_language_refuse() {
    let fixture = fixture::fixture("Hello Travis", 1, 0);
    let old = fixture.lexical.tape().source();
    let source = LanguageTextRevision::new(
        LanguageTextFinality::Final,
        LanguageText::new(
            old.material().identity().clone(),
            old.material().language().clone(),
            old.material().revision().clone(),
            "Hola Travis".into(),
        )
        .unwrap(),
        None,
        fixture::provenance(),
        0,
        None,
    )
    .unwrap();
    let lexical = conduit_language::lexical::prepare_lexical_tape(
        &source,
        fixture.lexical.tape().profile(),
        None,
    )
    .unwrap();
    assert!(matches!(
        prepare_rich_prosody(&lexical, 1, fixture.discourse.fact(), &fixture.profile),
        Err(ProsodyRefusal::Source)
    ));
    assert!(LanguageRichProsodyRequest::new(
        fixture.discourse.fact().clone(),
        lexical.tape().profile().clone(),
        fixture.profile.clone(),
        source,
        lexical.tape().tokens()[1].clone()
    )
    .is_err());
    let foreign = LanguageProsodyProfile::new(
        fixture.profile.fallback().clone(),
        "prosody/fr".into(),
        LanguageId::new("language/fr".into()).unwrap(),
        fixture::provenance(),
        fixture.profile.vocative().clone(),
    )
    .unwrap();
    assert!(matches!(
        prepare_rich_prosody(&fixture.lexical, 1, fixture.discourse.fact(), &foreign),
        Err(ProsodyRefusal::Native(_))
    ));
}
