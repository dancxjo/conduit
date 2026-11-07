//! CPU/static-code proof; not firmware placement, Root admission or a Play.
use conduit_core::{ConfigurationValue, PrimitiveInfoKind as K};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, generate_static_scalar_expression,
    parse_syntax_document, PortableExpressionEvaluationRefusal as R, PortableExpressionProgram,
    ProfileCatalog, ScalarTapeInstruction as I, ScalarTapeOperation as O, StartupCatalog,
    StaticScalarExpression,
};
use std::borrow::Cow;
#[path = "common/allocation_probe.rs"]
mod allocation_probe;
#[path = "common/compiled_rust.rs"]
mod compiled_rust;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_plot::BinaryOperator as B;
const fn instruction(kind: K, destination: usize, operation: O) -> I {
    I {
        kind,
        destination,
        operation,
    }
}
static LAZY: &[I] = &[
    instruction(K::U64, 0, O::Input),
    instruction(K::U64, 1, O::Literal(Cow::Borrowed(&[0; 8]))),
    instruction(K::Bool, 2, O::Binary(B::Equal, false, 0, 1)),
    instruction(K::U64, 3, O::BranchFalse(2, 7)),
    instruction(K::U64, 4, O::Literal(Cow::Borrowed(&[0; 8]))),
    instruction(K::U64, 3, O::Move(4)),
    instruction(K::U64, 6, O::Jump(11)),
    instruction(K::U64, 7, O::Input),
    instruction(K::U64, 8, O::Input),
    instruction(K::U64, 9, O::Binary(B::Divide, false, 7, 8)),
    instruction(K::U64, 3, O::Move(9)),
];
#[test]
fn resident_code_prepares_and_runs_without_heap_or_source_decode() {
    let (image, cost) =
        allocation_probe::observe(|| StaticScalarExpression::new(K::U64, K::U64, LAZY, 3).unwrap());
    assert_eq!(cost.allocations, 0);
    assert_eq!(cost.reallocations, 0);
    assert!(image.scratch_bytes() > 0);
    assert_eq!(image.input_bytes(), 8);
    assert_eq!(image.instruction_count(), LAZY.len());
    let mut output = [0xa5; 16];
    let (_, cost) = allocation_probe::observe(|| {
        for n in [0u64, 1, u64::MAX] {
            assert_eq!(
                image.evaluate_into(&n.to_le_bytes(), &mut output).unwrap(),
                8
            );
            assert_eq!(&output[..8], &u64::from(n != 0).to_le_bytes());
            assert_eq!(&output[8..], &[0xa5; 8]);
        }
        output.fill(0xa5);
        assert_eq!(
            image.evaluate_into(&[0; 7], &mut output),
            Err(R::InvalidInput)
        );
        assert_eq!(output, [0xa5; 16]);
        assert_eq!(
            image.evaluate_into(&[0; 8], &mut output[..7]),
            Err(R::InvalidProgram)
        );
        assert_eq!(output, [0xa5; 16]);
    });
    assert_eq!(cost.allocations, 0);
    assert_eq!(cost.reallocations, 0);
}
#[test]
fn resident_witnesses_allocate_nothing_and_refuse_before_publishing_output() {
    use conduit_core::{
        FixedIntegerContractSpec as Spec, FixedIntegerRange as Range, IntervalEndpoint::Inclusive,
    };
    static INPUT: Spec<'static> = Spec {
        value_kind: "value/u64",
        maximum_bytes: 8,
        range: Some(Range {
            minimum: Some(&0u64.to_le_bytes()),
            maximum: Some(&7u64.to_le_bytes()),
            minimum_endpoint: Inclusive,
            maximum_endpoint: Inclusive,
        }),
    };
    static OUTPUT: Spec<'static> = Spec {
        value_kind: "value/u64",
        maximum_bytes: 8,
        range: Some(Range {
            minimum: Some(&2u64.to_le_bytes()),
            maximum: Some(&7u64.to_le_bytes()),
            minimum_endpoint: Inclusive,
            maximum_endpoint: Inclusive,
        }),
    };
    static WRONG_KIND: Spec<'static> = Spec {
        value_kind: "value/u32",
        maximum_bytes: 4,
        range: None,
    };
    let (_, cost) = allocation_probe::observe(|| {
        let image = StaticScalarExpression::new(K::U64, K::U64, LAZY, 3)
            .unwrap()
            .with_value_contracts(Some(&INPUT), Some(&OUTPUT))
            .unwrap();
        assert_eq!(image.computation_work(), LAZY.len() as u16 + 10);
        let mut output = [0xa5; 16];
        assert_eq!(
            image.evaluate_into(&8u64.to_le_bytes(), &mut output),
            Err(R::InputValueConstraint(
                conduit_core::ValueConstraintRefusal::FixedIntegerRange
            ))
        );
        assert_eq!(output, [0xa5; 16]);
        assert_eq!(
            image.evaluate_into(&1u64.to_le_bytes(), &mut output),
            Err(R::OutputValueConstraint(
                conduit_core::ValueConstraintRefusal::FixedIntegerRange
            ))
        );
        assert_eq!(output, [0xa5; 16]);
        let good = StaticScalarExpression::new(K::U64, K::U64, LAZY, 3)
            .unwrap()
            .with_value_contracts(Some(&INPUT), Some(&INPUT))
            .unwrap();
        assert_eq!(
            good.evaluate_into(&1u64.to_le_bytes(), &mut output)
                .unwrap(),
            8
        );
        assert_eq!(&output[..8], &1u64.to_le_bytes());
        assert!(matches!(
            StaticScalarExpression::new(K::U64, K::U64, LAZY, 3)
                .unwrap()
                .with_value_contracts(Some(&WRONG_KIND), None),
            Err(R::InvalidProgram)
        ));
    });
    assert_eq!(cost.allocations, 0);
    assert_eq!(cost.reallocations, 0);
}

#[test]
fn resident_instruction_limit_is_unchanged_and_checked_before_execution() {
    static MAXIMUM: [I; 128] = [const { instruction(K::U8, 0, O::Input) }; 128];
    static OVERSIZE: [I; 129] = [const { instruction(K::U8, 0, O::Input) }; 129];
    let (_, cost) = allocation_probe::observe(|| {
        let image = StaticScalarExpression::new(K::U8, K::U8, &MAXIMUM, 0).unwrap();
        let mut output = [0];
        assert_eq!(image.evaluate_into(&[7], &mut output).unwrap(), 1);
        assert_eq!(output, [7]);
        assert!(matches!(
            StaticScalarExpression::new(K::U8, K::U8, &OVERSIZE, 0),
            Err(R::InvalidProgram)
        ));
    });
    assert_eq!(cost.allocations, 0);
    assert_eq!(cost.reallocations, 0);
}
#[test]
fn invalid_control_types_and_consumed_values_refuse_before_execution_without_allocation() {
    static CYCLE: &[I] = &[instruction(K::U8, 0, O::Jump(0))];
    static UNINITIALIZED: &[I] = &[
        instruction(K::U8, 0, O::Jump(1)),
        instruction(K::U8, 1, O::Move(0)),
    ];
    static ONE_BRANCH: &[I] = &[
        instruction(K::Bool, 0, O::Input),
        instruction(K::U8, 1, O::BranchFalse(0, 3)),
        instruction(K::U8, 1, O::Literal(Cow::Borrowed(&[1]))),
    ];
    static CONSUMED: &[I] = &[
        instruction(K::U8, 0, O::Input),
        instruction(K::U8, 1, O::Move(0)),
        instruction(K::U8, 2, O::Move(0)),
    ];
    static BAD_BOOL: &[I] = &[instruction(K::Bool, 0, O::Literal(Cow::Borrowed(&[2])))];
    static BAD_PROOF: &[I] = &[
        instruction(K::U8, 0, O::Input),
        instruction(K::U8, 1, O::Binary(B::Add, true, 0, 0)),
    ];
    let (_, cost) = allocation_probe::observe(|| {
        for (input, output, code, result) in [
            (K::U8, K::U8, CYCLE, 0),
            (K::U8, K::U8, UNINITIALIZED, 1),
            (K::Bool, K::U8, ONE_BRANCH, 1),
            (K::U8, K::U8, CONSUMED, 2),
            (K::Bool, K::Bool, BAD_BOOL, 0),
            (K::U8, K::U8, BAD_PROOF, 1),
        ] {
            assert!(matches!(
                StaticScalarExpression::new(input, output, code, result),
                Err(R::InvalidProgram)
            ));
        }
    });
    assert_eq!(cost.allocations, 0);
    assert_eq!(cost.reallocations, 0);
}
fn program(source: &str, entry: &str) -> PortableExpressionProgram {
    let checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    let expanded = expand_canonical_plot_for_authoring(&checked, entry, &ProfileCatalog::new())
        .unwrap()
        .expanded;
    let ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value else {
        panic!("program")
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}
#[test]
fn unsupported_record_input_refuses_explicitly_without_runtime_fallback() {
    let p = program("type Envelope = {\n value: U64\n}\nplot constant (\n input: Envelope >> result: U64\n) = (7)\n", "constant");
    assert!(
        matches!(generate_static_scalar_expression(&p), Err(R::UnsupportedType(reason)) if reason == "scalar-image/fixed-input@1")
    );
}
fn boundary_programs() -> Vec<PortableExpressionProgram> {
    use conduit_core::{
        CheckedValueContract, IntervalEndpoint, StructuredInfoTypeShape, ValueConstraint,
    };
    use conduit_plot::{PortableExpressionNode, PortableExpressionOperation};
    let base = program(
        "plot add (\n input: U64 >> result: U64\n) = (. + 1)\n",
        "add",
    );
    let StructuredInfoTypeShape::Leaf(kind) = base.input_type.shape() else {
        panic!("leaf")
    };
    let bound = CheckedValueContract::new(
        kind.clone(),
        8,
        vec![ValueConstraint::FixedIntegerRange {
            minimum: Some(0u64.to_le_bytes().to_vec()),
            maximum: Some(7u64.to_le_bytes().to_vec()),
            minimum_endpoint: IntervalEndpoint::Inclusive,
            maximum_endpoint: IntervalEndpoint::Inclusive,
        }],
    )
    .unwrap();
    [(true, false), (false, true), (true, true)]
        .into_iter()
        .map(|(input, output)| {
            let mut p = base.clone();
            p.root = PortableExpressionNode {
                value_type: p.output_type.clone(),
                operation: PortableExpressionOperation::ValueContract {
                    input: input.then(|| bound.clone()),
                    output: output.then(|| bound.clone()),
                    value: Box::new(p.root),
                },
            };
            p
        })
        .collect()
}
#[test]
fn checked_boundary_predicates_are_retained_once_and_other_profiles_refuse() {
    for p in boundary_programs() {
        let code = generate_static_scalar_expression(&p).unwrap();
        assert!(code.contains("FixedIntegerContractSpec"));
        assert_eq!(
            code.matches("static INPUT_CONTRACT").count(),
            usize::from(matches!(
                p.root.operation,
                conduit_plot::PortableExpressionOperation::ValueContract { input: Some(_), .. }
            ))
        );
        if matches!(
            p.root.operation,
            conduit_plot::PortableExpressionOperation::ValueContract {
                input: Some(_),
                output: Some(_),
                ..
            }
        ) {
            assert!(
                !code.contains("static OUTPUT_CONTRACT"),
                "identical guards share immutable metadata"
            );
        }
    }
    let mut p = boundary_programs().remove(0);
    let conduit_plot::PortableExpressionOperation::ValueContract {
        input: Some(contract),
        ..
    } = &mut p.root.operation
    else {
        panic!("input")
    };
    contract.constraints = vec![conduit_core::ValueConstraint::ByteLength {
        minimum: 0,
        maximum: 8,
    }];
    assert!(
        matches!(generate_static_scalar_expression(&p), Err(R::UnsupportedType(reason))
        if reason == "scalar-image/value-contract-witness-unavailable@1")
    );
}

#[test]
fn generated_resident_programs_compile_no_std_and_match_checked_source() {
    use sha2::Digest;
    let mut generated = String::new();
    let mut exercise = String::from("fn main() {\n");
    let mut cases = Vec::new();
    for operations in [1, 3, 8, 32] {
        cases.push(program(
            include_str!("fixtures/expression_baseline.conduit"),
            &format!("baseline-{operations}"),
        ));
        cases.push(program(
            include_str!("fixtures/expression_boolean_baseline.conduit"),
            &format!("boolean-{operations}"),
        ));
    }
    cases.push(program(
        "plot lazy (\n input: U64 >> result: U64\n) = (. == 0 ? 0 : . / .)\n",
        "lazy",
    ));
    cases.push(program(
        "plot add (\n input: I128 >> result: I128\n) = (. + 1)\n",
        "add",
    ));
    cases.extend(boundary_programs());
    for (index, p) in cases.iter().enumerate() {
        let first = generate_static_scalar_expression(p).unwrap();
        assert_eq!(
            first,
            generate_static_scalar_expression(p).unwrap(),
            "deterministic Make output"
        );
        if index == 8 {
            assert_eq!(
                first.matches("const LITERAL_").count(),
                1,
                "repeated zero literals share one resident slice"
            );
        }
        generated.push_str(&format!("mod image_{index} {{\n{first}\n}}\n"));
        let digest = sha2::Sha256::digest(p.canonical_bytes().unwrap());
        exercise.push_str(&format!(
            "assert_eq!(image_{index}::SOURCE_PROGRAM_SHA256, {digest:?});\n"
        ));
        exercise.push_str(&format!(
            "let image = image_{index}::prepare().unwrap();\nlet mut output = [0xa5u8;16];\n"
        ));
        let conduit_core::StructuredInfoTypeShape::Leaf(kind) = p.input_type.shape() else {
            panic!("leaf")
        };
        let inputs: Vec<Vec<u8>> = match conduit_core::primitive_info_kind(kind.as_str()).unwrap() {
            K::I128 => [0i128, 1, -1, i128::MAX]
                .iter()
                .map(|n| n.to_le_bytes().to_vec())
                .collect(),
            K::Bool => vec![vec![0], vec![1], vec![2], vec![], vec![0, 1]],
            K::U64 => [0u64, 1, 7, 8, u64::MAX / 64, u64::MAX]
                .iter()
                .map(|n| n.to_le_bytes().to_vec())
                .chain([vec![0]])
                .collect(),
            other => panic!("unexpected test profile {other:?}"),
        };
        for input in inputs {
            let expected = p.evaluate(&input);
            match expected {
                Ok(bytes) => exercise.push_str(&format!("let n = image.evaluate_into(&{input:?}, &mut output).unwrap(); assert_eq!(&output[..n], &{bytes:?});\n")),
                Err(error) => exercise.push_str(&format!("assert_eq!(format!(\"{{:?}}\", image.evaluate_into(&{input:?}, &mut output).unwrap_err()), {:?});\n", format!("{error:?}"))),
            }
        }
    }
    exercise.push_str("}\n");
    compiled_rust::verify(&generated, &exercise, "static-scalar");
}

#[test]
fn strict_integer_widening_preserves_sign_and_rejects_lossy_static_tapes() {
    static SIGNED: &[I] = &[
        instruction(K::I8, 0, O::Input),
        instruction(K::I128, 1, O::Widen(0)),
    ];
    static UNSIGNED: &[I] = &[
        instruction(K::U8, 0, O::Input),
        instruction(K::I128, 1, O::Widen(0)),
    ];
    let signed = StaticScalarExpression::new(K::I8, K::I128, SIGNED, 1).unwrap();
    let unsigned = StaticScalarExpression::new(K::U8, K::I128, UNSIGNED, 1).unwrap();
    let mut output = [0; 16];
    for value in [i8::MIN, -1, 0, 1, i8::MAX] {
        signed
            .evaluate_into(&value.to_le_bytes(), &mut output)
            .unwrap();
        assert_eq!(i128::from_le_bytes(output), i128::from(value));
    }
    for value in [0u8, 1, 127, 128, 255] {
        unsigned.evaluate_into(&[value], &mut output).unwrap();
        assert_eq!(i128::from_le_bytes(output), i128::from(value));
    }
    for (input, output) in [
        (K::U8, K::I8),
        (K::I8, K::U16),
        (K::U64, K::U8),
        (K::U8, K::U8),
        (K::Bool, K::U16),
    ] {
        let code = Box::leak(
            vec![
                instruction(input, 0, O::Input),
                instruction(output, 1, O::Widen(0)),
            ]
            .into_boxed_slice(),
        );
        assert!(StaticScalarExpression::new(input, output, code, 1).is_err());
    }
}
