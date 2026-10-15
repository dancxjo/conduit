//! Exact imported mathematical data through checked Source and finite projection.
//! No new symbolic expression language or eligible scalar realization is involved.
use conduit_core::{
    ConfigurationValue, ExactNumericWork as Work, ExactRational as Q,
    MathematicalEnclosureRefusal as Refusal, MathematicalEnclosureRequirements as Requirements,
    MathematicalNode as Node, MathematicalValue as Value, MathematicalValueBuilder as Builder,
    PreparedMathematicalEnclosure as Projection, SquareRootRefusal,
    MATHEMATICAL_VALUE_MAX_ENCODED_BYTES,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionEvaluationRefusal, PortableExpressionProgram,
    PreparedPortableExpressionEvaluator, ProfileCatalog, StartupCatalog,
};
use core::cmp::Ordering;
#[path = "common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn work() -> Work {
    Work::new(100_000_000)
}
fn q(n: i128, d: i128) -> Q {
    Q::ratio(n, d, &mut work()).unwrap()
}
fn specimens() -> [(Value, &'static str, Q, Q); 2] {
    let mut w = work();
    let mut pi = Builder::new();
    let constant = pi.insert(Node::Pi, &mut w).unwrap();
    let denominator = pi
        .insert(Node::Rational(Q::from_i128(180)), &mut w)
        .unwrap();
    let divided = pi
        .insert(Node::Divide(constant, denominator), &mut w)
        .unwrap();
    let pi_over_180 = pi.finish(divided, &mut w).unwrap();
    let mut root = Builder::new();
    let two = root
        .insert(Node::Rational(Q::from_i128(2)), &mut w)
        .unwrap();
    let square_root = root.insert(Node::PrincipalSquareRoot(two), &mut w).unwrap();
    let sqrt_two = root.finish(square_root, &mut w).unwrap();
    [
        (
            pi_over_180,
            "0.017453292519943295",
            q(3141592653589793, 180000000000000000),
            q(3141592653589794, 180000000000000000),
        ),
        (
            sqrt_two,
            "1.4142135623730951",
            q(1414213562373095, 1000000000000000),
            q(1414213562373096, 1000000000000000),
        ),
    ]
}
fn encode(value: &Value) -> Vec<u8> {
    let mut bytes = vec![0; MATHEMATICAL_VALUE_MAX_ENCODED_BYTES];
    let len = value.encode(&mut bytes).unwrap();
    bytes.truncate(len);
    bytes
}
fn program(expression: &str, output: &str) -> PortableExpressionProgram {
    let source = format!(
        "plot consumer (\n >> value: MathematicalValue\n result: {output} >>\n) = ({expression})\n"
    );
    let checked =
        check_syntax_document(&parse_syntax_document(&source), &StartupCatalog::new()).unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, "consumer", &ProfileCatalog::new()).unwrap();
    let ConfigurationValue::Text(bytes) = &expanded.expanded.gears[0].configuration[0].value else {
        panic!("ordinary expression Back")
    };
    PortableExpressionProgram::from_canonical_hex(bytes).unwrap()
}
fn requirements() -> Requirements {
    Requirements {
        maximum_width: q(1, 10000000000000000),
        maximum_leaf_width: q(1, 100000000000000000),
        maximum_constant_terms: 32,
        maximum_root_refinements: 64,
        maximum_scratch_payload_bytes: 131072,
        maximum_retained_payload_bytes: 131072,
    }
}
#[test]
fn pi_over_180_and_sqrt_two_keep_exact_source_bytes_before_certified_projection() {
    let transport = program(".", "MathematicalValue");
    let mut prepared = PreparedPortableExpressionEvaluator::new(&transport).unwrap();
    for (value, _, lower, upper) in specimens() {
        assert!(value.as_rational().is_none());
        let original = encode(&value);
        assert_eq!(transport.evaluate(&original).unwrap(), original);
        let (same, allocation) = allocation_probe::observe(|| {
            (0..16).all(|_| prepared.evaluate(&original).unwrap() == original)
        });
        assert!(same);
        assert_eq!((allocation.allocations, allocation.reallocations), (0, 0));
        let restored = Value::decode(prepared.evaluate(&original).unwrap(), &mut work()).unwrap();
        assert_eq!(restored.semantic_digest(), value.semantic_digest());
        assert_eq!(encode(&restored), original);
        let req = requirements();
        let projection = Projection::prepare(&restored, req, &mut work()).unwrap();
        assert_eq!(projection.source(), &value);
        assert_ne!(
            projection
                .target()
                .lower()
                .checked_compare(lower, &mut work())
                .unwrap(),
            Ordering::Less
        );
        assert_ne!(
            projection
                .target()
                .upper()
                .checked_compare(upper, &mut work())
                .unwrap(),
            Ordering::Greater
        );
        assert_ne!(
            projection
                .width()
                .checked_compare(req.maximum_width, &mut work())
                .unwrap(),
            Ordering::Greater
        );
        assert!(projection.scratch_payload_bytes() <= req.maximum_scratch_payload_bytes);
        assert!(projection.retained_payload_bytes() <= req.maximum_retained_payload_bytes);
        assert!(projection.limb_work() <= projection.admitted_limb_work());
        assert_eq!(encode(projection.source()), original);
    }
}
#[test]
fn rounded_values_do_not_become_exact_and_zero_width_requests_refuse() {
    for (value, approximation, _, _) in specimens() {
        let bytes = encode(&value);
        let rounded = Value::parse_literal(approximation, &mut work()).unwrap();
        assert_ne!(rounded.semantic_digest(), value.semantic_digest());
        let comparison = program(&format!(". == {approximation}"), "Boolean");
        let mut prepared = PreparedPortableExpressionEvaluator::new(&comparison).unwrap();
        assert_eq!(
            comparison.evaluate(&bytes),
            Err(PortableExpressionEvaluationRefusal::UnprovedMathematicalComparison)
        );
        assert_eq!(
            prepared.evaluate(&bytes),
            Err(PortableExpressionEvaluationRefusal::UnprovedMathematicalComparison)
        );
        let mut exact = requirements();
        exact.maximum_width = Q::ZERO;
        exact.maximum_leaf_width = Q::ZERO;
        assert!(matches!(
            Projection::prepare(&value, exact, &mut work()),
            Err(Refusal::ExactConstantEnclosureUnsupported
                | Refusal::ToleranceExceeded
                | Refusal::Root(SquareRootRefusal::RefinementLimit))
        ));
    }
}
