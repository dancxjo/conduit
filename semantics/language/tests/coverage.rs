use conduit_language::*;
use conduit_plot::rust_binding::BoundedSequence;

fn language(name: &str) -> LanguageId {
    LanguageId::new(name.into()).unwrap()
}
fn declaration(
    languages: Vec<LanguageId>,
    varieties: Vec<LanguageVariety>,
    mappings: Vec<LanguageMappingDeclaration>,
) -> LanguageCoverage {
    LanguageCoverage::new(
        "proof/fixture".into(),
        BoundedSequence::try_from_iter(languages).unwrap(),
        BoundedSequence::try_from_iter(mappings).unwrap(),
        "fixture@1".into(),
        BoundedSequence::try_from_iter(varieties).unwrap(),
        false,
    )
    .unwrap()
}
#[test]
fn duplicate_and_incoherent_declarations_cannot_be_advertised_as_coverage() {
    let en = language("English");
    let fr = language("French");
    let variety = LanguageVariety::new(
        VarietyId::new("session/variety".into()).unwrap(),
        fr.clone(),
    )
    .unwrap();
    let duplicate = declaration(vec![en.clone(), en.clone()], vec![], vec![]);
    assert_eq!(
        language_coverage_property(duplicate),
        Err(LanguageCoverageRefusal::MalformedDeclaration)
    );
    let wrong_language = declaration(vec![en.clone()], vec![variety.clone()], vec![]);
    assert_eq!(
        validate_language_coverage(&wrong_language),
        Err(LanguageCoverageRefusal::MalformedDeclaration)
    );
    let external = LanguageExternalIdentity::new("model/exact@1".into(), "row-17".into()).unwrap();
    let row = LanguageMappingDeclaration::new(external, en.clone(), Some(variety.clone())).unwrap();
    let wrong_mapping = declaration(vec![en.clone(), fr], vec![variety.clone()], vec![row]);
    assert_eq!(
        validate_language_coverage(&wrong_mapping),
        Err(LanguageCoverageRefusal::MalformedDeclaration)
    );
    let request =
        LanguageRequest::new(en, Some(variety), LanguageVarietyPolicy::LanguageSufficient).unwrap();
    assert_eq!(
        language_request_configuration(request),
        Err(LanguageCoverageRefusal::VarietyLanguage)
    );
}
#[test]
fn exact_variety_requires_an_identity_and_metadata_limits_are_native_bounds() {
    let request = LanguageRequest::new(
        language("unknown/valid"),
        None,
        LanguageVarietyPolicy::ExactVariety,
    )
    .unwrap();
    assert_eq!(
        validate_language_request(&request),
        Err(LanguageCoverageRefusal::MissingVariety)
    );
    assert!(BoundedSequence::<LanguageId, 64>::try_from_iter(
        (0..65).map(|n| language(&format!("language/{n}")))
    )
    .is_err());
    let request = LanguageRequest::new(
        language("unknown/valid"),
        None,
        LanguageVarietyPolicy::LanguageSufficient,
    )
    .unwrap();
    assert!(language_request_configuration(request.clone()).is_ok());
    assert_eq!(
        admit_language_coverage(&request, None),
        Err(LanguageCoverageRefusal::Undeclared)
    );
}
