use conduit_language::*;

fn external(contract: &str, name: &str) -> LanguageExternalIdentity {
    LanguageExternalIdentity::new(contract.into(), name.into()).unwrap()
}
fn selection(language: &str) -> LanguageSelection {
    LanguageSelection::new(LanguageId::new(language.into()).unwrap(), None).unwrap()
}

#[test]
fn exact_external_contract_and_name_choose_explicit_language_and_variety() {
    let french = LanguageId::new("language/french".into()).unwrap();
    let variety = LanguageVariety::new(
        VarietyId::new("session/quebec-profile".into()).unwrap(),
        french.clone(),
    )
    .unwrap();
    let rows = [
        LanguageMappingRow {
            external: external("model/fixture-v3", "row-42"),
            target: LanguageSelection::new(french.clone(), Some(variety.clone())).unwrap(),
        },
        LanguageMappingRow {
            external: external("bcp47/fixture@1", "fr-CA"),
            target: LanguageSelection::new(french, Some(variety)).unwrap(),
        },
        LanguageMappingRow {
            external: external("model/other", "row-42"),
            target: selection("language/english"),
        },
    ];
    let mapping = LanguageMapping::new(&rows).unwrap();
    let private = mapping
        .resolve(&external("model/fixture-v3", "row-42"))
        .unwrap();
    assert_eq!(private.language().get(), "language/french");
    assert_eq!(
        private.variety().unwrap().identity().get(),
        "session/quebec-profile"
    );
    assert_eq!(
        private,
        mapping
            .resolve(&external("bcp47/fixture@1", "fr-CA"))
            .unwrap()
    );
    for unknown in [
        external("model/fixture-v3", "42"),
        external("bcp47/fixture@1", "fr"),
        external("bcp47/fixture@1", "fr-CA-private"),
        external("model/unlisted", "row-42"),
    ] {
        assert_eq!(
            mapping.resolve(&unknown),
            Err(LanguageMappingRefusal::Undeclared)
        );
    }
}

#[test]
fn ambiguous_and_unbounded_tables_and_foreign_variety_refuse() {
    let row = LanguageMappingRow {
        external: external("provider@1", "native"),
        target: selection("language/english"),
    };
    assert!(matches!(
        LanguageMapping::new(&[row.clone(), row.clone()]),
        Err(LanguageMappingRefusal::Duplicate)
    ));
    assert!(matches!(
        LanguageMapping::new(&vec![row; MAXIMUM_LANGUAGE_MAPPING_ROWS + 1]),
        Err(LanguageMappingRefusal::Bound)
    ));
    let foreign = LanguageVariety::new(
        VarietyId::new("profile/fr".into()).unwrap(),
        LanguageId::new("language/french".into()).unwrap(),
    )
    .unwrap();
    assert_eq!(
        LanguageSelection::new(
            LanguageId::new("language/english".into()).unwrap(),
            Some(foreign)
        ),
        Err(LanguageMappingRefusal::VarietyLanguage)
    );
}
