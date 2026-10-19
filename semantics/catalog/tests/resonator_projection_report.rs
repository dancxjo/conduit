#![cfg(feature = "plot-catalog")]
use conduit_core::projection::*;
use conduit_plot::PortableExpressionProgram;
use conduit_semantic_catalog::*;
#[allow(dead_code)]
#[path = "../../../architecture/plot/tests/common/resonator.rs"]
mod fixture;
fn text(s: &str) -> ProjectionText<'_> {
    ProjectionText::new(s).unwrap()
}
#[test]
fn actual_public_source_coefficients_retain_evidence_and_refuse_exact_acoustic_fidelity() {
    let prepared = PreparedResonatorNumericProjection::new().unwrap();
    let original = fixture::request(prepared.input_type(), "", (1000, 1), (100, 1), 8000);
    let canonical = original.canonical_bytes().unwrap();
    let receipt = prepared.project(&canonical).unwrap();
    assert_eq!(receipt.original(), &original);
    assert_eq!(receipt.original_canonical(), canonical);
    assert_eq!(receipt.executions().len(), 24);
    assert!(!receipt.source_id().is_empty());
    assert!(!receipt.checked_plot_id().is_empty());
    assert!(!receipt.expanded_plot_id().is_empty());
    for execution in receipt.executions() {
        assert_eq!(
            PortableExpressionProgram::from_canonical_hex(execution.program_hex())
                .unwrap()
                .evaluate(execution.input_canonical())
                .unwrap(),
            execution.output_canonical()
        );
    }
    let domain = ResonatorNumericProjection::new(&receipt).unwrap();
    assert_eq!(
        domain.target().b_q14(),
        fixture::integer(receipt.candidate(), "b_q14")
    );
    assert_eq!(
        domain.target().c_q14(),
        fixture::integer(receipt.candidate(), "c_q14")
    );
    let facts = domain.facts();
    let policy = ResonatorPolynomialPolicy;
    let input = || ProjectionInput {
        basis: ProjectionBasis {
            report: text("report"),
            source: text("source"),
            target: Some(text("target")),
            projector: text("audio/public-resonator-source"),
            requested_route: text("q14"),
            boundary: None,
        },
        source: domain.source(),
        target: Some(domain.target()),
        facts: &facts,
        native: &[],
        scores: &[],
        admitted: &[],
        attempts: &[],
        selected_attempt: None,
        mechanism: ProjectionMechanism::Completed,
        diagnostics: &[],
    };
    let report = ProjectionReport::new(&domain, &policy, input()).unwrap();
    assert_eq!(
        report.summary().disposition,
        ProjectionDisposition::Completed(ProjectionFidelity::PermittedLossy)
    );
    assert_eq!(
        report.require_exact(),
        Err(ProjectionRefusal::ConsumerRequiresExact)
    );
    assert!(report
        .require_policy(text("audio/explicit-polynomial-q30-q14@1"))
        .is_ok());
    let ProjectionFact::Lost { detail, .. } = &facts[3] else {
        panic!("declared approximation")
    };
    assert_eq!(detail.certified_per_request_bound(), None);
    assert!(detail.corpus_center_tolerance().contains("corpus only"));
    let mut omitted = input();
    omitted.facts = &facts[..3];
    assert!(ProjectionReport::new(&domain, &policy, omitted).is_err());
    let forged = [
        ProjectionFact::Preserved {
            obligation: text("full-original-request-context"),
        },
        ProjectionFact::Transformed {
            obligation: text("source-profile-eligibility-and-stability"),
            law: text("audio/owned-resonator-prepare-started@1"),
        },
        ProjectionFact::Transformed {
            obligation: text("declared-q30-q14-rounding-policy"),
            law: text("audio/source-nine-term-polynomial-q14@1"),
        },
        ProjectionFact::Preserved {
            obligation: text("continuous-acoustic-coefficient-interpretation"),
        },
    ];
    let mut invalid = input();
    invalid.facts = &forged;
    assert!(ProjectionReport::new(&domain, &policy, invalid).is_err());
}
#[test]
fn refused_ranges_and_foreign_or_truncated_carriers_cannot_become_numeric_receipts() {
    let prepared = PreparedResonatorNumericProjection::new().unwrap();
    for (center, bandwidth, rate) in [
        ((1, 1), (100, 1), 8000),
        ((1000, 1), (1, 1), 8000),
        ((1000, 1), (100, 1), 1),
    ] {
        let original = fixture::request(prepared.input_type(), "", center, bandwidth, rate);
        assert!(prepared
            .project(&original.canonical_bytes().unwrap())
            .is_err());
    }
    assert!(prepared.project(&[]).is_err());
    let original = fixture::request(prepared.input_type(), "", (1000, 1), (100, 1), 8000)
        .canonical_bytes()
        .unwrap();
    assert!(prepared.project(&original[..original.len() - 1]).is_err());
}
