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
    //127 conditions fit the128 child-value slots; direct input branches do
    //not create scalar child values or relax the checked Native boundaries.
    let emitted = generate_static_structured_expression(&checked, &program, bounds(), 4096)
        .expect("bounded whole constructor");
    assert!(!emitted.is_empty());
    generic_semantics(&program);
}
#[test]
fn an_oversized_whole_constructor_reports_its_exact_value_limit() {
    let (checked, program) = checked_program(8);
    // Depth-first lowering fills128 condition slots before visiting the129th.
    // Whole resident emission refuses; generic preparation's existing recursive
    // fallback remains valid and is not evidence that this tape can be emitted.
    assert_eq!(
        generate_static_structured_expression(&checked, &program, bounds(), 4096),
        Err(StaticStructuredGenerationRefusal::ConstructorLimit {
            resource: "constructor-values",
            required: 129,
            maximum: 128,
        }),
    );
    generic_semantics(&program);
}
