//! Dynamic physical symbolic input, enclosure and midpoint binary32 certification.
use conduit_core::{projection::*, *};
#[path = "common/numerical_allocation.rs"]
mod allocation_probe;
use allocation_probe::observe;
fn work() -> ExactNumericWork {
    ExactNumericWork::new(200_000_000)
}
fn q(n: i128, d: i128) -> ExactRational {
    ExactRational::ratio(n, d, &mut work()).unwrap()
}
fn requirements() -> MathematicalEnclosureRequirements {
    MathematicalEnclosureRequirements {
        maximum_width: q(1, 100_000_000_000_000_000),
        maximum_leaf_width: q(1, 100_000_000_000_000_000_000),
        maximum_constant_terms: 32,
        maximum_root_refinements: 64,
        maximum_scratch_payload_bytes: 131072,
        maximum_retained_payload_bytes: 131072,
    }
}
fn frame(symbolic: bool, unit: Unit, role: QuantityRole) -> Vec<u8> {
    let value = if symbolic {
        MathematicalValue::parse_literal("pi", &mut work()).unwrap()
    } else {
        MathematicalValue::rational(ExactRational::ZERO)
    };
    let mut coordinate = vec![0; MATHEMATICAL_VALUE_MAX_ENCODED_BYTES];
    let n = value.encode(&mut coordinate).unwrap();
    let view = MathematicalValueView::decode(&coordinate[..n], &mut work()).unwrap();
    let mut bytes = vec![0; MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES];
    let n = encode_mathematical_quantity_value(&view, unit, role, &mut bytes, &mut work()).unwrap();
    bytes.truncate(n);
    bytes
}
fn limit(unit: Unit, role: QuantityRole) -> RationalQuantity {
    RationalQuantity::new(q(1, 1_000_000), unit, role).unwrap()
}
fn text(s: &str) -> ProjectionText<'_> {
    ProjectionText::new(s).unwrap()
}
fn input<'a, 's>(
    source: &'a BorrowedMathematicalQuantityBinary32<'s>,
    target: &'a QuantityBinary32,
    facts: &'a [ProjectionFact<'a, MathematicalQuantityBinary32Domain<'s>>],
) -> ProjectionInput<'a, MathematicalQuantityBinary32Domain<'s>> {
    ProjectionInput {
        basis: ProjectionBasis {
            report: text("test/report@1"),
            source: text("test/source@1"),
            target: Some(text("test/target@1")),
            projector: text("test/projector@1"),
            requested_route: text("test/route@1"),
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
fn actual_symbolic_coordinates_retain_frame_and_certify_both_endpoint_distances_without_growth() {
    for (unit, role, distance_role) in [
        (Unit::Radian, QuantityRole::Linear, QuantityRole::Linear),
        (Unit::Fahrenheit, QuantityRole::Point, QuantityRole::Delta),
        (Unit::Fahrenheit, QuantityRole::Delta, QuantityRole::Delta),
    ] {
        let maximum = limit(unit, distance_role);
        let mut prepared = PreparedMathematicalQuantityBinary32::new(
            requirements(),
            Binary32Rounding::NearestTiesEven,
            maximum,
            200_000_000,
        )
        .unwrap();
        let retained = prepared.retained_payload_bytes();
        let bytes = frame(true, unit, role);
        let (_, allocations) = observe(|| {
            for _ in 0..4 {
                let receipt = prepared.execute(&bytes, &mut work()).unwrap();
                assert_eq!(receipt.source().encoded(), bytes);
                assert_eq!(receipt.target().unit(), unit);
                assert_eq!(receipt.target().role(), role);
                assert_eq!(receipt.source().family(), unit.family());
                assert!(matches!(
                    receipt.error(),
                    Binary32ErrorClaim::CertifiedBound(_)
                ));
                let represented = receipt.midpoint().numeric().represented();
                for endpoint in [
                    receipt.enclosure().lower().coordinate(),
                    receipt.enclosure().upper().coordinate(),
                ] {
                    let delta = represented.checked_subtract(endpoint, &mut work()).unwrap();
                    let distance = if delta.is_negative() {
                        delta.negated()
                    } else {
                        delta
                    };
                    assert_ne!(
                        distance
                            .checked_compare(receipt.error().maximum(), &mut work())
                            .unwrap(),
                        core::cmp::Ordering::Greater
                    );
                }
                assert_ne!(
                    receipt
                        .error()
                        .maximum()
                        .checked_compare(maximum.coordinate(), &mut work())
                        .unwrap(),
                    core::cmp::Ordering::Greater
                );
                assert_eq!(prepared.retained_payload_bytes(), retained);
            }
        });
        assert_eq!(allocations, 0);
    }
}
#[test]
fn general_report_refuses_false_exactness_missing_physical_fact_and_wrong_policy_or_target() {
    let maximum = limit(Unit::Radian, QuantityRole::Linear);
    let mut prepared = PreparedMathematicalQuantityBinary32::new(
        requirements(),
        Binary32Rounding::NearestTiesEven,
        maximum,
        200_000_000,
    )
    .unwrap();
    let bytes = frame(true, Unit::Radian, QuantityRole::Linear);
    let source = prepared.execute(&bytes, &mut work()).unwrap();
    let target = source.target();
    let facts = source.facts();
    let domain = MathematicalQuantityBinary32Domain::new();
    let policy = MathematicalQuantityBinary32Policy {
        maximum_error: maximum,
        rounding: Binary32Rounding::NearestTiesEven,
    };
    let wrong = PreparedQuantityBinary32Projection::prepare(
        RationalQuantity::new(
            source.midpoint().numeric().source(),
            Unit::Degree,
            QuantityRole::Linear,
        )
        .unwrap(),
        Binary32Rounding::NearestTiesEven,
        limit(Unit::Degree, QuantityRole::Linear),
        &mut work(),
    )
    .unwrap()
    .target();
    assert_eq!(wrong.coordinate(), target.coordinate());
    let wrong_value = PreparedQuantityBinary32Projection::prepare(
        RationalQuantity::new(ExactRational::ONE, Unit::Radian, QuantityRole::Linear).unwrap(),
        Binary32Rounding::NearestTiesEven,
        maximum,
        &mut work(),
    )
    .unwrap()
    .target();
    let false_facts = [
        ProjectionFact::Preserved {
            obligation: text("mathematical-denotation"),
        },
        ProjectionFact::Transformed {
            obligation: text("numeric-representation"),
            law: text(MATHEMATICAL_QUANTITY_BINARY32_LAW),
        },
        ProjectionFact::Preserved {
            obligation: text("physical-coordinate-contract"),
        },
    ];
    let (_, allocations) = observe(|| {
        let report =
            ProjectionReport::new(&domain, &policy, input(&source, &target, &facts)).unwrap();
        assert!(report.require_exact().is_err());
        assert!(report
            .require_policy(text("numeric/enclosed-quantity-binary32-policy@1"))
            .is_ok());
        assert!(
            ProjectionReport::new(&domain, &policy, input(&source, &target, &facts[..2])).is_err()
        );
        assert!(
            ProjectionReport::new(&domain, &policy, input(&source, &target, &false_facts)).is_err()
        );
        assert!(ProjectionReport::new(&domain, &policy, input(&source, &wrong, &facts)).is_err());
        assert!(
            ProjectionReport::new(&domain, &policy, input(&source, &wrong_value, &facts)).is_err()
        );
        assert!(!MathematicalQuantityBinary32Policy {
            rounding: Binary32Rounding::TowardZero,
            ..policy
        }
        .permits(&source, &target, &facts));
        assert!(!MathematicalQuantityBinary32Policy {
            maximum_error: limit(Unit::Degree, QuantityRole::Linear),
            ..policy
        }
        .permits(&source, &target, &facts));
    });
    assert_eq!(allocations, 0);
}
#[test]
fn exact_zero_tight_tolerance_work_and_physical_refusals_do_not_poison_recovery() {
    let zero =
        RationalQuantity::new(ExactRational::ZERO, Unit::Radian, QuantityRole::Linear).unwrap();
    let mut prepared = PreparedMathematicalQuantityBinary32::new(
        requirements(),
        Binary32Rounding::NearestTiesEven,
        zero,
        200_000_000,
    )
    .unwrap();
    let symbolic = frame(true, Unit::Radian, QuantityRole::Linear);
    let exact = frame(false, Unit::Radian, QuantityRole::Linear);
    let foreign = frame(false, Unit::Degree, QuantityRole::Linear);
    let (_, allocations) = observe(|| {
        assert!(prepared.execute(&symbolic, &mut work()).is_err());
        assert!(prepared
            .execute(&exact, &mut ExactNumericWork::new(0))
            .is_err());
        assert!(matches!(
            prepared.execute(&foreign, &mut work()),
            Err(MathematicalQuantityBinary32Refusal::PhysicalToleranceMismatch)
        ));
        let result = prepared.execute(&exact, &mut work()).unwrap();
        assert_eq!(
            result.error(),
            Binary32ErrorClaim::ExactDistance(ExactRational::ZERO)
        );
        let policy = MathematicalQuantityBinary32Policy {
            maximum_error: zero,
            rounding: Binary32Rounding::NearestTiesEven,
        };
        let target = result.target();
        let facts = result.facts();
        assert!(ProjectionReport::new(
            &MathematicalQuantityBinary32Domain::new(),
            &policy,
            input(&result, &target, &facts)
        )
        .unwrap()
        .require_exact()
        .is_ok());
    });
    assert_eq!(allocations, 0);
    assert!(PreparedMathematicalQuantityBinary32::new(
        requirements(),
        Binary32Rounding::NearestTiesEven,
        RationalQuantity::new(q(-1, 10), Unit::Radian, QuantityRole::Linear).unwrap(),
        200_000_000
    )
    .is_err());
    assert!(PreparedMathematicalQuantityBinary32::new(
        requirements(),
        Binary32Rounding::NearestTiesEven,
        zero,
        0
    )
    .is_err());
    assert!(PreparedMathematicalQuantityBinary32::new(
        MathematicalEnclosureRequirements {
            maximum_retained_payload_bytes: 1,
            ..requirements()
        },
        Binary32Rounding::NearestTiesEven,
        zero,
        200_000_000
    )
    .is_err());
}
