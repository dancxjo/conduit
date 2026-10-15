//! Arithmetic extraction seam only; filter admission is a separate contract.
use conduit_core::{
    ConfigurationValue, StructuredFieldValue, StructuredInfoTypeShape, StructuredInfoValue,
};
use conduit_plot::*;
const SOURCE: &str = include_str!("../../../semantics/audio/dsp/resonator-recurrence.conduit");
fn program(integer: &str) -> PortableExpressionProgram {
    let source = format!("{SOURCE}\ntype RecurrenceInput = {{\n drive: {integer}\n b: {integer}\n c: {integer}\n y1: {integer}\n y2: {integer}\n}}\nplot recurrence (\n >> value: RecurrenceInput\n result: {integer} >>\n) {{\n arithmetic: dsp/resonator-recurrence(item = RecurrenceInput, output = {integer})\n value >> arithmetic.value\n arithmetic.result >> result\n}}\n");
    let syntax = parse_syntax_document(&source);
    assert_eq!(syntax.round_trip(), source);
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let authored =
        expand_canonical_plot_for_authoring(&checked, "recurrence", &ProfileCatalog::new())
            .unwrap();
    assert_eq!(authored.expanded.gears.len(), 1);
    let ConfigurationValue::Text(encoded) = &authored.expanded.gears[0].configuration[0].value
    else {
        panic!("program")
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}
fn input(p: &PortableExpressionProgram, values: [i128; 5], wide: bool) -> Vec<u8> {
    let StructuredInfoTypeShape::Record { fields, .. } = p.input_type.shape() else {
        panic!("record")
    };
    StructuredInfoValue::record(
        p.input_type.clone(),
        fields
            .iter()
            .map(|f| {
                let index = ["drive", "b", "c", "y1", "y2"]
                    .iter()
                    .position(|n| *n == f.name())
                    .unwrap();
                let bytes = if wide {
                    values[index].to_le_bytes().to_vec()
                } else {
                    i32::try_from(values[index]).unwrap().to_le_bytes().to_vec()
                };
                StructuredFieldValue::new(
                    f.name(),
                    StructuredInfoValue::leaf(f.value_type().clone(), bytes).unwrap(),
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}
#[test]
fn compact_voice_ordering_and_wide_profile_share_one_checked_arithmetic_source() {
    for (integer, wide) in [("I32", false), ("I128", true)] {
        let p = program(integer);
        let mut evaluator = PreparedPortableExpressionEvaluator::new(&p).unwrap();
        let capacity = evaluator.output_capacity();
        for values in [
            [1000, 16300, -12000, 500, -200],
            [-1000, 16300, -12000, -500, 200],
            [32767, 0, -15146, 0, 0],
            [0, 22275, -15146, 1000, -1000],
        ] {
            let [drive, b, c, y1, y2] = values;
            let expected = (drive + b * y1 + c * y2) / 16384;
            let bytes = input(&p, values, wide);
            let result = evaluator.evaluate(&bytes).unwrap();
            if wide {
                assert_eq!(i128::from_le_bytes(result.try_into().unwrap()), expected)
            } else {
                assert_eq!(
                    i128::from(i32::from_le_bytes(result.try_into().unwrap())),
                    expected
                )
            }
            assert_eq!(result, p.evaluate(&bytes).unwrap());
            assert_eq!(evaluator.output_capacity(), capacity);
        }
    }
}
#[test]
fn legacy_overflow_refuses_instead_of_widening_speech_arithmetic() {
    let p = program("I32");
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    for values in [
        [0, i128::from(i32::MAX), 0, 2, 0],
        [0, 0, i128::from(i32::MIN), 0, -1],
    ] {
        let bytes = input(&p, values, false);
        assert!(p.evaluate(&bytes).is_err());
        assert!(evaluator.evaluate(&bytes).is_err());
    }
}
