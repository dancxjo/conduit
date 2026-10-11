//! Directed bounds use independent Euclidean integer arithmetic as an oracle.
use conduit_core::{projection::*, *};
fn work() -> ExactNumericWork {
    ExactNumericWork::new(100_000_000)
}
fn ratio(n: i128, d: i128) -> ExactRational {
    ExactRational::ratio(n, d, &mut work()).unwrap()
}
fn text(s: &str) -> ProjectionText<'_> {
    ProjectionText::new(s).unwrap()
}
fn input<'a>(
    source: &'a PreparedIntegerEnclosure,
    target: &'a IntegerEnclosure,
    facts: &'a [ProjectionFact<'a, IntegerEnclosureDomain>],
) -> ProjectionInput<'a, IntegerEnclosureDomain> {
    ProjectionInput {
        basis: ProjectionBasis {
            report: text("fixture/report"),
            source: text("fixture/exact"),
            target: Some(text("fixture/bounds")),
            projector: text("numeric/outward-integer-enclosure@1"),
            requested_route: text("fixture/admitted-enclosure"),
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
fn signed_outward_bounds_and_distances_agree_with_independent_small_oracle() {
    for numerator in -20i128..=20 {
        for denominator in 1i128..=12 {
            let value = ratio(numerator, denominator);
            let prepared = PreparedIntegerEnclosure::prepare(
                value,
                PrimitiveInfoKind::I16,
                ExactRational::ONE,
                &mut work(),
            )
            .unwrap();
            let lower = numerator.div_euclid(denominator);
            let upper = -(-numerator).div_euclid(denominator);
            assert_eq!(prepared.target().lower().signed().unwrap(), lower);
            assert_eq!(prepared.target().upper().signed().unwrap(), upper);
            assert_eq!(
                prepared.error().lower_distance(),
                ratio(numerator - lower * denominator, denominator)
            );
            assert_eq!(
                prepared.error().upper_distance(),
                ratio(upper * denominator - numerator, denominator)
            );
            assert_eq!(prepared.source(), value);
        }
    }
}
#[test]
fn every_integer_width_checks_both_bounds_and_preserves_native_extrema() {
    use PrimitiveInfoKind as K;
    for kind in [K::I8, K::I16, K::I32, K::I64, K::I128] {
        let p =
            PreparedIntegerEnclosure::prepare(ratio(-1, 3), kind, ExactRational::ONE, &mut work())
                .unwrap();
        assert_eq!(p.target().lower().signed().unwrap(), -1);
        assert_eq!(p.target().upper().signed().unwrap(), 0);
    }
    for kind in [K::U8, K::U16, K::U32, K::U64, K::U128] {
        let p =
            PreparedIntegerEnclosure::prepare(ratio(1, 3), kind, ExactRational::ONE, &mut work())
                .unwrap();
        assert_eq!(p.target().lower().unsigned().unwrap(), 0);
        assert_eq!(p.target().upper().unsigned().unwrap(), 1);
        assert!(PreparedIntegerEnclosure::prepare(
            ratio(-1, 3),
            kind,
            ExactRational::ONE,
            &mut work()
        )
        .is_err());
    }
    let max = PreparedIntegerEnclosure::prepare(
        ExactRational::from_u128(u128::MAX),
        K::U128,
        ExactRational::ZERO,
        &mut work(),
    )
    .unwrap();
    assert_eq!(max.target().upper().unsigned().unwrap(), u128::MAX);
    let min = PreparedIntegerEnclosure::prepare(
        ExactRational::from_i128(i128::MIN),
        K::I128,
        ExactRational::ZERO,
        &mut work(),
    )
    .unwrap();
    assert_eq!(min.target().lower().signed().unwrap(), i128::MIN);
    for (value, kind) in [
        (ratio(2551, 10), K::U8),
        (ratio(1271, 10), K::I8),
        (ratio(-1281, 10), K::I8),
    ] {
        assert!(matches!(
            PreparedIntegerEnclosure::prepare(value, kind, ExactRational::ONE, &mut work()),
            Err(IntegerEnclosureRefusal::Target(_))
        ));
    }
}
#[test]
fn extent_work_and_tolerance_refuse_without_float_or_saturation_fallback() {
    use IntegerEnclosureRefusal as R;
    let fractional = ratio(1, 3);
    assert_eq!(
        PreparedIntegerEnclosure::prepare(
            fractional,
            PrimitiveInfoKind::U64,
            ratio(1, 2),
            &mut work()
        ),
        Err(R::ToleranceExceeded)
    );
    assert_eq!(
        PreparedIntegerEnclosure::prepare(
            fractional,
            PrimitiveInfoKind::U64,
            ExactRational::from_i128(-1),
            &mut work()
        ),
        Err(R::NegativeTolerance)
    );
    assert_eq!(
        PreparedIntegerEnclosure::prepare(
            fractional,
            PrimitiveInfoKind::U64,
            ExactRational::ONE,
            &mut ExactNumericWork::new(0)
        ),
        Err(R::Numeric(ExactNumericRefusal::WorkLimit))
    );
    for kind in [
        PrimitiveInfoKind::F32,
        PrimitiveInfoKind::F64,
        PrimitiveInfoKind::Scalar,
        PrimitiveInfoKind::Count,
    ] {
        assert_eq!(
            PreparedIntegerEnclosure::prepare(fractional, kind, ExactRational::ONE, &mut work()),
            Err(R::UnsupportedTarget)
        );
    }
    let mut budget = work();
    let enormous = ExactRational::from_i128(10)
        .checked_power(1000, &mut budget)
        .unwrap();
    assert_eq!(
        PreparedIntegerEnclosure::prepare(
            enormous,
            PrimitiveInfoKind::U128,
            ExactRational::ONE,
            &mut work()
        ),
        Err(R::Numeric(ExactNumericRefusal::OutOfRange))
    );
    // The numerator itself is far above u128, while the quotient fits: use
    // full bounded division, never narrow the operands before dividing.
    let nearly_one = enormous
        .checked_add(ExactRational::ONE, &mut budget)
        .unwrap()
        .checked_divide(enormous, &mut budget)
        .unwrap();
    let bounds = PreparedIntegerEnclosure::prepare(
        nearly_one,
        PrimitiveInfoKind::U8,
        ExactRational::ONE,
        &mut budget,
    )
    .unwrap()
    .target();
    assert_eq!(bounds.lower().unsigned().unwrap(), 1);
    assert_eq!(bounds.upper().unsigned().unwrap(), 2);
}
#[test]
fn common_fidelity_reports_fractional_loss_and_rejects_forged_custody() {
    let prepared = PreparedIntegerEnclosure::prepare(
        ratio(1, 3),
        PrimitiveInfoKind::U64,
        ExactRational::ONE,
        &mut work(),
    )
    .unwrap();
    let target = prepared.target();
    let facts = prepared.facts();
    let policy = IntegerEnclosurePolicy {
        maximum_outward_error: ExactRational::ONE,
    };
    let report = ProjectionReport::new(
        &IntegerEnclosureDomain,
        &policy,
        input(&prepared, &target, &facts),
    )
    .unwrap();
    assert_eq!(
        report.require_exact(),
        Err(ProjectionRefusal::ConsumerRequiresExact)
    );
    assert_eq!(report.require_policy(policy.identity()), Ok(&target));
    assert_eq!(
        report.summary().disposition,
        ProjectionDisposition::Completed(ProjectionFidelity::PermittedLossy)
    );
    let wrong = PreparedIntegerEnclosure::prepare(
        ratio(4, 3),
        PrimitiveInfoKind::U64,
        ExactRational::ONE,
        &mut work(),
    )
    .unwrap()
    .target();
    assert!(matches!(
        ProjectionReport::new(
            &IntegerEnclosureDomain,
            &policy,
            input(&prepared, &wrong, &facts)
        ),
        Err(ProjectionRefusal::Domain)
    ));
    let fake_exact = [
        ProjectionFact::Preserved {
            obligation: text("exact-denotation"),
        },
        ProjectionFact::Transformed {
            obligation: text("numeric-representation"),
            law: text("numeric/outward-integer-enclosure@1"),
        },
    ];
    assert!(matches!(
        ProjectionReport::new(
            &IntegerEnclosureDomain,
            &policy,
            input(&prepared, &target, &fake_exact)
        ),
        Err(ProjectionRefusal::Domain)
    ));
    let strict = IntegerEnclosurePolicy {
        maximum_outward_error: ExactRational::ZERO,
    };
    assert_eq!(
        ProjectionReport::new(
            &IntegerEnclosureDomain,
            &strict,
            input(&prepared, &target, &facts)
        )
        .unwrap()
        .summary()
        .disposition,
        ProjectionDisposition::Insufficient
    );
    let exact = PreparedIntegerEnclosure::prepare(
        ExactRational::from_i128(7),
        PrimitiveInfoKind::U8,
        ExactRational::ZERO,
        &mut work(),
    )
    .unwrap();
    let target = exact.target();
    let facts = exact.facts();
    assert_eq!(
        ProjectionReport::new(
            &IntegerEnclosureDomain,
            &strict,
            input(&exact, &target, &facts)
        )
        .unwrap()
        .require_exact(),
        Ok(&target)
    );
}
