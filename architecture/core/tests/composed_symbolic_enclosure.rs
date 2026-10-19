//! Finite work for a composed exact graph, separate from Source/kernel proof.
use conduit_core::*;
#[path = "common/numerical_allocation.rs"]
mod allocation_probe;
use allocation_probe::observe;
fn work() -> ExactNumericWork {
    ExactNumericWork::new(200_000_000)
}
fn q(n: i128, d: i128) -> ExactRational {
    ExactRational::ratio(n, d, &mut work()).unwrap()
}
fn source() -> Vec<u8> {
    let mut b = MathematicalValueBuilder::new();
    let mut w = work();
    let two = b
        .insert(MathematicalNode::Rational(q(2, 1)), &mut w)
        .unwrap();
    let root = b
        .insert(MathematicalNode::PrincipalSquareRoot(two), &mut w)
        .unwrap();
    let factor = b
        .insert(MathematicalNode::Rational(q(1, 180)), &mut w)
        .unwrap();
    let pi = b.insert(MathematicalNode::Pi, &mut w).unwrap();
    let coordinate = b
        .insert(MathematicalNode::Multiply(factor, pi), &mut w)
        .unwrap();
    let shifted = b
        .insert(MathematicalNode::Add(root, coordinate), &mut w)
        .unwrap();
    let three = b
        .insert(MathematicalNode::Rational(q(3, 1)), &mut w)
        .unwrap();
    let scaled = b
        .insert(MathematicalNode::Multiply(shifted, three), &mut w)
        .unwrap();
    let result = b
        .insert(MathematicalNode::Divide(scaled, two), &mut w)
        .unwrap();
    let value = b.finish(result, &mut w).unwrap();
    let mut encoded = vec![0; MATHEMATICAL_VALUE_MAX_ENCODED_BYTES];
    let n = value.encode(&mut encoded).unwrap();
    encoded.truncate(n);
    encoded
}
#[test]
fn composed_root_pi_scale_has_an_explicit_work_bound_and_refuses_smaller_grants() {
    let source = source();
    let requirements = MathematicalEnclosureRequirements {
        maximum_width: q(1, 100_000_000_000_000_000),
        maximum_leaf_width: q(1, 6_400_000_000_000_000_000),
        maximum_constant_terms: 32,
        maximum_root_refinements: 64,
        maximum_scratch_payload_bytes: 131072,
        maximum_retained_payload_bytes: 131072,
    };
    let mut evaluator = PreparedMathematicalEnclosureEvaluator::new(
        requirements,
        200_000_000,
        MATHEMATICAL_VALUE_MAX_ENCODED_BYTES,
    )
    .unwrap();
    let lower = ExactRational::parse_decimal("2.1475002823395574", &mut work()).unwrap();
    let upper = ExactRational::parse_decimal("2.1475002823395578", &mut work()).unwrap();
    let (_, allocations) = observe(|| {
        let mut small = ExactNumericWork::new(100_000_000);
        assert!(matches!(
            evaluator.execute(&source, &mut small),
            Err(MathematicalEnclosureRefusal::Numeric(
                ExactNumericRefusal::WorkLimit
            ))
        ));
        assert!(small.spent() <= 100_000_000);
        let mut admitted = work();
        let result = evaluator.execute(&source, &mut admitted).unwrap();
        assert_eq!(result.source().encoded(), source);
        assert_ne!(result.width(), ExactRational::ZERO);
        assert!(!result
            .width()
            .checked_compare(requirements.maximum_width, &mut work())
            .unwrap()
            .is_gt());
        assert!(!result
            .target()
            .lower()
            .checked_compare(lower, &mut work())
            .unwrap()
            .is_lt());
        assert!(!result
            .target()
            .upper()
            .checked_compare(upper, &mut work())
            .unwrap()
            .is_gt());
        // Keep diagnostics outside this guarded path: formatting can allocate.
        (result.limb_work(), admitted.spent())
    });
    assert_eq!(allocations, 0);
    let mut admitted = work();
    let result = evaluator.execute(&source, &mut admitted).unwrap();
    println!(
        "COMPOSED_ENCLOSURE_WORK nodes={} spent={} admitted={} caller_spent={} source_digest={:?}",
        result.source().node_count(),
        result.limb_work(),
        result.admitted_limb_work(),
        admitted.spent(),
        result.source().semantic_digest()
    );
}
