//! Ordinary exact Source operations retain bounded symbolic denotation graphs.
use conduit_core::*;
use conduit_plot::*;
#[path = "common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn work() -> ExactNumericWork {
    ExactNumericWork::new(100_000_000)
}
fn bytes(value: &MathematicalValue) -> Vec<u8> {
    let mut out = vec![0; MATHEMATICAL_VALUE_MAX_ENCODED_BYTES];
    let n = value.encode(&mut out).unwrap();
    out.truncate(n);
    out
}
fn insert(
    builder: &mut MathematicalValueBuilder,
    which: u8,
    w: &mut ExactNumericWork,
) -> MathematicalNodeId {
    match which {
        0 => {
            // The reviewed Angle reference writer retains (1/180) * pi.
            // pi / 180 is equal mathematically but has a different graph.
            let q = ExactRational::ratio(1, 180, w).unwrap();
            let factor = builder.insert(MathematicalNode::Rational(q), w).unwrap();
            let pi = builder.insert(MathematicalNode::Pi, w).unwrap();
            builder
                .insert(MathematicalNode::Multiply(factor, pi), w)
                .unwrap()
        }
        1 => {
            let two = builder
                .insert(MathematicalNode::Rational(ExactRational::from_i128(2)), w)
                .unwrap();
            builder
                .insert(MathematicalNode::PrincipalSquareRoot(two), w)
                .unwrap()
        }
        2 => builder
            .insert(
                MathematicalNode::Rational(ExactRational::ratio(3, 10, w).unwrap()),
                w,
            )
            .unwrap(),
        _ => builder
            .insert(MathematicalNode::Rational(ExactRational::ONE), w)
            .unwrap(),
    }
}
fn input(which: u8) -> Vec<u8> {
    let mut b = MathematicalValueBuilder::new();
    let mut w = work();
    let root = insert(&mut b, which, &mut w);
    bytes(&b.finish(root, &mut w).unwrap())
}
fn expected(operator: BinaryOperator, a: u8, b: u8) -> Vec<u8> {
    let mut builder = MathematicalValueBuilder::new();
    let mut w = work();
    let a = insert(&mut builder, a, &mut w);
    let b = insert(&mut builder, b, &mut w);
    let node = match operator {
        BinaryOperator::Add => MathematicalNode::Add(a, b),
        BinaryOperator::Subtract => MathematicalNode::Subtract(a, b),
        BinaryOperator::Multiply => MathematicalNode::Multiply(a, b),
        BinaryOperator::Divide => MathematicalNode::Divide(a, b),
        _ => panic!("operator"),
    };
    let root = builder.insert(node, &mut w).unwrap();
    bytes(&builder.finish(root, &mut w).unwrap())
}
fn program(expression: &str) -> PortableExpressionProgram {
    let source=format!("plot compute (\n >> value: MathematicalValue\n result: MathematicalValue >>\n) = ({expression})\n");
    let checked =
        check_syntax_document(&parse_syntax_document(&source), &StartupCatalog::new()).unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, "compute", &ProfileCatalog::new()).unwrap();
    let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!("program")
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}
#[test]
fn authored_symbolic_operations_match_existing_exact_core_construction() {
    for (expression, op, a, b) in [
        ("math/radians(1°) + math/sqrt(2)", BinaryOperator::Add, 0, 1),
        (
            "math/sqrt(2) - math/radians(1°)",
            BinaryOperator::Subtract,
            1,
            0,
        ),
        (
            "math/radians(1°) * math/sqrt(2)",
            BinaryOperator::Multiply,
            0,
            1,
        ),
        ("math/sqrt(2) / 1", BinaryOperator::Divide, 1, 3),
    ] {
        let p = program(expression);
        let input = input(2);
        let expected = expected(op, a, b);
        let mut prepared = PreparedPortableExpressionEvaluator::new(&p).unwrap();
        let capacity = prepared.output_capacity();
        assert_eq!(p.evaluate(&input).unwrap(), expected);
        let (_, observation) = allocation_probe::observe(|| {
            for _ in 0..16 {
                assert_eq!(prepared.evaluate(&input).unwrap(), expected);
            }
        });
        assert_eq!((observation.allocations, observation.reallocations), (0, 0));
        assert_eq!(prepared.output_capacity(), capacity);
    }
}
#[test]
fn changing_symbolic_and_rational_inputs_share_the_prepared_writer_without_growth() {
    let p = program(". + 1");
    let (mut prepared, preparation) =
        allocation_probe::observe(|| PreparedPortableExpressionEvaluator::new(&p).unwrap());
    println!("SYMBOLIC_ARITHMETIC_PREPARATION allocations={} reallocations={} peak_live_payload_bytes={} output_capacity={}",
        preparation.allocations, preparation.reallocations, preparation.peak_bytes, prepared.output_capacity());
    for which in 0..4 {
        let input = input(which);
        let expected = expected(BinaryOperator::Add, which, 3);
        assert_eq!(p.evaluate(&input).unwrap(), expected);
        let (_, observation) = allocation_probe::observe(|| {
            for _ in 0..32 {
                assert_eq!(prepared.evaluate(&input).unwrap(), expected);
            }
        });
        assert_eq!((observation.allocations, observation.reallocations), (0, 0));
    }
}
#[test]
fn symbolic_denominator_and_zero_division_refuse_with_named_causes_then_recover() {
    let p = program(". / .");
    let mut prepared = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    let symbolic = input(1);
    let rational = input(2);
    let one = input(3);
    let (_, observation) = allocation_probe::observe(|| {
        assert!(matches!(
            prepared.evaluate(&symbolic),
            Err(PortableExpressionEvaluationRefusal::MathematicalArithmetic(
                MathematicalValueRefusal::UnsupportedDomainProof
            ))
        ));
        assert_eq!(prepared.evaluate(&rational).unwrap(), one);
    });
    assert_eq!((observation.allocations, observation.reallocations), (0, 0));
    let p = program("math/sqrt(2) / (0 - 0)");
    let mut prepared = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    assert!(matches!(
        p.evaluate(&rational),
        Err(PortableExpressionEvaluationRefusal::MathematicalArithmetic(
            MathematicalValueRefusal::Numeric(ExactNumericRefusal::DivisionByZero)
        ))
    ));
    assert!(matches!(
        prepared.evaluate(&rational),
        Err(PortableExpressionEvaluationRefusal::MathematicalArithmetic(
            MathematicalValueRefusal::Numeric(ExactNumericRefusal::DivisionByZero)
        ))
    ));
    assert_eq!(
        mathematical_arithmetic_failure_detail(&MathematicalValueRefusal::UnsupportedDomainProof),
        38
    );
}
#[test]
fn bounded_symbolic_depth_refusal_does_not_poison_later_valid_inputs() {
    let mut b = MathematicalValueBuilder::new();
    let mut w = work();
    let mut root = b.insert(MathematicalNode::Pi, &mut w).unwrap();
    for n in 1..MATHEMATICAL_VALUE_MAX_DEPTH {
        let q = b
            .insert(
                MathematicalNode::Rational(ExactRational::from_i128(n as i128)),
                &mut w,
            )
            .unwrap();
        root = b.insert(MathematicalNode::Add(root, q), &mut w).unwrap();
    }
    let deep = bytes(&b.finish(root, &mut w).unwrap());
    let p = program(". + 1");
    let mut prepared = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    let good = input(1);
    let expected = expected(BinaryOperator::Add, 1, 3);
    let (_, observation) = allocation_probe::observe(|| {
        assert!(matches!(
            prepared.evaluate(&deep),
            Err(PortableExpressionEvaluationRefusal::MathematicalArithmetic(
                MathematicalValueRefusal::DepthBound
            ))
        ));
        assert_eq!(prepared.evaluate(&good).unwrap(), expected);
    });
    assert_eq!((observation.allocations, observation.reallocations), (0, 0));
}

#[test]
fn unselected_symbolic_failures_preserve_lazy_conditional_meaning() {
    let p = program(". == 1 ? 2 : math/sqrt(2) / math/sqrt(2)");
    let mut prepared = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    let one = input(3);
    let two = bytes(&MathematicalValue::rational(ExactRational::from_i128(2)));
    let zero = bytes(&MathematicalValue::rational(ExactRational::ZERO));
    assert_eq!(p.evaluate(&one).unwrap(), two);
    let (_, observation) = allocation_probe::observe(|| {
        assert_eq!(prepared.evaluate(&one).unwrap(), two);
        assert!(matches!(
            prepared.evaluate(&zero),
            Err(PortableExpressionEvaluationRefusal::MathematicalArithmetic(
                MathematicalValueRefusal::UnsupportedDomainProof
            ))
        ));
        assert_eq!(prepared.evaluate(&one).unwrap(), two);
    });
    assert_eq!((observation.allocations, observation.reallocations), (0, 0));
}

#[test]
fn previously_unsupported_pi_addition_now_retains_exact_shared_graph_data() {
    let mut b = MathematicalValueBuilder::new();
    let mut w = work();
    let pi = b.insert(MathematicalNode::Pi, &mut w).unwrap();
    let sum = b.insert(MathematicalNode::Add(pi, pi), &mut w).unwrap();
    let expected = bytes(&b.finish(sum, &mut w).unwrap());
    let input = bytes(&MathematicalValue::parse_literal("pi", &mut work()).unwrap());
    let p = program(". + .");
    assert_eq!(p.evaluate(&input).unwrap(), expected);
    let mut prepared = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    let (_, observation) = allocation_probe::observe(|| {
        for _ in 0..32 {
            assert_eq!(prepared.evaluate(&input).unwrap(), expected);
        }
    });
    assert_eq!((observation.allocations, observation.reallocations), (0, 0));
}
