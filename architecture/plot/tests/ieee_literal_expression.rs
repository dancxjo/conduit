use conduit_core::ConfigurationValue;
use conduit_plot::*;
fn program(source: &str) -> Result<PortableExpressionProgram, CanonicalExpansionDiagnostic> {
    let checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    let expanded = expand_canonical_plot_for_authoring(&checked, "make", &ProfileCatalog::new())?;
    let ConfigurationValue::Text(bytes) = &expanded.expanded.gears[0].configuration[0].value else {
        panic!("expression")
    };
    Ok(PortableExpressionProgram::from_canonical_hex(bytes).unwrap())
}
#[test]
fn contextual_f32_bits_preserve_exact_payload_and_integer_context() {
    let p = program("plot make (\n >> value: U16\n result: F32 >>\n) = (0x3f59999a)\n").unwrap();
    let expected = 0x3f59999au32.to_le_bytes();
    assert_eq!(p.evaluate(&0u16.to_le_bytes()).unwrap(), expected);
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    assert_eq!(evaluator.evaluate(&0u16.to_le_bytes()).unwrap(), expected);
    let integer =
        program("plot make (\n >> value: U16\n result: U32 >>\n) = (0x3f59999a)\n").unwrap();
    assert_eq!(integer.evaluate(&0u16.to_le_bytes()).unwrap(), expected);
}
#[test]
fn nominal_finite_f32_rejects_nan_infinity_and_inexact_syntax() {
    for literal in ["0x7fc00000", "0x7f800000", "0x0000000", "0.85"] {
        let source=format!("type Finite = F32 finite\nplot make (\n >> value: U16\n result: Finite >>\n) = ({literal})\n");
        assert!(program(&source).is_err(), "{literal}");
    }
    let p=program("type Finite = F32 finite\ntype Vector = collection Finite = 1\nplot make (\n >> value: U16\n result: Vector >>\n) = ([0x3727c5ac])\n").unwrap();
    let expected = p.evaluate(&0u16.to_le_bytes()).unwrap();
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    assert_eq!(evaluator.evaluate(&0u16.to_le_bytes()).unwrap(), expected);
}
