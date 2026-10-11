//! Checked branch facts narrow identical byte projections, never their contents.
use conduit_core::{
    ConfigurationValue, StructuredFieldValue, StructuredInfoType, StructuredInfoTypeShape,
    StructuredInfoValue, StructuredInfoValueShape,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    CanonicalExpansionDiagnostic, ExpandedAuthoringPlot, PortableExpressionProgram,
    PreparedPortableExpressionEvaluator, ProfileCatalog, StartupCatalog,
};
const TYPES: &str = "
type Wide = {
 wire: Bytes <= 2048B
 other: Bytes <= 2048B
 flag: Boolean
}
type Small = {
 wire: Bytes <= 1514B
}
type Result =
 ready Small
 | refused
";
fn expand(expression: &str) -> Result<ExpandedAuthoringPlot, CanonicalExpansionDiagnostic> {
    let source =
        format!("{TYPES}\nplot narrow (\n value: Wide >> result: Result\n) = ({expression})\n");
    let parsed = parse_syntax_document(&source);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let checked = check_syntax_document(&parsed, &StartupCatalog::new()).unwrap();
    expand_canonical_plot_for_authoring(&checked, "narrow", &ProfileCatalog::new())
}
fn program(expression: &str) -> PortableExpressionProgram {
    let expanded = expand(expression).unwrap();
    let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!("program")
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}
fn field<'a>(ty: &'a StructuredInfoType, name: &str) -> &'a StructuredInfoType {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("record")
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .unwrap()
        .value_type()
}
fn input(program: &PortableExpressionProgram, wire: &[u8], other: &[u8], flag: bool) -> Vec<u8> {
    StructuredInfoValue::record(
        program.input_type.clone(),
        vec![
            StructuredFieldValue::new(
                "wire",
                StructuredInfoValue::leaf(
                    field(&program.input_type, "wire").clone(),
                    wire.to_vec(),
                )
                .unwrap(),
            )
            .unwrap(),
            StructuredFieldValue::new(
                "other",
                StructuredInfoValue::leaf(
                    field(&program.input_type, "other").clone(),
                    other.to_vec(),
                )
                .unwrap(),
            )
            .unwrap(),
            StructuredFieldValue::new(
                "flag",
                StructuredInfoValue::leaf(
                    field(&program.input_type, "flag").clone(),
                    vec![u8::from(flag)],
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
#[test]
fn direct_polarities_and_conjunction_prove_only_unchanged_bounded_bytes() {
    for expression in [
        "bytes/length(.wire) <= 1514 ? ready({wire:.wire}) : refused(empty)",
        "bytes/length(.wire) > 1514 ? refused(empty) : ready({wire:.wire})",
        "!(bytes/length(.wire) > 1514) ? ready({wire:.wire}) : refused(empty)",
        "bytes/length(.wire) < 1515 && .flag ? ready({wire:.wire}) : refused(empty)",
        "bytes/length(.wire) >= 1515 || !.flag ? refused(empty) : ready({wire:.wire})",
    ] {
        let program = program(expression);
        let mut evaluator = PreparedPortableExpressionEvaluator::new(&program).unwrap();
        for extent in [0, 14, 1514, 1515, 2048] {
            let bytes: Vec<_> = (0..extent).map(|i| (i % 251) as u8).collect();
            let input = input(&program, &bytes, &[], true);
            let output = evaluator.evaluate(&input).unwrap();
            assert_eq!(output, program.evaluate(&input).unwrap());
            let value = StructuredInfoValue::from_canonical_bytes(output).unwrap();
            let StructuredInfoValueShape::Variant { tag, payload } = value.shape() else {
                panic!("result")
            };
            assert_eq!(tag, if extent <= 1514 { "ready" } else { "refused" });
            if extent <= 1514 {
                let StructuredInfoValueShape::Record(fields) = payload.shape() else {
                    panic!("payload")
                };
                let StructuredInfoValueShape::Leaf(observed) = fields[0].value().shape() else {
                    panic!("wire")
                };
                assert_eq!(observed, &bytes);
            }
        }
    }
}
#[test]
fn ambiguous_polarities_wrong_projection_and_join_do_not_establish_a_bound() {
    for expression in [
        "bytes/length(.wire) <= 1514 || .flag ? ready({wire:.wire}) : refused(empty)",
        "bytes/length(.wire) > 1514 && .flag ? refused(empty) : ready({wire:.wire})",
        "bytes/length(.other) <= 1514 ? ready({wire:.wire}) : refused(empty)",
        "bytes/length(.wire) <= 1514 ? ready({wire:.flag ? .wire : .other}) : refused(empty)",
        ".flag ? (bytes/length(.wire)<=1514 ? ready({wire:.wire}) : refused(empty)) : ready({wire:.wire})",
        "bytes/length(.wire) <= 18446744073709551615 ? ready({wire:.wire}) : refused(empty)",
        "bytes/length(.wire) > 18446744073709551615 ? ready({wire:.wire}) : refused(empty)",
        "bytes/length(.wire) < 0 ? ready({wire:.wire}) : refused(empty)",
    ] {
        assert_eq!(
            expand(expression).unwrap_err().code,
            "CND-FRM-046",
            "{expression}"
        );
    }
}
#[test]
fn branch_join_with_separate_bounds_keeps_projections_distinct() {
    let program = program(
        ".flag ? (bytes/length(.wire)<=1514 ? ready({wire:.wire}) : refused(empty)) : (bytes/length(.other)<=1514 ? ready({wire:.other}) : refused(empty))",
    );
    for flag in [false, true] {
        let bytes = program
            .evaluate(&input(&program, b"left", b"right", flag))
            .unwrap();
        let value = StructuredInfoValue::from_canonical_bytes(&bytes).unwrap();
        let StructuredInfoValueShape::Variant { payload, .. } = value.shape() else {
            panic!("variant")
        };
        let StructuredInfoValueShape::Record(fields) = payload.shape() else {
            panic!("record")
        };
        let StructuredInfoValueShape::Leaf(wire) = fields[0].value().shape() else {
            panic!("bytes")
        };
        assert_eq!(
            wire,
            if flag {
                b"left".as_slice()
            } else {
                b"right".as_slice()
            }
        );
    }
}
#[test]
fn overflowing_length_literal_cannot_supply_a_constructor_bound() {
    let source = format!(
        "{TYPES}\nplot narrow (\n value: Wide >> result: Result\n) = (bytes/length(.wire) <= 18446744073709551616 ? ready({{wire:.wire}}) : refused(empty))\n"
    );
    let parsed = parse_syntax_document(&source);
    let checked = check_syntax_document(&parsed, &StartupCatalog::new()).unwrap();
    let refusal = expand_canonical_plot_for_authoring(&checked, "narrow", &ProfileCatalog::new())
        .unwrap_err();
    assert_eq!(refusal.code, "CND-FRM-046");
}
