//! Source-authored exact root data and a bounded prepared writer.
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
    let mut output = vec![0; MATHEMATICAL_VALUE_MAX_ENCODED_BYTES];
    let n = value.encode(&mut output).unwrap();
    output.truncate(n);
    output
}
fn number(n: i128) -> Vec<u8> {
    bytes(&MathematicalValue::rational(ExactRational::from_i128(n)))
}
fn source(input: &str, expression: &str) -> String {
    format!("plot root (\n >> value: {input}\n result: MathematicalValue >>\n) = ({expression})\n")
}
fn compile(source: &str) -> Result<PortableExpressionProgram, String> {
    let checked = check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new())
        .map_err(|e| format!("{e:?}"))?;
    let expanded = expand_canonical_plot_for_authoring(&checked, "root", &ProfileCatalog::new())
        .map_err(|e| format!("{e:?}"))?;
    let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!("program")
    };
    Ok(PortableExpressionProgram::from_canonical_hex(encoded).unwrap())
}
fn expected(n: i128) -> Vec<u8> {
    let mut b = MathematicalValueBuilder::new();
    let q = b
        .insert(
            MathematicalNode::Rational(ExactRational::from_i128(n)),
            &mut work(),
        )
        .unwrap();
    let root = b
        .insert(MathematicalNode::PrincipalSquareRoot(q), &mut work())
        .unwrap();
    bytes(&b.finish(root, &mut work()).unwrap())
}
#[test]
fn source_root_and_dynamic_prepared_writer_preserve_exact_data_without_warm_allocations() {
    let constant = compile(&source("MathematicalValue", "math/sqrt(2)")).unwrap();
    let dynamic = compile(&source("MathematicalValue", "math/sqrt(.)")).unwrap();
    let mut folded = PreparedPortableExpressionEvaluator::new(&constant).unwrap();
    let mut prepared = PreparedPortableExpressionEvaluator::new(&dynamic).unwrap();
    let capacity = prepared.output_capacity();
    assert_eq!(constant.evaluate(&number(0)).unwrap(), expected(2));
    assert_eq!(folded.evaluate(&number(0)).unwrap(), expected(2));
    for n in [0, 1, 2, 3, 81] {
        let input = number(n);
        let expected = expected(n);
        assert_eq!(dynamic.evaluate(&input).unwrap(), expected);
        let (same, allocation) = allocation_probe::observe(|| {
            (0..32).all(|_| prepared.evaluate(&input).unwrap() == expected)
        });
        assert!(same);
        assert_eq!((allocation.allocations, allocation.reallocations), (0, 0));
        assert_eq!(prepared.output_capacity(), capacity);
    }
    // sqrt(2) remains an exact root graph, never a claimed exact decimal.
    assert!(MathematicalValue::decode(&expected(2), &mut work())
        .unwrap()
        .as_rational()
        .is_none());
}
#[test]
fn wrong_types_arity_domains_and_unproved_symbolic_inputs_refuse_and_writer_recovers() {
    for (input, expression) in [
        ("Angle", "math/sqrt(.)"),
        ("Distance", "math/sqrt(.)"),
        ("F64", "math/sqrt(.)"),
        ("MathematicalValue", "math/sqrt(1, 2)"),
    ] {
        assert!(
            compile(&source(input, expression)).is_err(),
            "{input}: {expression}"
        );
    }
    // The existing unary-expression checker rejects a closed negative literal
    // before preparation; dynamic values retain the precise runtime domain cause.
    assert!(compile(&source("MathematicalValue", "math/sqrt(-1)")).is_err());
    let dynamic = compile(&source("MathematicalValue", "math/sqrt(.)")).unwrap();
    let mut prepared = PreparedPortableExpressionEvaluator::new(&dynamic).unwrap();
    let negative = number(-1);
    let pi = bytes(&MathematicalValue::parse_literal("pi", &mut work()).unwrap());
    let good = number(2);
    let expected = expected(2);
    let (_, allocation) = allocation_probe::observe(|| {
        assert!(matches!(
            prepared.evaluate(&negative),
            Err(PortableExpressionEvaluationRefusal::SquareRoot(
                MathematicalValueRefusal::NegativeRadicand
            ))
        ));
        assert!(matches!(
            prepared.evaluate(&pi),
            Err(PortableExpressionEvaluationRefusal::SquareRoot(
                MathematicalValueRefusal::UnsupportedDomainProof
            ))
        ));
        assert_eq!(prepared.evaluate(&good).unwrap(), expected);
    });
    assert_eq!((allocation.allocations, allocation.reallocations), (0, 0));
    assert_eq!(
        square_root_failure_detail(&MathematicalValueRefusal::NegativeRadicand),
        33
    );
    assert_eq!(
        square_root_failure_detail(&MathematicalValueRefusal::UnsupportedDomainProof),
        34
    );
}
