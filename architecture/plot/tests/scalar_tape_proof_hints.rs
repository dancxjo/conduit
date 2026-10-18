//! Optimization annotations cannot weaken the checked fixed-integer law.
use conduit_core::ConfigurationValue;
use conduit_plot::*;
fn program(expression: &str) -> PortableExpressionProgram {
    let source =
        format!("plot arithmetic (\n >> value: U64\n result: U64 >>\n) = ({expression})\n");
    let checked =
        check_syntax_document(&parse_syntax_document(&source), &StartupCatalog::new()).unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, "arithmetic", &ProfileCatalog::new())
            .unwrap();
    let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!("program")
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}
#[test]
fn an_unretained_proof_hint_cannot_turn_overflow_or_underflow_into_wrapping() {
    for (expression, invalid) in [(". + 1", u64::MAX), (". * 2", u64::MAX), (". - 1", 0)] {
        let checked = program(expression);
        let expected = checked.evaluate(&invalid.to_le_bytes()).unwrap_err();
        assert_eq!(expected, PortableExpressionEvaluationRefusal::Arithmetic);
        let mut annotated = checked.clone();
        let PortableExpressionOperation::Binary { proven, .. } = &mut annotated.root.operation
        else {
            panic!("binary")
        };
        *proven = true;
        let mut tape = PreparedPortableExpressionEvaluator::new_scalar_tape(&annotated).unwrap();
        assert_eq!(
            tape.evaluate(&7u64.to_le_bytes()).unwrap(),
            checked.evaluate(&7u64.to_le_bytes()).unwrap()
        );
        assert_eq!(tape.evaluate(&invalid.to_le_bytes()).unwrap_err(), expected);
    }
}
