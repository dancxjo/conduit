use conduit_core::{
    projection::*, ExactNumericWork as Work, ExactRational as Q,
    MathematicalEnclosureDomain as Domain, MathematicalEnclosurePolicy as Policy,
    MathematicalEnclosureRefusal as R, MathematicalEnclosureRequirements as Requirements,
    MathematicalNode as Node, MathematicalValue as Value, MathematicalValueBuilder as Builder,
    PreparedMathematicalEnclosure as Prepared,
};
use core::cmp::Ordering;
fn work() -> Work {
    Work::new(100_000_000)
}
fn q(n: i128, d: i128) -> Q {
    Q::ratio(n, d, &mut work()).unwrap()
}
fn value(name: &str) -> Value {
    Value::parse_literal(name, &mut work()).unwrap()
}
fn requirements(width: Q) -> Requirements {
    Requirements {
        maximum_width: width,
        maximum_leaf_width: width,
        maximum_constant_terms: 32,
        maximum_root_refinements: 64,
        maximum_scratch_payload_bytes: 131072,
        maximum_retained_payload_bytes: 131072,
    }
}
#[test]
fn pi_tau_and_root_bounds_preserve_exact_source_and_tolerance() {
    for (name, lower, upper) in [
        (
            "π",
            q(3141592653589793, 1_000_000_000_000_000),
            q(3141592653589794, 1_000_000_000_000_000),
        ),
        (
            "τ",
            q(6283185307179586, 1_000_000_000_000_000),
            q(6283185307179587, 1_000_000_000_000_000),
        ),
    ] {
        let source = value(name);
        let mut req = requirements(q(1, 10_000_000_000_000_000));
        req.maximum_leaf_width = q(1, 100_000_000_000_000_000);
        let prepared = Prepared::prepare(&source, req, &mut work()).unwrap();
        assert_eq!(prepared.source(), &source);
        // Independent established decimal bracket, used only as a regression
        // oracle. The production certificate derives solely from the series.
        assert_eq!(
            prepared
                .target()
                .lower()
                .checked_compare(lower, &mut work())
                .unwrap(),
            Ordering::Greater
        );
        assert_eq!(
            prepared
                .target()
                .upper()
                .checked_compare(upper, &mut work())
                .unwrap(),
            Ordering::Less
        );
        assert_ne!(
            prepared
                .width()
                .checked_compare(req.maximum_width, &mut work())
                .unwrap(),
            Ordering::Greater
        );
        assert!(prepared.maximum_used_constant_terms() > 0);
        assert!(prepared.limb_work() > 0);
    }
    let mut b = Builder::new();
    let mut w = work();
    let two = b.insert(Node::Rational(Q::from_i128(2)), &mut w).unwrap();
    let root = b.insert(Node::PrincipalSquareRoot(two), &mut w).unwrap();
    let source = b.finish(root, &mut w).unwrap();
    let prepared = Prepared::prepare(&source, requirements(q(1, 10000)), &mut w).unwrap();
    let lo = prepared.target().lower();
    let hi = prepared.target().upper();
    assert_eq!(
        lo.checked_multiply(lo, &mut w)
            .unwrap()
            .checked_compare(Q::from_i128(2), &mut w)
            .unwrap(),
        Ordering::Less
    );
    assert_eq!(
        hi.checked_multiply(hi, &mut w)
            .unwrap()
            .checked_compare(Q::from_i128(2), &mut w)
            .unwrap(),
        Ordering::Greater
    );
}
#[test]
fn exact_literal_has_no_graph_scratch_and_exact_root_remains_exact() {
    let mut req = requirements(Q::ZERO);
    req.maximum_scratch_payload_bytes = 0;
    let source = value("0.1");
    let prepared = Prepared::prepare(&source, req, &mut work()).unwrap();
    assert_eq!(prepared.target().lower(), q(1, 10));
    assert_eq!(prepared.target().upper(), q(1, 10));
    assert_eq!(prepared.scratch_payload_bytes(), 0);
    let mut b = Builder::new();
    let mut w = work();
    let four = b.insert(Node::Rational(Q::from_i128(4)), &mut w).unwrap();
    let root = b.insert(Node::PrincipalSquareRoot(four), &mut w).unwrap();
    let prepared = Prepared::prepare(
        &b.finish(root, &mut w).unwrap(),
        requirements(Q::ZERO),
        &mut w,
    )
    .unwrap();
    assert_eq!(prepared.target().lower(), Q::from_i128(2));
    assert_eq!(prepared.target().upper(), Q::from_i128(2));
    assert_eq!(prepared.width(), Q::ZERO);
}
#[test]
fn algorithm_limits_and_final_composed_error_are_distinct_refusals() {
    let source = value("π");
    let req = requirements(q(1, 1000));
    let mut bad = req;
    bad.maximum_constant_terms = 0;
    assert_eq!(
        Prepared::prepare(&source, bad, &mut work()),
        Err(R::ConstantTermLimit)
    );
    let mut bad = req;
    bad.maximum_scratch_payload_bytes = 0;
    assert_eq!(
        Prepared::prepare(&source, bad, &mut work()),
        Err(R::ScratchStorageLimit)
    );
    let mut bad = req;
    bad.maximum_retained_payload_bytes = 0;
    assert_eq!(
        Prepared::prepare(&source, bad, &mut work()),
        Err(R::RetainedStorageLimit)
    );
    let mut bad = req;
    bad.maximum_width = Q::from_i128(-1);
    assert_eq!(
        Prepared::prepare(&source, bad, &mut work()),
        Err(R::NegativeTolerance)
    );
    let mut bad = req;
    bad.maximum_leaf_width = Q::ZERO;
    assert_eq!(
        Prepared::prepare(&source, bad, &mut work()),
        Err(R::ExactConstantEnclosureUnsupported)
    );
    assert!(matches!(
        Prepared::prepare(&source, req, &mut Work::new(0)),
        Err(R::Numeric(conduit_core::ExactNumericRefusal::WorkLimit))
    ));
    // A leaf certificate alone cannot justify the error of the full expression.
    let source = value("τ");
    let mut bad = req;
    bad.maximum_leaf_width = Q::ONE;
    assert_eq!(
        Prepared::prepare(&source, bad, &mut work()),
        Err(R::ToleranceExceeded)
    );
}
#[test]
fn signs_division_and_intermediate_dependencies_use_outward_interval_laws() {
    let mut b = Builder::new();
    let mut w = work();
    let pi = b.insert(Node::Pi, &mut w).unwrap();
    let negative = b.insert(Node::Negate(pi), &mut w).unwrap();
    let minus_two = b.insert(Node::Rational(Q::from_i128(-2)), &mut w).unwrap();
    let positive_half = b.insert(Node::Divide(negative, minus_two), &mut w).unwrap();
    let source = b.finish(positive_half, &mut w).unwrap();
    let prepared = Prepared::prepare(&source, requirements(q(1, 1000)), &mut w).unwrap();
    assert_eq!(
        prepared
            .target()
            .lower()
            .checked_compare(q(157, 100), &mut w)
            .unwrap(),
        Ordering::Greater
    );
    assert_eq!(
        prepared
            .target()
            .upper()
            .checked_compare(q(158, 100), &mut w)
            .unwrap(),
        Ordering::Less
    );
    let difference = b.insert(Node::Subtract(pi, pi), &mut w).unwrap();
    let source = b.finish(difference, &mut w).unwrap();
    assert_eq!(
        Prepared::prepare(&source, requirements(Q::ZERO), &mut w),
        Err(R::ExactConstantEnclosureUnsupported)
    );
    let prepared = Prepared::prepare(&source, requirements(q(1, 1000)), &mut w).unwrap();
    assert_ne!(
        prepared
            .target()
            .lower()
            .checked_compare(Q::ZERO, &mut w)
            .unwrap(),
        Ordering::Greater
    );
    assert_ne!(
        prepared
            .target()
            .upper()
            .checked_compare(Q::ZERO, &mut w)
            .unwrap(),
        Ordering::Less
    );
    // A shared π node is evaluated once but interval dependency is not falsely
    // treated as an exact algebraic simplification.
    assert_ne!(prepared.width(), Q::ZERO);
}
fn text(s: &str) -> ProjectionText<'_> {
    ProjectionText::new(s).unwrap()
}
fn input<'a>(
    source: &'a Prepared,
    target: &'a conduit_core::MathematicalInterval,
    facts: &'a [ProjectionFact<'a, Domain>],
) -> ProjectionInput<'a, Domain> {
    ProjectionInput {
        basis: ProjectionBasis {
            report: text("math/report@1"),
            source: text("math/source@1"),
            target: Some(text("math/target@1")),
            projector: text("math/projector@1"),
            requested_route: text("math/route@1"),
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
fn generic_projection_checks_complete_certificate_and_retained_policy() {
    let req = requirements(q(1, 1000));
    let source = Prepared::prepare(&value("π"), req, &mut work()).unwrap();
    let target = source.target();
    let facts = source.facts();
    let policy = Policy { requirements: req };
    let report = ProjectionReport::new(&Domain, &policy, input(&source, &target, &facts)).unwrap();
    assert!(report.require_exact().is_err());
    assert!(report.require_policy(policy.identity()).is_ok());
    let wrong = Prepared::prepare(&value("τ"), requirements(q(1, 10)), &mut work())
        .unwrap()
        .target();
    assert!(ProjectionReport::new(&Domain, &policy, input(&source, &wrong, &facts)).is_err());
    assert!(ProjectionReport::new(&Domain, &policy, input(&source, &target, &facts[..1])).is_err());
    let changed = Policy {
        requirements: requirements(q(1, 1_000_000)),
    };
    assert!(!changed.permits(&source, &target, &facts));
    assert!(
        ProjectionReport::new(&Domain, &changed, input(&source, &target, &facts))
            .unwrap()
            .require_policy(changed.identity())
            .is_err()
    );
}

#[test]
fn compound_pi_coordinates_and_root_cancellation_keep_algorithm_wide_bounds() {
    let mut b = Builder::new();
    let mut w = work();
    let pi = b.insert(Node::Pi, &mut w).unwrap();
    let d = b.insert(Node::Rational(Q::from_i128(180)), &mut w).unwrap();
    let root = b.insert(Node::Divide(pi, d), &mut w).unwrap();
    let source = b.finish(root, &mut w).unwrap();
    let mut req = requirements(q(1, 10_000_000_000_000_000_000));
    req.maximum_leaf_width = q(1, 1_000_000_000_000_000_000);
    let prepared = Prepared::prepare(&source, req, &mut w).unwrap();
    assert_eq!(
        prepared.source().semantic_digest(),
        source.semantic_digest()
    );
    let lower = q(17_453_292_519_943_295, 1_000_000_000_000_000_000);
    let upper = q(17_453_292_519_943_296, 1_000_000_000_000_000_000);
    assert_eq!(
        prepared
            .target()
            .lower()
            .checked_compare(lower, &mut w)
            .unwrap(),
        Ordering::Greater
    );
    assert_eq!(
        prepared
            .target()
            .upper()
            .checked_compare(upper, &mut w)
            .unwrap(),
        Ordering::Less
    );
    assert!(prepared.limb_work() <= prepared.admitted_limb_work());
    let two = b.insert(Node::Rational(Q::from_i128(2)), &mut w).unwrap();
    let sqrt = b.insert(Node::PrincipalSquareRoot(two), &mut w).unwrap();
    let square = b.insert(Node::Multiply(sqrt, sqrt), &mut w).unwrap();
    let cancellation = b.insert(Node::Subtract(square, two), &mut w).unwrap();
    let source = b.finish(cancellation, &mut w).unwrap();
    let mut req = requirements(q(1, 10000));
    req.maximum_leaf_width = q(1, 100000);
    let prepared = Prepared::prepare(&source, req, &mut w).unwrap();
    assert_eq!(
        prepared
            .target()
            .lower()
            .checked_compare(Q::ZERO, &mut w)
            .unwrap(),
        Ordering::Less
    );
    assert_eq!(
        prepared
            .target()
            .upper()
            .checked_compare(Q::ZERO, &mut w)
            .unwrap(),
        Ordering::Greater
    );
    assert_ne!(prepared.width(), Q::ZERO);
}
#[test]
fn certified_comparison_proves_only_exact_points_identity_or_disjointness() {
    use conduit_core::MathematicalComparison as C;
    let req = requirements(q(1, 1000));
    let pi = Prepared::prepare(&value("π"), req, &mut work()).unwrap();
    let tau = Prepared::prepare(&value("τ"), requirements(q(1, 100)), &mut work()).unwrap();
    assert_eq!(pi.compare(&tau, &mut work()), Ok(C::ProvedDifferent));
    assert_eq!(pi.compare(&pi, &mut work()), Ok(C::ProvedEqual));
    let coarse = Prepared::prepare(&value("π"), requirements(q(1, 10)), &mut work()).unwrap();
    let rational = Prepared::prepare(&value("3.1415"), req, &mut work()).unwrap();
    assert_eq!(coarse.compare(&rational, &mut work()), Ok(C::Unknown));
    let mut b = Builder::new();
    let mut w = work();
    let four = b.insert(Node::Rational(Q::from_i128(4)), &mut w).unwrap();
    let root = b.insert(Node::PrincipalSquareRoot(four), &mut w).unwrap();
    let exact_root = Prepared::prepare(
        &b.finish(root, &mut w).unwrap(),
        requirements(Q::ZERO),
        &mut w,
    )
    .unwrap();
    let exact_two = Prepared::prepare(&value("2"), requirements(Q::ZERO), &mut w).unwrap();
    assert_eq!(exact_root.compare(&exact_two, &mut w), Ok(C::ProvedEqual));
    assert!(matches!(
        pi.compare(&tau, &mut Work::new(0)),
        Err(R::Numeric(conduit_core::ExactNumericRefusal::WorkLimit))
    ));
}
