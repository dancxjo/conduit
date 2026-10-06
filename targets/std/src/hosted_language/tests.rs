//! Shared finite declarations for repository-owned adapter proof fixtures.
use conduit_language::*;
use conduit_plot::rust_binding::BoundedSequence;

pub(crate) fn request(language: &str) -> LanguageRequest {
    LanguageRequest::new(
        LanguageId::new(language.into()).unwrap(),
        None,
        LanguageVarietyPolicy::LanguageSufficient,
    )
    .unwrap()
}

pub(crate) fn fixture_coverage(
    artifact: &str,
    private_name: &str,
    language: &str,
) -> LanguageCoverage {
    let language = LanguageId::new(language.into()).unwrap();
    let row = LanguageMappingDeclaration::new(
        LanguageExternalIdentity::new(artifact.into(), private_name.into()).unwrap(),
        language.clone(),
        None,
    )
    .unwrap();
    LanguageCoverage::new(
        artifact.into(),
        BoundedSequence::try_from_iter([language]).unwrap(),
        BoundedSequence::try_from_iter([row]).unwrap(),
        "supplied-fixture@1".into(),
        BoundedSequence::try_from_iter([]).unwrap(),
        false,
    )
    .unwrap()
}

#[test]
fn language_refusals_keep_distinct_finite_host_call_details() {
    use super::HostedLanguageRefusal as H;
    let refusals = [
        H::Coverage(LanguageCoverageRefusal::Undeclared),
        H::Coverage(LanguageCoverageRefusal::Language),
        H::Coverage(LanguageCoverageRefusal::Variety),
        H::Coverage(LanguageCoverageRefusal::MissingVariety),
        H::Coverage(LanguageCoverageRefusal::VarietyLanguage),
        H::Coverage(LanguageCoverageRefusal::MalformedDeclaration),
        H::Coverage(LanguageCoverageRefusal::Bound),
        H::Configuration,
        H::Mapping,
        H::Artifact,
    ];
    for (index, refusal) in refusals.iter().enumerate() {
        assert!(refusal.host_detail() > 0);
        assert!(refusals[..index]
            .iter()
            .all(|previous| previous.host_detail() != refusal.host_detail()));
        let failure =
            crate::hosted_speech_synthesis::EspeakFailure::Language(*refusal).host_failure();
        assert_eq!(failure.0, conduit_kernel::HostCallDisposition::Denied);
        assert_eq!(failure.1.detail, refusal.host_detail());
    }
}
