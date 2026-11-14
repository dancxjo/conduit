//! Exact child descriptors repeat at original branch execution points.
use super::*;
#[test]
fn repeated_branch_descriptors_keep_missing_projections_lazy_and_values_fresh() {
    let d=checked("type Input = {\n flag: Boolean\n values: sequence U8 <= 2\n}\ntype Output = {\n left: U8\n right: U8\n}\nplot choose (\n input: Input...| >> output: Output...|\n) = .flag ? {left: .values.1, right: .values.1} : (.flag ? {left: .values.1, right: .values.1} : {left: .values.0, right: .values.0})\n");
    let p = program(&d, "choose");
    let ty = member(&p.input_type, "values");
    let StructuredInfoTypeShape::Sequence { element, .. } = ty.shape() else {
        panic!("sequence")
    };
    let mut prepared = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    let mut main=String::from("fn main(){let mut e=image::prepare().unwrap();assert_eq!(image::CHILD_COUNT,3);let mut output=[0xa5;4096];let mut scratch=[0;4096];");
    for (flag, values) in [
        (false, vec![7]),
        (true, vec![1, 9]),
        (true, vec![3]),
        (false, vec![6]),
        (false, vec![]),
        (true, vec![2, 8]),
    ] {
        let input = record(
            &p.input_type,
            vec![
                ("flag", leaf(member(&p.input_type, "flag"), u8::from(flag))),
                (
                    "values",
                    StructuredInfoValue::sequence(
                        ty.clone(),
                        values
                            .into_iter()
                            .map(|n| leaf(element.clone(), n))
                            .collect(),
                    )
                    .unwrap(),
                ),
            ],
        );
        let expected = p.evaluate(&input);
        let actual = prepared.evaluate(&input).map(<[u8]>::to_vec);
        match (&actual, &expected) {
            (Ok(actual), Ok(expected)) => assert_eq!(actual, expected),
            // These engines retain their own missing-projection refusal phase.
            (Err(_), Err(_)) => {}
            _ => panic!("prepared/reference acceptance differs: {actual:?} {expected:?}"),
        }
        main.push_str(&format!(
            "output.fill(0xa5);let result=e.evaluate_into(&{input:?},&mut output,&mut scratch);"
        ));
        match expected {
            Ok(bytes) => main.push_str(&format!(
                "assert_eq!(result,Ok({}));assert_eq!(&output[..{}],&{bytes:?});",
                bytes.len(),
                bytes.len()
            )),
            Err(_) => main.push_str("assert!(result.is_err());assert_eq!(output,[0xa5;4096]);"),
        }
    }
    main.push('}');
    for borrowed in [false, true] {
        let image = if borrowed {
            generate_borrowed_static_structured_expression_with_work(&d, &p, bounds(), 4096)
                .unwrap()
        } else {
            generate_static_structured_expression_with_work(&d, &p, bounds(), 4096).unwrap()
        };
        compiled_rust::verify(
            &format!("mod image{{{} }}", image.source),
            &main,
            if borrowed {
                "descriptor-reuse-lazy-borrowed"
            } else {
                "descriptor-reuse-lazy-owned"
            },
        );
    }
}
#[test]
fn a_reused_guarded_child_enforces_its_original_output_law_on_every_step() {
    let d=checked("type Input = {\n value: U8\n}\ntype Output = {\n left: U8\n right: U8\n}\nplot calculate (\n input: Input...| >> output: Output...|\n) = {left: .value + 1, right: .value + 1}\n");
    let mut p = program(&d, "calculate");
    let PortableExpressionOperation::Record(fields) = &mut p.root.operation else {
        panic!("record")
    };
    for (_, node) in fields {
        *node = PortableExpressionNode {
            value_type: node.value_type.clone(),
            operation: PortableExpressionOperation::ValueContract {
                input: None,
                output: Some(
                    CheckedValueContract::new(
                        kind_id("value/u8"),
                        1,
                        vec![ValueConstraint::FixedIntegerRange {
                            minimum: Some(vec![0]),
                            maximum: Some(vec![7]),
                            minimum_endpoint: IntervalEndpoint::Inclusive,
                            maximum_endpoint: IntervalEndpoint::Inclusive,
                        }],
                    )
                    .unwrap(),
                ),
                value: Box::new(node.clone()),
            },
        };
    }
    let allocator =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/common/allocation_probe.rs");
    let mut exercise=format!("#[path={allocator:?}]mod probe;#[global_allocator]static ALLOC:probe::Allocator=probe::Allocator;fn main(){{let(mut e,c)=probe::observe(||image::prepare().unwrap());assert_eq!(c.allocations,0);assert_eq!(c.reallocations,0);assert_eq!(image::CHILD_COUNT,1);let mut output=[0xa5;4096];let mut scratch=[0;4096];");
    let mut prepared = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    for n in [1, 6, 7, 2, 254, 0] {
        let input = record(
            &p.input_type,
            vec![("value", leaf(member(&p.input_type, "value"), n))],
        );
        let expected = p.evaluate(&input);
        assert_eq!(prepared.evaluate(&input).map(<[u8]>::to_vec), expected);
        exercise.push_str(&format!("output.fill(0xa5);let(result,c)=probe::observe(||e.evaluate_into(&{input:?},&mut output,&mut scratch));assert_eq!(c.allocations,0);assert_eq!(c.reallocations,0);"));
        match expected {
            Ok(bytes) => exercise.push_str(&format!(
                "assert_eq!(result,Ok({}));assert_eq!(&output[..{}],&{bytes:?});",
                bytes.len(),
                bytes.len()
            )),
            Err(_) => exercise.push_str("assert!(result.is_err());assert_eq!(output,[0xa5;4096]);"),
        }
    }
    exercise.push('}');
    for borrowed in [false, true] {
        let image = if borrowed {
            generate_borrowed_static_structured_expression_with_work(&d, &p, bounds(), 4096)
                .unwrap()
        } else {
            generate_static_structured_expression_with_work(&d, &p, bounds(), 4096).unwrap()
        };
        compiled_rust::verify(
            &format!("mod image{{{} }}", image.source),
            &exercise,
            if borrowed {
                "descriptor-reuse-guard-borrowed"
            } else {
                "descriptor-reuse-guard-owned"
            },
        );
    }
}
