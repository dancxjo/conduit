//! Physical coordinates and numerical loss use the existing projection report.
use conduit_core::{projection::*, *};
fn work() -> ExactNumericWork {
    ExactNumericWork::new(100_000_000)
}
fn quantity(n: i128, d: i128, unit: &str, role: QuantityRole) -> RationalQuantity {
    RationalQuantity::new(
        ExactRational::ratio(n, d, &mut work()).unwrap(),
        Unit::resolve(unit).unwrap(),
        role,
    )
    .unwrap()
}
fn text(s: &str) -> ProjectionText<'_> {
    ProjectionText::new(s).unwrap()
}
fn input<'a>(
    source: &'a PreparedQuantityBinary32Projection,
    target: &'a QuantityBinary32,
    facts: &'a [ProjectionFact<'a, QuantityBinary32ProjectionDomain>],
) -> ProjectionInput<'a, QuantityBinary32ProjectionDomain> {
    ProjectionInput {
        basis: ProjectionBasis {
            report: text("fixture/quantity-binary32/report"),
            source: text("fixture/quantity-binary32/source"),
            target: Some(text("fixture/quantity-binary32/target")),
            projector: text("numeric/quantity-binary32-projection@1"),
            requested_route: text("fixture/quantity-binary32"),
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
fn quantity_third_has_truthful_numeric_loss_and_unchanged_physical_facts() {
    let source = quantity(1, 3, "m", QuantityRole::Linear);
    let tolerance = quantity(1, 1_000_000, "m", QuantityRole::Linear);
    let p = PreparedQuantityBinary32Projection::prepare(
        source,
        Binary32Rounding::NearestTiesEven,
        tolerance,
        &mut work(),
    )
    .unwrap();
    let target = p.target();
    assert_eq!(target.coordinate().bits(), 0x3eaa_aaab);
    assert_eq!(target.unit(), source.unit());
    assert_eq!(target.unit().dimension(), source.dimension());
    assert_eq!(target.unit().family(), source.family());
    assert_eq!(target.role(), source.role());
    assert_eq!(p.source(), source);
    let facts = p.facts();
    let policy = QuantityBinary32ProjectionPolicy {
        maximum_error: tolerance,
        rounding: Binary32Rounding::NearestTiesEven,
    };
    let report = ProjectionReport::new(
        &QuantityBinary32ProjectionDomain,
        &policy,
        input(&p, &target, &facts),
    )
    .unwrap();
    assert_eq!(
        report.require_exact(),
        Err(ProjectionRefusal::ConsumerRequiresExact)
    );
    assert_eq!(report.require_policy(policy.identity()), Ok(&target));
    // Every consuming requirement must revalidate its actual bound; the static
    // policy-class name alone is not an identity for all tolerance parameters.
    let weaker = QuantityBinary32ProjectionPolicy {
        maximum_error: quantity(2, 1_000_000, "m", QuantityRole::Linear),
        ..policy
    };
    let accepted = ProjectionReport::new(
        &QuantityBinary32ProjectionDomain,
        &weaker,
        input(&p, &target, &facts),
    )
    .unwrap();
    assert_eq!(accepted.require_policy(weaker.identity()), Ok(&target));
    for denied in [
        QuantityBinary32ProjectionPolicy {
            maximum_error: quantity(1, 1_000_000_000, "m", QuantityRole::Linear),
            ..policy
        },
        QuantityBinary32ProjectionPolicy {
            maximum_error: quantity(1, 1, "s", QuantityRole::Linear),
            ..policy
        },
    ] {
        let report = ProjectionReport::new(
            &QuantityBinary32ProjectionDomain,
            &denied,
            input(&p, &target, &facts),
        )
        .unwrap();
        assert_eq!(
            report.summary().disposition,
            ProjectionDisposition::Insufficient
        );
        assert_eq!(
            report.require_policy(denied.identity()),
            Err(ProjectionRefusal::ConsumerRequiresExact)
        );
    }
    assert!(ProjectionReport::new(
        &QuantityBinary32ProjectionDomain,
        &policy,
        input(&p, &target, &facts[..2])
    )
    .is_err());
}
#[test]
fn coordinate_and_numeric_projections_remain_independent_and_affine_roles_refuse() {
    let original = Quantity::parse_plot_literal("90cm").unwrap();
    let coordinate = RationalQuantityConversionReceipt::from_checked(
        original,
        Unit::resolve("m").unwrap(),
        "90cm",
        "m",
        &mut work(),
    )
    .unwrap();
    let source = coordinate.result().unwrap();
    assert_eq!(
        source.coordinate(),
        ExactRational::ratio(9, 10, &mut work()).unwrap()
    );
    let tolerance = quantity(1, 1_000_000, "m", QuantityRole::Linear);
    let p = PreparedQuantityBinary32Projection::prepare(
        source,
        Binary32Rounding::NearestTiesEven,
        tolerance,
        &mut work(),
    )
    .unwrap();
    assert_eq!(coordinate.original(), "90cm");
    assert_eq!(coordinate.source(), original);
    assert_eq!(p.source(), source);
    assert_eq!(p.target().unit(), source.unit());
    assert_eq!(p.target().coordinate().bits(), 0x3f66_6666);
    assert_eq!(
        PreparedQuantityBinary32Projection::prepare(
            quantity(1, 1, "m", QuantityRole::Linear),
            Binary32Rounding::NearestTiesEven,
            quantity(1, 1, "s", QuantityRole::Linear),
            &mut work()
        ),
        Err(QuantityBinary32ProjectionRefusal::PhysicalToleranceMismatch)
    );
    let point = quantity(1, 3, "°C", QuantityRole::Point);
    let delta = quantity(1, 1_000_000, "°C", QuantityRole::Delta);
    let affine = PreparedQuantityBinary32Projection::prepare(
        point,
        Binary32Rounding::NearestTiesEven,
        delta,
        &mut work(),
    )
    .unwrap();
    assert_eq!(affine.target().role(), QuantityRole::Point);
    for wrong in [
        quantity(1, 1, "°C", QuantityRole::Point),
        quantity(1, 1, "K", QuantityRole::Delta),
    ] {
        assert_eq!(
            PreparedQuantityBinary32Projection::prepare(
                point,
                Binary32Rounding::NearestTiesEven,
                wrong,
                &mut work()
            ),
            Err(QuantityBinary32ProjectionRefusal::PhysicalToleranceMismatch)
        );
    }
}

#[test]
fn zero_loss_physical_binary32_still_refuses_wrong_policy_rounding_units_and_roles() {
    let source = quantity(2, 1, "m", QuantityRole::Linear);
    let limit = quantity(0, 1, "m", QuantityRole::Linear);
    let receipt = PreparedQuantityBinary32Projection::prepare(
        source,
        Binary32Rounding::NearestTiesEven,
        limit,
        &mut work(),
    )
    .unwrap();
    assert_eq!(
        receipt.error(),
        Binary32ErrorClaim::ExactDistance(ExactRational::ZERO)
    );
    let target = receipt.target();
    let facts = receipt.facts();
    let permitted = QuantityBinary32ProjectionPolicy {
        maximum_error: limit,
        rounding: Binary32Rounding::NearestTiesEven,
    };
    let report = ProjectionReport::new(
        &QuantityBinary32ProjectionDomain,
        &permitted,
        input(&receipt, &target, &facts),
    )
    .unwrap();
    assert_eq!(report.require_exact(), Ok(&target));
    for denied in [
        QuantityBinary32ProjectionPolicy {
            rounding: Binary32Rounding::TowardZero,
            ..permitted
        },
        QuantityBinary32ProjectionPolicy {
            maximum_error: quantity(0, 1, "s", QuantityRole::Linear),
            ..permitted
        },
        QuantityBinary32ProjectionPolicy {
            maximum_error: quantity(-1, 1, "m", QuantityRole::Linear),
            ..permitted
        },
        QuantityBinary32ProjectionPolicy {
            maximum_error: quantity(0, 1, "°C", QuantityRole::Delta),
            ..permitted
        },
    ] {
        assert!(!denied.permits(&receipt, &target, &facts));
        let report = ProjectionReport::new(
            &QuantityBinary32ProjectionDomain,
            &denied,
            input(&receipt, &target, &facts),
        )
        .unwrap();
        assert_eq!(
            report.summary().disposition,
            ProjectionDisposition::Completed(ProjectionFidelity::Exact)
        );
        assert!(report.require_policy(denied.identity()).is_err());
        assert_eq!(report.require_exact(), Ok(&target));
        assert_eq!(report.inspect_target(), Some(&target));
    }
}

#[test]
fn zero_loss_affine_point_keeps_exact_fidelity_but_denies_a_point_error_allowance() {
    let source = quantity(2, 1, "°C", QuantityRole::Point);
    let delta = quantity(0, 1, "°C", QuantityRole::Delta);
    let receipt = PreparedQuantityBinary32Projection::prepare(
        source,
        Binary32Rounding::NearestTiesEven,
        delta,
        &mut work(),
    )
    .unwrap();
    let target = receipt.target();
    let facts = receipt.facts();
    let admitted = QuantityBinary32ProjectionPolicy {
        maximum_error: delta,
        rounding: Binary32Rounding::NearestTiesEven,
    };
    let report = ProjectionReport::new(
        &QuantityBinary32ProjectionDomain,
        &admitted,
        input(&receipt, &target, &facts),
    )
    .unwrap();
    assert_eq!(report.require_policy(admitted.identity()), Ok(&target));
    let denied = QuantityBinary32ProjectionPolicy {
        maximum_error: quantity(0, 1, "°C", QuantityRole::Point),
        ..admitted
    };
    assert!(!denied.permits(&receipt, &target, &facts));
    let report = ProjectionReport::new(
        &QuantityBinary32ProjectionDomain,
        &denied,
        input(&receipt, &target, &facts),
    )
    .unwrap();
    assert_eq!(report.require_exact(), Ok(&target));
    assert_eq!(
        report.require_policy(denied.identity()),
        Err(ProjectionRefusal::PolicyMismatch)
    );
    assert_eq!(report.inspect_target(), Some(&target));
}
