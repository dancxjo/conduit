//! Compact intrinsic children share outer validation and preserve lazy failures.
use conduit_core::{
    ConfigurationValue, StructuredFieldValue as Field, StructuredInfoValue as Value,
};
use conduit_plot::*;
#[path = "common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn program(source: &str) -> PortableExpressionProgram {
    let checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, "choose", &ProfileCatalog::new()).unwrap();
    let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!()
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}
fn input(p: &PortableExpressionProgram, value: u8, gate: bool) -> Vec<u8> {
    let conduit_core::StructuredInfoTypeShape::Record { fields, .. } = p.input_type.shape() else {
        panic!()
    };
    Value::record(
        p.input_type.clone(),
        vec![
            Field::new(
                "count",
                Value::leaf(
                    fields
                        .iter()
                        .find(|f| f.name() == "count")
                        .unwrap()
                        .value_type()
                        .clone(),
                    conduit_core::encode_count(42).to_vec(),
                )
                .unwrap(),
            )
            .unwrap(),
            Field::new(
                "value",
                Value::leaf(
                    fields
                        .iter()
                        .find(|f| f.name() == "value")
                        .unwrap()
                        .value_type()
                        .clone(),
                    vec![value],
                )
                .unwrap(),
            )
            .unwrap(),
            Field::new(
                "gate",
                Value::leaf(
                    fields
                        .iter()
                        .find(|f| f.name() == "gate")
                        .unwrap()
                        .value_type()
                        .clone(),
                    vec![u8::from(gate)],
                )
                .unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}
const SOURCE: &str = "type Input = {
 count: Count
 value: U8
 gate: Boolean
}
type Output = {
 first: U8
 second: U8
}
plot choose (
 >> value: Input
 result: Output >>
) = ({ first: .gate ? (.value + 1) : 0, second: .value | 1 })
";
#[test]
fn fixed_field_arithmetic_and_lazy_branches_use_less_retained_storage_without_play_allocation() {
    let p = program(SOURCE);
    let (mut ordinary, before) =
        allocation_probe::observe(|| PreparedPortableExpressionEvaluator::new(&p).unwrap());
    let (mut compact, after) =
        allocation_probe::observe(|| PreparedPortableExpressionEvaluator::new_compact(&p).unwrap());
    eprintln!(
        "ordinary retained={} compact retained={} compact preparation peak={}",
        before.retained_bytes(),
        after.retained_bytes(),
        after.peak_bytes
    );
    assert!(after.retained_bytes() < before.retained_bytes());
    let cases: Vec<_> = [0, 1, 127, 254, 255]
        .into_iter()
        .flat_map(|n| [false, true].map(|g| input(&p, n, g)))
        .collect();
    let expected: Vec<_> = cases.iter().map(|bytes| p.evaluate(bytes)).collect();
    let mut malformed = cases[0].clone();
    malformed.pop();
    let mut trailing = cases[0].clone();
    trailing.push(0);
    let ((), memory) = allocation_probe::observe(|| {
        for _ in 0..8 {
            for (input, expected) in cases.iter().zip(&expected) {
                let a = ordinary.evaluate(input);
                let b = compact.evaluate(input);
                match expected {
                    Ok(bytes) => {
                        assert_eq!(a.unwrap(), bytes);
                        assert_eq!(b.unwrap(), bytes);
                    }
                    Err(error) => {
                        assert_eq!(a.unwrap_err(), *error);
                        assert_eq!(b.unwrap_err(), *error);
                    }
                }
            }
            for bytes in [&malformed, &trailing] {
                assert_eq!(ordinary.evaluate(bytes), compact.evaluate(bytes));
            }
        }
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
}
#[test]
fn nominal_profiles_keep_ordinary_representation_and_refusal_behavior() {
    let p = program(
        "type Small = U64 in 0..=10
plot choose (
 >> value: Small
 result: Small >>
) = (.)
",
    );
    let mut ordinary = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    let mut compact = PreparedPortableExpressionEvaluator::new_compact(&p).unwrap();
    assert!(compact.scalar_tape_limits().is_none());
    for n in [0u64, 1, 10, 11, u64::MAX] {
        let conduit_core::StructuredInfoTypeShape::Nominal { representation, .. } =
            p.input_type.shape()
        else {
            panic!()
        };
        let bytes = Value::nominal(
            p.input_type.clone(),
            Value::leaf(representation.clone(), n.to_le_bytes().to_vec()).unwrap(),
        )
        .unwrap()
        .canonical_bytes()
        .unwrap();
        let expected = ordinary.evaluate(&bytes);
        // This optimization does not supply a missing native-law witness.
        // Preserve the existing evaluator, including its acceptance boundary.
        if n <= 10 {
            assert!(expected.is_ok());
        }
        assert_eq!(expected, compact.evaluate(&bytes));
        assert!(compact.evaluate(&bytes[..bytes.len() - 1]).is_err());
        assert!(compact.evaluate(&n.to_le_bytes()).is_err());
    }
}

#[test]
fn boxed_boundary_predicates_survive_root_and_nested_scalar_lowering() {
    use conduit_core::{
        kind_id, CheckedValueContract, IntervalEndpoint, StructuredFieldType, StructuredInfoType,
        ValueConstraint,
    };
    fn contract(max: u8) -> CheckedValueContract {
        CheckedValueContract::new(
            kind_id("value/u8"),
            1,
            vec![ValueConstraint::FixedIntegerRange {
                minimum: None,
                maximum: Some(vec![max]),
                minimum_endpoint: IntervalEndpoint::Inclusive,
                maximum_endpoint: IntervalEndpoint::Inclusive,
            }],
        )
        .unwrap()
    }
    for nested in [false, true] {
        let mut p = program("plot choose (\n >> value: U8\n result: U8 >>\n) = (. + 1)\n");
        let root = PortableExpressionNode {
            value_type: p.output_type.clone(),
            operation: PortableExpressionOperation::ValueContract {
                input: Some(contract(20)),
                output: Some(contract(10)),
                value: Box::new(p.root),
            },
        };
        p.root = if nested {
            let output = StructuredInfoType::record(
                kind_id("test/contract-output"),
                vec![StructuredFieldType::new("value", p.output_type.clone()).unwrap()],
            )
            .unwrap();
            p.output_type = output.clone();
            PortableExpressionNode {
                value_type: output,
                operation: PortableExpressionOperation::Record(vec![("value".into(), root)]),
            }
        } else {
            root
        };
        let mut compact = PreparedPortableExpressionEvaluator::new_compact(&p).unwrap();
        let mut ordinary = PreparedPortableExpressionEvaluator::new(&p).unwrap();
        let cases: Vec<_> = [0u8, 9, 10, 20, 21, 255]
            .into_iter()
            .map(|n| (n, p.evaluate(&[n])))
            .collect();
        let ((), memory) = allocation_probe::observe(|| {
            for (n, expected) in &cases {
                let actual = compact.evaluate(&[*n]);
                let baseline = ordinary.evaluate(&[*n]);
                match expected {
                    Ok(bytes) => {
                        assert_eq!(actual.unwrap(), bytes);
                        assert_eq!(baseline.unwrap(), bytes);
                    }
                    Err(error) => {
                        assert_eq!(actual.unwrap_err(), *error);
                        assert_eq!(baseline.unwrap_err(), *error);
                    }
                }
            }
        });
        assert_eq!((memory.allocations, memory.reallocations), (0, 0));
        assert!(matches!(
            compact.evaluate(&[10]),
            Err(PortableExpressionEvaluationRefusal::OutputValueConstraint(
                _
            ))
        ));
        assert!(matches!(
            compact.evaluate(&[21]),
            Err(PortableExpressionEvaluationRefusal::InputValueConstraint(_))
        ));
    }
}

#[test]
fn checked_widening_lowers_to_tape_without_truncation_or_play_allocation() {
    let p = program("plot choose (\n >> value: U8\n result: I128 >>\n) = (value/i128(.) * -10)\n");
    let mut tape = PreparedPortableExpressionEvaluator::new_scalar_tape(&p).unwrap();
    let mut compact = PreparedPortableExpressionEvaluator::new_compact(&p).unwrap();
    assert!(compact.scalar_tape_limits().is_some());
    let cases: Vec<_> = (0..=255u8)
        .map(|n| (n, p.evaluate(&[n]).unwrap()))
        .collect();
    let ((), memory) = allocation_probe::observe(|| {
        for (n, expected) in &cases {
            assert_eq!(tape.evaluate(&[*n]).unwrap(), expected);
            assert_eq!(compact.evaluate(&[*n]).unwrap(), expected);
        }
        assert!(compact.evaluate(&[]).is_err());
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
}
