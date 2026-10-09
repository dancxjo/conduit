use super::*;
use conduit_core::projection::*;
fn text(value: &str) -> ProjectionText<'_> {
    ProjectionText::new(value).unwrap()
}
fn report_basis() -> ProjectionBasis<'static> {
    ProjectionBasis {
        report: text("rate-projection-test"),
        source: text("exact-authored-request"),
        target: Some(text("declared-integer-grid")),
        projector: text("audio/checked-source"),
        requested_route: text("declared-rate-only"),
        boundary: None,
    }
}
fn check_report<R: AudioFrameProjectionProof>(receipt: &R, expected: ProjectionFidelity) {
    let domain = AudioFrameGridProjection::new(receipt);
    let facts = domain.facts();
    let native = domain.native();
    let policy = AudioFloorFrameGridPolicy;
    let input = ProjectionInput {
        basis: report_basis(),
        source: receipt.source(),
        target: Some(receipt.target()),
        facts: &facts,
        native: &native,
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
        ProjectionDisposition::Completed(expected)
    );
    assert_eq!(
        report.require_exact().is_ok(),
        expected == ProjectionFidelity::Exact
    );
}
#[test]
fn core_reports_distinguish_integer_grid_loss_from_retained_exact_source() {
    let prepared = PreparedAudioSampleProjection::new().unwrap();
    let exact = prepared
        .project(&request(1, 10, 8000).encode().unwrap())
        .unwrap();
    check_report(&exact, ProjectionFidelity::Exact);
    let floored = prepared
        .project(&request(1, 3, 8000).encode().unwrap())
        .unwrap();
    check_report(&floored, ProjectionFidelity::PermittedLossy);
    let cursor = AudioCumulativeFrameCursor::new(
        AudioCumulativeFrameBasis::new(basis(8000), 3).unwrap(),
        2,
        2666,
    )
    .unwrap();
    let cumulative = prepared
        .append(
            &AudioCumulativeFrameRequest::new(request(1, 3, 8000), cursor)
                .unwrap()
                .encode()
                .unwrap(),
        )
        .unwrap();
    check_report(&cumulative, ProjectionFidelity::PermittedLossy);
}
#[test]
fn projection_report_rejects_omission_fabricated_preservation_and_other_loss() {
    let receipt = PreparedAudioSampleProjection::new()
        .unwrap()
        .project(&request(1, 3, 8000).encode().unwrap())
        .unwrap();
    let domain = AudioFrameGridProjection::new(&receipt);
    let native = domain.native();
    let policy = AudioFloorFrameGridPolicy;
    let build = |facts| {
        ProjectionReport::new(
            &domain,
            &policy,
            ProjectionInput {
                basis: report_basis(),
                source: receipt.original(),
                target: Some(receipt.integer_target()),
                facts,
                native: &native,
                scores: &[],
                admitted: &[],
                attempts: &[],
                selected_attempt: None,
                mechanism: ProjectionMechanism::Completed,
                diagnostics: &[],
            },
        )
    };
    let facts = domain.facts();
    assert!(build(&facts[..3]).is_err());
    let mut facts = domain.facts();
    facts[3] = ProjectionFact::Preserved {
        obligation: text("frame-grid-precision"),
    };
    assert!(build(&facts).is_err());
    let mut facts = domain.facts();
    if let ProjectionFact::Lost { class, .. } = &mut facts[3] {
        *class = ProjectionLoss::Approximation;
    }
    assert!(build(&facts).is_err());
    // Exact consumer refuses even a correctly reported, explicitly permitted floor.
    let facts = domain.facts();
    let report = build(&facts).unwrap();
    assert_eq!(
        report.require_exact().err(),
        Some(ProjectionRefusal::ConsumerRequiresExact)
    );
}
