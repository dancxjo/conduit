use conduit_core::projection::*;
use conduit_language::*;
fn t(s: &str) -> ProjectionText<'_> {
    ProjectionText::new(s).unwrap()
}

#[test]
fn provider_private_identity_reports_declared_mapping_and_unknowns_remain_insufficient() {
    let source =
        LanguageExternalIdentity::new("model/fixed-rows-v3".into(), "row-42".into()).unwrap();
    let target =
        LanguageSelection::new(LanguageId::new("language/french".into()).unwrap(), None).unwrap();
    let rows = [LanguageMappingRow {
        external: source.clone(),
        target: target.clone(),
    }];
    let mapping = LanguageMapping::new(&rows).unwrap();
    let domain = LanguageMappingProjection {
        mapping: &mapping,
        law: t("mapping/artifact-v3@1"),
    };
    let policy = ExactLanguageMappingPolicy;
    let known_facts = [ProjectionFact::Transformed {
        obligation: t("language-identity"),
        law: domain.law,
    }];
    let known_native = [ProjectionNativeFact {
        identity: t("external-language"),
        contract: t(source.contract()),
        provider: t(source.contract()),
        encoding: t("utf8@1"),
        bytes: source.name().as_bytes(),
    }];
    let input = ProjectionInput {
        basis: ProjectionBasis {
            report: t("report/known"),
            source: t("native/42"),
            target: Some(t("language/french")),
            projector: domain.law,
            requested_route: t("fixture/mapping"),
            boundary: None,
        },
        source: &source,
        target: Some(&target),
        facts: &known_facts,
        native: &known_native,
        scores: &[],
        admitted: &[],
        attempts: &[],
        selected_attempt: None,
        mechanism: ProjectionMechanism::Completed,
        diagnostics: &[],
    };
    let report = ProjectionReport::new(&domain, &policy, input).unwrap();
    assert_eq!(
        report.summary().disposition,
        ProjectionDisposition::Completed(ProjectionFidelity::Exact)
    );
    assert_eq!(report.summary().transformed, 1);
    assert_eq!(report.native()[0].bytes, b"row-42");
    let unknown = LanguageExternalIdentity::new("model/fixed-rows-v3".into(), "42".into()).unwrap();
    let loss = LanguageMappingRefusal::Undeclared;
    let unknown_facts = [ProjectionFact::Lost {
        obligation: t("language-identity"),
        class: ProjectionLoss::Unrecognized,
        detail: &loss,
        native_fact: Some(t("external-language")),
    }];
    let unknown_native = [ProjectionNativeFact {
        identity: t("external-language"),
        contract: t(unknown.contract()),
        provider: t(unknown.contract()),
        encoding: t("utf8@1"),
        bytes: unknown.name().as_bytes(),
    }];
    let input = ProjectionInput {
        basis: ProjectionBasis {
            report: t("report/unknown"),
            source: t("native/unknown"),
            target: None,
            projector: domain.law,
            requested_route: t("fixture/mapping"),
            boundary: None,
        },
        source: &unknown,
        target: None,
        facts: &unknown_facts,
        native: &unknown_native,
        scores: &[],
        admitted: &[],
        attempts: &[],
        selected_attempt: None,
        mechanism: ProjectionMechanism::Completed,
        diagnostics: &[],
    };
    let report = ProjectionReport::new(&domain, &policy, input).unwrap();
    assert_eq!(
        report.summary().disposition,
        ProjectionDisposition::Insufficient
    );
    assert!(report.inspect_target().is_none());
    assert_eq!(report.native()[0].bytes, b"42");
}
