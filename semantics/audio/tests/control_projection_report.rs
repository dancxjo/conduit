#[allow(dead_code)]
#[path = "trajectory/common.rs"]
mod common;
use common::*;
use conduit_audio::*;
use conduit_core::projection::*;
use conduit_data::{ClockRelation, ClockRelationQuality};
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
fn text(value: &str) -> ProjectionText<'_> {
    ProjectionText::new(value).unwrap()
}
fn clock() -> PreparedAudioClockBinding {
    let r = ClockRelation::new(
        "report-clock".into(),
        ClockRelationQuality::Exact,
        7,
        "seconds".into(),
        1,
        19,
        "ticks".into(),
        1000,
    )
    .unwrap();
    let b = AudioClockBinding::new(
        anchor(1, 1),
        r,
        "seconds".into(),
        7,
        AudioClockSourceUnit::Seconds,
        "ticks".into(),
    )
    .unwrap();
    PreparedAudioClockBinding::new(&b.encode().unwrap()).unwrap()
}
fn input<'a, D: ProjectionDomain>(
    source: &'a D::Source,
    target: &'a D::Target,
    facts: &'a [ProjectionFact<'a, D>],
) -> ProjectionInput<'a, D> {
    ProjectionInput {
        basis: ProjectionBasis {
            report: text("report"),
            source: text("source"),
            target: Some(text("target")),
            projector: text("audio/checked-source"),
            requested_route: text("finite-projection"),
            boundary: None,
        },
        source,
        target: Some(target),
        facts,
        native: &[],
        scores: &[],
        admitted: &[],
        attempts: &[],
        selected_attempt: None,
        mechanism: ProjectionMechanism::Completed,
        diagnostics: &[],
    }
}
#[test]
fn exact_clock_ticks_and_retained_fractional_ticks_have_different_fidelity() {
    let clock = clock();
    for (n, d, exact) in [(1, 1, true), (1, 3, false)] {
        let receipt = clock.project(&query(time(n, d))).unwrap();
        let domain = AudioIntegerClockProjection::new(&receipt);
        let facts = domain.facts();
        let policy = AudioFloorClockTickPolicy;
        let report = ProjectionReport::new(
            &domain,
            &policy,
            input(domain.source(), domain.target(), &facts),
        )
        .unwrap();
        assert_eq!(report.source().query_canonical(), receipt.query_canonical());
        if exact {
            assert!(report.require_exact().is_ok());
        } else {
            assert_eq!(
                report.require_exact(),
                Err(ProjectionRefusal::ConsumerRequiresExact)
            );
            assert_eq!(
                report.summary().disposition,
                ProjectionDisposition::Completed(ProjectionFidelity::PermittedLossy)
            );
            assert!(report
                .require_policy(text("audio/explicit-floor-clock-tick@1"))
                .is_ok());
            assert_eq!(*receipt.result().raw().remainder_numerator(), 1);
            assert_eq!(*receipt.result().raw().remainder_denominator(), 3);
        }
        assert!(ProjectionReport::new(
            &domain,
            &policy,
            input(domain.source(), domain.target(), &facts[..2])
        )
        .is_err());
    }
}
#[test]
fn point_values_never_claim_exact_continuous_trajectory_fidelity() {
    let trajectory = trajectory(vec![segment(
        time(0, 1),
        time(1, 1),
        hz(100, 1),
        hz(200, 1),
        AudioTrajectoryInterpolation::Linear,
    )]);
    let set = AudioControlTrajectorySet::new(
        AudioControlConflictPolicy::RefuseOverlap,
        BoundedSequence::try_from_iter(vec![AudioScopedControlTrajectory::new(
            "pitch".into(),
            AudioTrajectoryDomain::Frequency,
            0,
            "source/pitch".into(),
            trajectory,
        )
        .unwrap()])
        .unwrap(),
        AudioControlGapPolicy::RefuseAnyGap,
    )
    .unwrap();
    let original = set.encode().unwrap();
    let schedule = AudioControlProjectionSchedule::new(
        anchor(1, 1),
        AudioControlCadence::new(3).unwrap(),
        AudioControlReconstruction::PointSamplesOnly,
        time(0, 1),
        4,
    )
    .unwrap()
    .encode()
    .unwrap();
    let receipt = PreparedAudioControlProjection::new(&original)
        .unwrap()
        .project(&schedule, &clock())
        .unwrap();
    let domain = AudioControlPointProjection::new(&receipt).unwrap();
    let facts = domain.facts();
    let policy = AudioPointSamplesOnlyPolicy;
    let report = ProjectionReport::new(
        &domain,
        &policy,
        input(domain.source(), domain.target(), &facts),
    )
    .unwrap();
    assert_eq!(
        report.require_exact(),
        Err(ProjectionRefusal::ConsumerRequiresExact)
    );
    assert_eq!(
        report.summary().disposition,
        ProjectionDisposition::Completed(ProjectionFidelity::PermittedLossy)
    );
    assert_eq!(
        report.summary().losses[ProjectionLoss::Truncation as usize],
        2
    );
    assert_eq!(report.source().original_canonical(), original);
    assert_eq!(report.source().schedule_canonical(), schedule);
    assert!(report.native().is_empty());
    let target = report
        .require_policy(text("audio/point-samples-only-policy@1"))
        .unwrap();
    assert_eq!(target.points().len(), 4);
    assert_eq!(
        target.points()[1].query_canonical(),
        receipt.points()[1].query_canonical()
    );
    let (n, d) = ratio(&target.points()[3].values()[0]);
    assert_eq!(n, 200 * d);
    assert_eq!(target.points()[0].selected_indices(), [0]);
    assert!(ProjectionReport::new(
        &domain,
        &policy,
        input(domain.source(), domain.target(), &facts[..5])
    )
    .is_err());
    // Omitting the mandatory shape/coverage losses cannot manufacture exactness.
    assert!(ProjectionReport::new(
        &domain,
        &policy,
        input(domain.source(), domain.target(), &facts[..4])
    )
    .is_err());
}
