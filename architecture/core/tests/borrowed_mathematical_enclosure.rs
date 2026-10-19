//! Exact dynamic data reaches the existing certified numeric engine without growth.
use conduit_core::projection::*;
use conduit_core::*;
use core::cmp::Ordering;
#[path = "common/numerical_allocation.rs"]
mod allocation_probe;
use allocation_probe::observe;
type Q = ExactRational;
type R = MathematicalEnclosureRefusal;
fn work() -> ExactNumericWork {
    ExactNumericWork::new(100_000_000)
}
fn q(n: i128, d: i128) -> Q {
    Q::ratio(n, d, &mut work()).unwrap()
}
fn requirements() -> MathematicalEnclosureRequirements {
    MathematicalEnclosureRequirements {
        maximum_width: q(1, 10_000_000_000_000_000),
        maximum_leaf_width: q(1, 100_000_000_000_000_000),
        maximum_constant_terms: 32,
        maximum_root_refinements: 64,
        maximum_scratch_payload_bytes: 131072,
        maximum_retained_payload_bytes: 131072,
    }
}
fn populate(
    b: &mut PreparedMathematicalValueBuilder,
    which: u8,
    w: &mut ExactNumericWork,
) -> MathematicalNodeId {
    if which == 0 {
        let pi = b.insert(MathematicalNode::Pi, w).unwrap();
        let n = b
            .insert(MathematicalNode::Rational(Q::from_i128(180)), w)
            .unwrap();
        b.insert(MathematicalNode::Divide(pi, n), w).unwrap()
    } else {
        let n = b
            .insert(
                MathematicalNode::Rational(if which == 1 { Q::from_i128(2) } else { q(1, 7) }),
                w,
            )
            .unwrap();
        b.insert(
            if which == 1 {
                MathematicalNode::PrincipalSquareRoot(n)
            } else {
                MathematicalNode::SineTurns(n)
            },
            w,
        )
        .unwrap()
    }
}
fn encoded(which: u8) -> Vec<u8> {
    let mut b =
        PreparedMathematicalValueBuilder::new(1_000_000, MATHEMATICAL_VALUE_MAX_ENCODED_BYTES)
            .unwrap();
    let root = populate(&mut b, which, &mut work());
    let mut bytes = vec![0; MATHEMATICAL_VALUE_MAX_ENCODED_BYTES];
    let n = b.encode(root, &mut bytes, &mut work()).unwrap();
    bytes.truncate(n);
    bytes
}
fn point() -> Vec<u8> {
    let value = MathematicalValue::rational(q(3, 10));
    let mut bytes = vec![0; MATHEMATICAL_VALUE_MAX_ENCODED_BYTES];
    let n = value.encode(&mut bytes).unwrap();
    bytes.truncate(n);
    bytes
}

#[test]
fn dynamic_construction_and_borrowed_numeric_consumption_preserve_source_and_verified_bounds() {
    let req = requirements();
    let mut b =
        PreparedMathematicalValueBuilder::new(1_000_000, MATHEMATICAL_VALUE_MAX_ENCODED_BYTES)
            .unwrap();
    let mut evaluator = PreparedMathematicalEnclosureEvaluator::new(
        req,
        100_000_000,
        MATHEMATICAL_VALUE_MAX_ENCODED_BYTES,
    )
    .unwrap();
    let retained = evaluator.retained_payload_bytes();
    let mut bytes = vec![0; MATHEMATICAL_VALUE_MAX_ENCODED_BYTES];
    for which in 0..2 {
        let input = encoded(which);
        let source = MathematicalValue::decode(&input, &mut work()).unwrap();
        let expected = PreparedMathematicalEnclosure::prepare(&source, req, &mut work()).unwrap();
        let (lower, upper) = if which == 0 {
            (
                q(174532925199432, 10_000_000_000_000_000),
                q(174532925199434, 10_000_000_000_000_000),
            )
        } else {
            (
                q(14142135623730950, 10_000_000_000_000_000),
                q(14142135623730951, 10_000_000_000_000_000),
            )
        };
        let (correct, allocations) = observe(|| {
            (0..16).all(|_| {
                b.reset();
                let mut w = work();
                let root = populate(&mut b, which, &mut w);
                let n = b.encode(root, &mut bytes, &mut w).unwrap();
                evaluator.execute(&bytes[..n], &mut w).is_ok_and(|result| {
                    result.source().encoded() == input
                        && result.target() == expected.target()
                        && result.width() == expected.width()
                        && result
                            .width()
                            .checked_compare(req.maximum_width, &mut w)
                            .unwrap()
                            != Ordering::Greater
                        && result
                            .target()
                            .lower()
                            .checked_compare(lower, &mut w)
                            .unwrap()
                            == Ordering::Greater
                        && result
                            .target()
                            .upper()
                            .checked_compare(upper, &mut w)
                            .unwrap()
                            == Ordering::Less
                        && result.maximum_used_constant_terms()
                            == expected.maximum_used_constant_terms()
                })
            })
        });
        assert!(correct, "specimen {which}");
        assert_eq!(allocations, 0);
        assert_eq!(evaluator.retained_payload_bytes(), retained);
    }
}

#[test]
fn refusal_after_partial_work_does_not_change_prior_result_and_valid_source_recovers() {
    let mut req = requirements();
    req.maximum_width = Q::ZERO;
    req.maximum_leaf_width = Q::ZERO;
    let mut e = PreparedMathematicalEnclosureEvaluator::new(req, 100_000_000, 4096).unwrap();
    let pi = encoded(0);
    let rational = point();
    let previous = e.execute(&rational, &mut work()).unwrap();
    let (correct, allocations) = observe(|| {
        let mut failed = work();
        matches!(
            e.execute(&pi, &mut failed),
            Err(R::ExactConstantEnclosureUnsupported)
        ) && failed.spent() > 0
            && previous.target().lower() == q(3, 10)
            && previous.source().encoded() == rational
            && e.execute(&rational, &mut work())
                .is_ok_and(|r| r.width() == Q::ZERO && r.target() == previous.target())
    });
    assert!(correct);
    assert_eq!(allocations, 0);

    let mut e =
        PreparedMathematicalEnclosureEvaluator::new(requirements(), 100_000_000, 4096).unwrap();
    let unsupported = encoded(2);
    let (correct, allocations) = observe(|| {
        matches!(
            e.execute(&unsupported, &mut work()),
            Err(R::UnsupportedTrigonometricEnclosure)
        ) && e
            .execute(&rational, &mut work())
            .is_ok_and(|r| r.width() == Q::ZERO)
    });
    assert!(correct);
    assert_eq!(allocations, 0);
}

#[test]
fn source_extent_malformed_input_and_caller_work_cannot_escape_the_prepared_profile() {
    let mut e =
        PreparedMathematicalEnclosureEvaluator::new(requirements(), 100_000_000, 4).unwrap();
    assert!(matches!(
        e.execute(&point(), &mut work()),
        Err(R::Numeric(ExactNumericRefusal::ExtentExceeded))
    ));
    assert!(matches!(
        e.verify(
            &[0; 5],
            Some(&[0; 4]),
            Q::ZERO,
            Q::ZERO,
            Q::ZERO,
            &mut work()
        ),
        Err(R::Numeric(ExactNumericRefusal::ExtentExceeded))
    ));
    assert!(matches!(
        e.execute(&[255, 0, 0, 0], &mut work()),
        Err(R::InvalidExpression)
    ));
    let source = encoded(0);
    let mut e =
        PreparedMathematicalEnclosureEvaluator::new(requirements(), 100_000_000, 4096).unwrap();
    let mut small = ExactNumericWork::new(1);
    assert!(matches!(
        e.execute(&source, &mut small),
        Err(R::Numeric(ExactNumericRefusal::WorkLimit))
    ));
    assert!(small.spent() <= 1);
    let mut large = ExactNumericWork::new(u64::MAX);
    let result = e.execute(&source, &mut large).unwrap();
    assert_eq!(result.admitted_limb_work(), 100_000_000);
    assert_eq!(large.spent(), result.limb_work());
}

#[test]
fn preparation_admits_scratch_retained_bytes_and_numerical_requirements() {
    let req = requirements();
    let (e, preparation_allocations) =
        observe(|| PreparedMathematicalEnclosureEvaluator::new(req, 100_000_000, 4096).unwrap());
    println!(
        "borrowed_enclosure_preparation allocations={preparation_allocations} scratch_payload_bytes={} retained_payload_bytes={} source_extent=4096 maximum_limb_work=100000000",
        e.scratch_payload_bytes(), e.retained_payload_bytes()
    );
    let exact = MathematicalEnclosureRequirements {
        maximum_scratch_payload_bytes: e.scratch_payload_bytes() as u32,
        maximum_retained_payload_bytes: e.retained_payload_bytes() as u32,
        ..req
    };
    assert!(PreparedMathematicalEnclosureEvaluator::new(exact, 100_000_000, 4096).is_ok());
    assert!(matches!(
        PreparedMathematicalEnclosureEvaluator::new(
            MathematicalEnclosureRequirements {
                maximum_scratch_payload_bytes: exact.maximum_scratch_payload_bytes - 1,
                ..exact
            },
            100_000_000,
            4096
        ),
        Err(R::ScratchStorageLimit)
    ));
    assert!(matches!(
        PreparedMathematicalEnclosureEvaluator::new(
            MathematicalEnclosureRequirements {
                maximum_retained_payload_bytes: exact.maximum_retained_payload_bytes - 1,
                ..exact
            },
            100_000_000,
            4096
        ),
        Err(R::RetainedStorageLimit)
    ));
    assert!(matches!(
        PreparedMathematicalEnclosureEvaluator::new(
            MathematicalEnclosureRequirements {
                maximum_width: Q::from_i128(-1),
                ..req
            },
            100_000_000,
            4096
        ),
        Err(R::NegativeTolerance)
    ));
    assert!(matches!(
        PreparedMathematicalEnclosureEvaluator::new(req, 0, 4096),
        Err(R::Numeric(ExactNumericRefusal::WorkLimit))
    ));
}

#[test]
fn finite_series_and_root_refinement_limits_remain_named_refusals() {
    let mut req = requirements();
    req.maximum_constant_terms = 0;
    let mut e = PreparedMathematicalEnclosureEvaluator::new(req, 100_000_000, 4096).unwrap();
    assert!(matches!(
        e.execute(&encoded(0), &mut work()),
        Err(R::ConstantTermLimit)
    ));
    let mut req = requirements();
    req.maximum_root_refinements = 0;
    let mut e = PreparedMathematicalEnclosureEvaluator::new(req, 100_000_000, 4096).unwrap();
    assert!(matches!(
        e.execute(&encoded(1), &mut work()),
        Err(R::Root(SquareRootRefusal::RefinementLimit))
    ));
}

#[test]
fn independent_verification_refuses_changed_source_bounds_and_width_without_allocation() {
    let source = encoded(1);
    let different = encoded(0);
    let mut e =
        PreparedMathematicalEnclosureEvaluator::new(requirements(), 100_000_000, 4096).unwrap();
    let expected = e.execute(&source, &mut work()).unwrap();
    let (lower, upper, width) = (
        expected.target().lower(),
        expected.target().upper(),
        expected.width(),
    );
    let false_lower = lower.checked_add(Q::ONE, &mut work()).unwrap();
    let (correct, allocations) = observe(|| {
        let mut spent = work();
        matches!(
            e.verify(&source, Some(&different), lower, upper, width, &mut spent),
            Err(R::SourceMismatch)
        ) && matches!(
            e.verify(
                &source,
                Some(&source),
                false_lower,
                upper,
                width,
                &mut spent
            ),
            Err(R::InvalidClaim)
        ) && matches!(
            e.verify(&source, Some(&source), lower, upper, Q::ZERO, &mut spent),
            Err(R::InvalidClaim)
        ) && spent.spent() > 0
            && e.verify(&source, Some(&source), lower, upper, width, &mut work())
                .is_ok_and(|r| r.target() == expected.target() && r.source().encoded() == source)
    });
    assert!(correct);
    assert_eq!(allocations, 0);
}

fn text(s: &str) -> ProjectionText<'_> {
    ProjectionText::new(s).unwrap()
}
fn report_input<'a, 'source>(
    source: &'a BorrowedMathematicalEnclosure<'source>,
    target: &'a MathematicalInterval,
    facts: &'a [ProjectionFact<'a, BorrowedMathematicalEnclosureDomain<'source>>],
) -> ProjectionInput<'a, BorrowedMathematicalEnclosureDomain<'source>> {
    ProjectionInput {
        basis: ProjectionBasis {
            report: text("borrowed/report@1"),
            source: text("borrowed/source@1"),
            target: Some(text("borrowed/target@1")),
            projector: text("borrowed/projector@1"),
            requested_route: text("borrowed/route@1"),
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
fn existing_projection_framework_rejects_false_exactness_omitted_facts_and_changed_policy() {
    let req = requirements();
    let mut e = PreparedMathematicalEnclosureEvaluator::new(req, 100_000_000, 4096).unwrap();
    let bytes = encoded(1);
    let source = e.execute(&bytes, &mut work()).unwrap();
    let target = source.target();
    let facts = source.facts();
    let domain = BorrowedMathematicalEnclosureDomain::new();
    let policy = BorrowedMathematicalEnclosurePolicy { requirements: req };
    let wrong = e.execute(&point(), &mut work()).unwrap().target();
    let false_facts = [
        ProjectionFact::Preserved {
            obligation: text("exact-denotation"),
        },
        ProjectionFact::Transformed {
            obligation: text("numeric-representation"),
            law: text("numeric/exact-interval-expression@1"),
        },
    ];
    let changed = BorrowedMathematicalEnclosurePolicy {
        requirements: MathematicalEnclosureRequirements {
            maximum_width: Q::ONE,
            ..req
        },
    };
    let (correct, allocations) = observe(|| {
        let report =
            ProjectionReport::new(&domain, &policy, report_input(&source, &target, &facts))
                .unwrap();
        report.require_exact().is_err()
            && report
                .require_policy(text("numeric/exact-interval-expression-policy@1"))
                .is_ok()
            && ProjectionReport::new(
                &domain,
                &policy,
                report_input(&source, &target, &facts[..1]),
            )
            .is_err()
            && ProjectionReport::new(&domain, &policy, report_input(&source, &wrong, &facts))
                .is_err()
            && ProjectionReport::new(
                &domain,
                &policy,
                report_input(&source, &target, &false_facts),
            )
            .is_err()
            && !changed.permits(&source, &target, &facts)
    });
    assert!(correct);
    assert_eq!(allocations, 0);
    let bytes = point();
    let source = e.execute(&bytes, &mut work()).unwrap();
    let target = source.target();
    let facts = source.facts();
    assert!(
        ProjectionReport::new(&domain, &policy, report_input(&source, &target, &facts))
            .unwrap()
            .require_exact()
            .is_ok()
    );
}
