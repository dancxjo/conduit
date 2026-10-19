//! Generic projection reports must retain physical and numerical obligations.
use conduit_core::projection::*;
use conduit_core::*;
#[path = "common/numerical_allocation.rs"]
mod allocation_probe;
use allocation_probe::observe;
fn text(s: &str) -> ProjectionText<'_> {
    ProjectionText::new(s).unwrap()
}
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
        MathematicalValue::rational(q(1, 3))
    };
    let mut coordinate = vec![0; MATHEMATICAL_VALUE_MAX_ENCODED_BYTES];
    let n = value.encode(&mut coordinate).unwrap();
    let view = MathematicalValueView::decode(&coordinate[..n], &mut work()).unwrap();
    let mut out = vec![0; MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES];
    let n = encode_mathematical_quantity_value(&view, unit, role, &mut out, &mut work()).unwrap();
    out.truncate(n);
    out
}
fn input<'a, 's>(
    source: &'a BorrowedMathematicalQuantityEnclosure<'s>,
    target: &'a MathematicalQuantityInterval,
    facts: &'a [ProjectionFact<'a, MathematicalQuantityEnclosureDomain<'s>>],
) -> ProjectionInput<'a, MathematicalQuantityEnclosureDomain<'s>> {
    ProjectionInput {
        basis: ProjectionBasis {
            report: text("quantity/report@1"),
            source: text("quantity/source@1"),
            target: Some(text("quantity/target@1")),
            projector: text("quantity/projector@1"),
            requested_route: text("quantity/route@1"),
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
fn report_retains_unit_role_and_truthful_numeric_precision_without_execution_allocation() {
    let req = requirements();
    let policy = MathematicalQuantityEnclosurePolicy { requirements: req };
    let domain = MathematicalQuantityEnclosureDomain::new();
    let mut evaluator = PreparedMathematicalQuantityEnclosure::new(req, 200_000_000).unwrap();
    let retained = evaluator.retained_payload_bytes();
    for symbolic in [false, true] {
        for (unit, role) in [
            (Unit::Radian, QuantityRole::Linear),
            (Unit::Fahrenheit, QuantityRole::Point),
            (Unit::Fahrenheit, QuantityRole::Delta),
        ] {
            let bytes = frame(symbolic, unit, role);
            let source = evaluator.execute(&bytes, &mut work()).unwrap();
            let target = source.target();
            let facts = source.facts();
            let wrong_unit = MathematicalQuantityInterval {
                unit: Unit::Kelvin,
                ..target
            };
            let wrong_role = MathematicalQuantityInterval {
                role: if role == QuantityRole::Point {
                    QuantityRole::Delta
                } else {
                    QuantityRole::Point
                },
                ..target
            };
            let wrong_numeric = MathematicalQuantityInterval {
                coordinate: evaluator
                    .execute(&frame(!symbolic, unit, role), &mut work())
                    .unwrap()
                    .target()
                    .coordinate,
                ..target
            };
            let false_facts = [
                ProjectionFact::Preserved {
                    obligation: text("exact-denotation"),
                },
                ProjectionFact::Transformed {
                    obligation: text("numeric-representation"),
                    law: text("numeric/exact-interval-expression@1"),
                },
                ProjectionFact::Preserved {
                    obligation: text("physical-coordinate-contract"),
                },
            ];
            let changed = MathematicalQuantityEnclosurePolicy {
                requirements: MathematicalEnclosureRequirements {
                    maximum_width: ExactRational::ONE,
                    ..req
                },
            };
            let (_, allocations) = observe(|| {
                let report =
                    ProjectionReport::new(&domain, &policy, input(&source, &target, &facts))
                        .unwrap();
                assert_eq!(report.require_exact().is_err(), symbolic);
                assert!(report
                    .require_policy(text("numeric/quantity-coordinate-interval-policy@1"))
                    .is_ok());
                for bad in [&wrong_unit, &wrong_role, &wrong_numeric] {
                    assert!(
                        ProjectionReport::new(&domain, &policy, input(&source, bad, &facts))
                            .is_err()
                    );
                }
                assert!(ProjectionReport::new(
                    &domain,
                    &policy,
                    input(&source, &target, &facts[..2])
                )
                .is_err());
                assert!(!changed.permits(&source, &target, &facts));
                if symbolic {
                    assert!(ProjectionReport::new(
                        &domain,
                        &policy,
                        input(&source, &target, &false_facts)
                    )
                    .is_err());
                }
                assert_eq!(source.source().encoded(), bytes);
                assert_eq!(target.unit, unit);
                assert_eq!(target.role, role);
                assert_eq!(target.unit.family(), unit.family());
                assert_eq!(target.unit.dimension(), unit.dimension());
                assert_eq!(evaluator.retained_payload_bytes(), retained);
            });
            assert_eq!(allocations, 0);
        }
    }
}
