#![cfg(feature = "semantic-bindings")]
use conduit_core::projection::*;
use conduit_language::*;
use conduit_speech::language_projection::*;
fn t(s: &str) -> ProjectionText<'_> {
    ProjectionText::new(s).unwrap()
}

#[test]
fn asr_language_hypothesis_consumes_exact_projection_and_keeps_variety() {
    let source =
        LanguageExternalIdentity::new("asr/model-v2".into(), "fr-native-row".into()).unwrap();
    let language = LanguageId::new("language/french".into()).unwrap();
    let variety = LanguageVariety::new(
        VarietyId::new("profile/session-fr".into()).unwrap(),
        language.clone(),
    )
    .unwrap();
    let target = LanguageSelection::new(language.clone(), Some(variety.clone())).unwrap();
    let rows = [LanguageMappingRow {
        external: source.clone(),
        target: target.clone(),
    }];
    let mapping = LanguageMapping::new(&rows).unwrap();
    let domain = LanguageMappingProjection {
        mapping: &mapping,
        law: t("asr/mapping-v2"),
    };
    let policy = ExactLanguageMappingPolicy;
    let facts = [ProjectionFact::Transformed {
        obligation: t("language-identity"),
        law: domain.law,
    }];
    let native = [ProjectionNativeFact {
        identity: t("external-language"),
        contract: t(source.contract()),
        provider: t(source.contract()),
        encoding: t("utf8@1"),
        bytes: source.name().as_bytes(),
    }];
    let report = ProjectionReport::new(
        &domain,
        &policy,
        ProjectionInput {
            basis: ProjectionBasis {
                report: t("report/asr"),
                source: t("hypothesis/1"),
                target: Some(t("selection/1")),
                projector: domain.law,
                requested_route: t("asr/fixture"),
                boundary: None,
            },
            source: &source,
            target: Some(&target),
            facts: &facts,
            native: &native,
            scores: &[],
            admitted: &[],
            attempts: &[],
            selected_attempt: None,
            mechanism: ProjectionMechanism::Completed,
            diagnostics: &[],
        },
    )
    .unwrap();
    let hypothesis = project_language_hypothesis(&report, None).unwrap();
    assert_eq!(hypothesis.language(), &language);
    assert_eq!(hypothesis.variety(), &Some(variety.identity().clone()));
    assert!(hypothesis.confidence().is_none());
    // A forged portable target cannot enter listening through this report.
    let forged =
        LanguageSelection::new(LanguageId::new("language/english".into()).unwrap(), None).unwrap();
    assert!(ProjectionReport::new(
        &domain,
        &policy,
        ProjectionInput {
            basis: ProjectionBasis {
                report: t("report/forged"),
                source: t("hypothesis/1"),
                target: Some(t("selection/forged")),
                projector: domain.law,
                requested_route: t("asr/fixture"),
                boundary: None
            },
            source: &source,
            target: Some(&forged),
            facts: &facts,
            native: &native,
            scores: &[],
            admitted: &[],
            attempts: &[],
            selected_attempt: None,
            mechanism: ProjectionMechanism::Completed,
            diagnostics: &[],
        }
    )
    .is_err());
}
