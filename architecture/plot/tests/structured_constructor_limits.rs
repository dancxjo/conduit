//! Real static emission has a finite whole-constructor budget. The generic
//! evaluator may retain its existing recursive fallback without enlarging it.
use conduit_core::{
    kind_id, ConfigurationValue, StructuredFieldValue, StructuredInfoType, StructuredInfoValue,
};
use conduit_plot::{rust_binding::PreparedNativeBounds, *};

fn branch(depth: u8) -> String {
    if depth == 0 {
        return ".".into();
    }
    let child = branch(depth - 1);
    format!("(.flag ? {child} : {child})")
}
fn checked_program(depth: u8) -> (CheckedSyntaxDocument, PortableExpressionProgram) {
    let source = format!(
        "type Frame = {{\n flag: Boolean\n value: U64\n}}\n\
         plot choose (\n input: Frame >> output: Frame\n) = ({})\n",
        branch(depth),
    );
    let syntax = parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, "choose", &ProfileCatalog::new()).unwrap();
    assert_eq!(expanded.expanded.gears.len(), 1);
    let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0]
        .configuration
        .iter()
        .find(|configuration| configuration.key == "program")
        .unwrap()
        .value
    else {
        panic!("checked expression program");
    };
    let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
    (checked, program)
}
fn bounds() -> PreparedNativeBounds {
    PreparedNativeBounds {
        canonical_bytes: 4096,
        scopes: 64,
        invariants: 64,
        contracts: 64,
    }
}
fn input(program: &PortableExpressionProgram, flag: bool, value: u64) -> Vec<u8> {
    StructuredInfoValue::record(
        program.input_type.clone(),
        vec![
            StructuredFieldValue::new(
                "flag",
                StructuredInfoValue::leaf(
                    StructuredInfoType::leaf(kind_id("value/bool")).unwrap(),
                    vec![u8::from(flag)],
                )
                .unwrap(),
            )
            .unwrap(),
            StructuredFieldValue::new(
                "value",
                StructuredInfoValue::leaf(
                    StructuredInfoType::leaf(kind_id("value/u64")).unwrap(),
                    value.to_le_bytes().to_vec(),
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
fn generic_semantics(program: &PortableExpressionProgram) {
    let mut prepared = PreparedPortableExpressionEvaluator::new(program).unwrap();
    for flag in [false, true] {
        for value in [0, 17, u64::MAX] {
            let input = input(program, flag, value);
            assert_eq!(program.evaluate(&input).unwrap(), input);
            assert_eq!(prepared.evaluate(&input).unwrap(), input);
        }
    }
}
#[test]
fn a_smaller_balanced_constructor_emits_with_the_original_limits() {
    let (checked, program) = checked_program(7);
    // All127 condition invocations share one exact descriptor; their original
    // execution points and independently checked Native boundaries remain.
    let emitted = generate_static_structured_expression(&checked, &program, bounds(), 4096)
        .expect("bounded whole constructor");
    assert!(!emitted.is_empty());
    generic_semantics(&program);
}
#[test]
fn an_oversized_whole_constructor_keeps_its_exact_operation_limit() {
    let (checked, program) = checked_program(8);
    // Descriptor reuse does not remove any branch/value operation. This larger
    // constructor still refuses at the unchanged instruction ceiling.
    assert_eq!(
        generate_static_structured_expression(&checked, &program, bounds(), 4096),
        Err(StaticStructuredGenerationRefusal::ConstructorLimit {
            resource: "constructor-operations",
            required: 513,
            maximum: 512,
        }),
    );
    generic_semantics(&program);
}

#[path = "common/compiled_rust.rs"]
mod compiled_rust;

#[test]
fn many_primitive_conditionals_use_the_existing_scalar_profile_in_the_emitted_image() {
    //129 scalar invocations share one exact descriptor; their calls remain.
    let values = (0..129)
        .map(|_| "(.flag ? .value + 1 : .value + 2)")
        .collect::<Vec<_>>()
        .join(", ");
    let source = format!(
        "type Frame = {{\n flag: Boolean\n value: U64\n}}\n\
         type Results = {{\n values: collection U64 = 129\n}}\n\
         plot many (\n input: Frame >> output: Results\n) = ({{values: [{values}]}})\n",
    );
    let syntax = parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, "many", &ProfileCatalog::new()).unwrap();
    assert_eq!(expanded.expanded.gears.len(), 1);
    let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!("program");
    };
    let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
    let image = generate_static_structured_expression(&checked, &program, bounds(), 4096)
        .expect("129 invocations share one descriptor under the original128-unique limit");
    assert_eq!(
        image,
        generate_static_structured_expression(&checked, &program, bounds(), 4096).unwrap(),
    );
    assert!(image.contains("pub const CHILD_COUNT: usize = 1;"));
    let allocator =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/common/allocation_probe.rs");
    let mut exercise = format!(
        "#[path={allocator:?}]mod probe;\n\
         #[global_allocator]static ALLOC:probe::Allocator=probe::Allocator;\n\
         fn main(){{\n\
         let(mut expression,cost)=probe::observe(||image::prepare().unwrap());\n\
         assert_eq!(cost.allocations,0);assert_eq!(cost.reallocations,0);\n\
         let mut output=[0xa5;4096];let mut scratch=[0;4096];\n",
    );
    let mut prepared = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    for (flag, value) in [
        (false, 0),
        (true, 0),
        (true, u64::MAX - 1), // The unselected +2 branch would overflow.
        (false, u64::MAX - 2),
        (false, u64::MAX - 1),
        (true, u64::MAX),
    ] {
        let input = input(&program, flag, value);
        let expected = program.evaluate(&input);
        assert_eq!(prepared.evaluate(&input).map(<[u8]>::to_vec), expected);
        exercise.push_str(&format!(
            "output.fill(0xa5);let(actual,cost)=probe::observe(||expression.evaluate_into(&{input:?},&mut output,&mut scratch));\n\
             assert_eq!(cost.allocations,0);assert_eq!(cost.reallocations,0);\n",
        ));
        match expected {
            Ok(bytes) => {
                let expected_value = value.checked_add(if flag { 1 } else { 2 }).unwrap();
                let decoded = StructuredInfoValue::from_canonical_bytes(&bytes).unwrap();
                let conduit_core::StructuredInfoValueShape::Record(fields) = decoded.shape() else {
                    panic!("results");
                };
                assert_eq!(fields.len(), 1);
                let conduit_core::StructuredInfoValueShape::Collection(items) =
                    fields[0].value().shape()
                else {
                    panic!("collection")
                };
                assert_eq!(items.len(), 129);
                for item in items {
                    assert_eq!(
                        item.shape(),
                        conduit_core::StructuredInfoValueShape::Leaf(&expected_value.to_le_bytes())
                    );
                }
                exercise.push_str(&format!(
                    "assert_eq!(actual,Ok({length}));assert_eq!(&output[..{length}],&{bytes:?});\n\
                     assert!(output[{length}..].iter().all(|byte|*byte==0xa5));\n",
                    length = bytes.len(),
                ));
            }
            Err(error) => {
                assert_eq!(error, PortableExpressionEvaluationRefusal::Arithmetic);
                exercise.push_str(
                    "assert_eq!(actual,Err(conduit_plot::StaticStructuredRefusal::Child{index:0,refusal:conduit_plot::StaticProjectedRefusal::Expression(conduit_plot::PortableExpressionEvaluationRefusal::Arithmetic)}));\n\
                     assert_eq!(output,[0xa5;4096]);\n",
                );
            }
        }
    }
    let mut malformed = input(&program, false, 0);
    malformed.pop();
    assert!(program.evaluate(&malformed).is_err());
    assert!(prepared.evaluate(&malformed).is_err());
    exercise.push_str(&format!(
        "output.fill(0xa5);assert!(matches!(expression.evaluate_into(&{malformed:?},&mut output,&mut scratch),Err(conduit_plot::StaticStructuredRefusal::Input(_))));assert_eq!(output,[0xa5;4096]);\n}}\n",
    ));
    compiled_rust::verify_with_libraries(
        &format!("mod image{{\n{image}\n}}"),
        &exercise,
        "bounded-primitive-constructor-children",
        &["conduit_plot", "conduit_core"],
    );
    let borrowed = generate_borrowed_static_structured_expression_with_work(
        &checked,
        &program,
        bounds(),
        4096,
    )
    .unwrap();
    compiled_rust::verify_with_libraries(
        &format!("mod image{{\n{}\n}}", borrowed.source),
        &exercise,
        "bounded-primitive-constructor-children-borrowed",
        &["conduit_plot", "conduit_core"],
    );
}
