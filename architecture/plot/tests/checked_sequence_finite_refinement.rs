//! Exact all-element finite metadata survives copying; arithmetic proves no law.
use conduit_core::{
    ConfigurationValue, StructuredFieldValue, StructuredInfoType, StructuredInfoTypeShape as Shape,
    StructuredInfoValue as Value,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, PreparedPortableExpressionEvaluator, ProfileCatalog, StartupCatalog,
};
const TYPES: &str = "
type Finite = F32 finite
type Vector<N: U16> = collection Finite = N
type Four = Vector<4>
type Two = Vector<2>
type RawFour = collection F32 = 4
type Positive = F32 finite in 0.0..
type PositiveTwo = collection Positive = 2
type Input = {
 source: Four
 raw: RawFour
 index: U64
}
";
fn expand(expression: &str, output: &str) -> Result<PortableExpressionProgram, String> {
    let source =
        format!("{TYPES}\nplot select (\n input: Input >> output: {output}\n) = ({expression})\n");
    let syntax = parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked =
        check_syntax_document(&syntax, &StartupCatalog::new()).map_err(|e| format!("{e:?}"))?;
    let expanded = expand_canonical_plot_for_authoring(&checked, "select", &ProfileCatalog::new())
        .map_err(|e| format!("{e:?}"))?;
    let ConfigurationValue::Text(hex) = &expanded.expanded.gears[0].configuration[0].value else {
        panic!("pure program")
    };
    Ok(PortableExpressionProgram::from_canonical_hex(hex).unwrap())
}
fn floats(ty: &StructuredInfoType, values: &mut impl Iterator<Item = f32>) -> Value {
    match ty.shape() {
        Shape::Nominal { representation, .. } => {
            Value::nominal(ty.clone(), floats(representation, values)).unwrap()
        }
        Shape::Collection { element, length } => Value::collection(
            ty.clone(),
            (0..length).map(|_| floats(element, values)).collect(),
        )
        .unwrap(),
        Shape::Leaf(kind) if kind.as_str() == conduit_core::F32_INFO_ID => {
            Value::leaf(ty.clone(), values.next().unwrap().to_le_bytes().to_vec()).unwrap()
        }
        _ => panic!("float tree"),
    }
}
fn input(program: &PortableExpressionProgram, values: [f32; 4], index: u64) -> Vec<u8> {
    let Shape::Record { fields, .. } = program.input_type.shape() else {
        panic!("input record")
    };
    Value::record(
        program.input_type.clone(),
        fields
            .iter()
            .map(|f| {
                let value = if f.name() == "index" {
                    Value::leaf(f.value_type().clone(), index.to_le_bytes().to_vec()).unwrap()
                } else {
                    floats(f.value_type(), &mut values.into_iter())
                };
                StructuredFieldValue::new(f.name(), value).unwrap()
            })
            .collect(),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}
#[test]
fn finite_copy_preserves_nominal_elements_at_constant_and_dynamic_indices() {
    for expression in [
        "[sequence/at(.source,3),sequence/at(.source,0)]",
        "[sequence/at(.source,.index),sequence/at(.source,0)]",
    ] {
        let program = expand(expression, "Two").unwrap();
        let values = [-0.0, f32::from_bits(1), f32::MAX, -1.25];
        let encoded = input(&program, values, 3);
        let expected = floats(&program.output_type, &mut [-1.25, -0.0].into_iter())
            .canonical_bytes()
            .unwrap();
        let mut prepared = PreparedPortableExpressionEvaluator::new(&program).unwrap();
        assert_eq!(program.evaluate(&encoded).unwrap(), expected);
        assert_eq!(prepared.evaluate(&encoded).unwrap(), expected);
    }
}
#[test]
fn successful_copy_does_not_fabricate_an_out_of_bounds_element() {
    let program = expand(
        "[sequence/at(.source,.index),sequence/at(.source,0)]",
        "Two",
    )
    .unwrap();
    let mut prepared = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    for index in [4, u64::MAX] {
        let encoded = input(&program, [1., 2., 3., 4.], index);
        assert!(program.evaluate(&encoded).is_err());
        assert!(prepared.evaluate(&encoded).is_err());
    }
    assert!(prepared
        .evaluate(&input(&program, [1., 2., 3., 4.], 2))
        .is_ok());
}
#[test]
fn raw_arithmetic_and_narrower_contract_cannot_borrow_finite_copy_evidence() {
    for (expression, output) in [
        ("[sequence/at(.raw,0),sequence/at(.raw,1)]", "Two"),
        (
            "[sequence/at(.source,0)*sequence/at(.source,1),sequence/at(.source,2)]",
            "Two",
        ),
        (
            "[sequence/at(.source,0),sequence/at(.source,1)]",
            "PositiveTwo",
        ),
        (
            "sequence/at(.raw,0)>=0x00000000 ? [sequence/at(.raw,1),sequence/at(.raw,2)] : [sequence/at(.source,0),sequence/at(.source,1)]",
            "Two",
        ),
    ] {
        assert!(expand(expression, output).is_err(), "{expression}");
    }
}
