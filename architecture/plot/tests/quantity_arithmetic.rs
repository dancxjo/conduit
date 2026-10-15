//! The instrument's authored law runs through the same checked expression as other Plots.
use conduit_core::{ConfigurationValue, Quantity, Scalar};
use conduit_plot::*;
#[path = "common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

fn program(source: &str, name: &str) -> PortableExpressionProgram {
    let syntax = parse_syntax_document(source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    assert_eq!(syntax.round_trip(), source);
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, name, &ProfileCatalog::new()).unwrap();
    let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!("program")
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}
fn expression(input: &str, output: &str, expression: &str) -> PortableExpressionProgram {
    program(
        &format!(
            "plot arithmetic (\n >> value: {input}\n result: {output} >>\n) = ({expression})\n"
        ),
        "arithmetic",
    )
}
fn quantity(text: &str) -> Vec<u8> {
    Quantity::parse_plot_literal(text)
        .unwrap()
        .encode()
        .to_vec()
}
fn parity(program: &PortableExpressionProgram, input: &[u8], expected: &[u8]) {
    assert_eq!(program.evaluate(input).unwrap(), expected);
    let mut prepared = PreparedPortableExpressionEvaluator::new(program).unwrap();
    assert_eq!(prepared.evaluate(input).unwrap(), expected);
    let (_, allocations) =
        allocation_probe::observe(|| assert_eq!(prepared.evaluate(input).unwrap(), expected));
    assert_eq!(allocations.allocations, 0);
    assert_eq!(allocations.reallocations, 0);
}
#[test]
fn theremin_pitch_is_named_clamped_unit_aware_and_allocation_free() {
    let source = include_str!("../../../plots/pocket-theremin/main.conduit")
        .split("# The one canonical Theremin.")
        .next()
        .unwrap();
    let program = program(source, "theremin-pitch");
    for (distance, hz) in [
        ("-1cm", "1760Hz"),
        ("0cm", "1760Hz"),
        ("1mm", "1754.5Hz"),
        ("15cm", "935Hz"),
        ("0.3m", "110Hz"),
        ("31cm", "110Hz"),
    ] {
        parity(&program, &quantity(distance), &quantity(hz));
    }
}
#[test]
fn quantity_scaling_and_ratios_are_exact_and_convert_units() {
    for (input, output, body, value, expected) in [
        (
            "Distance",
            "Scalar",
            "value / 1cm",
            "1mm",
            Scalar::from_raw_microunits(100_000).encode().to_vec(),
        ),
        (
            "Frequency",
            "Frequency",
            "value + 1kHz",
            "440Hz",
            quantity("1440Hz"),
        ),
        (
            "Frequency",
            "Frequency",
            "value * 0.5",
            "440Hz",
            quantity("220Hz"),
        ),
        (
            "Frequency",
            "Frequency",
            "0.5 * value",
            "440Hz",
            quantity("220Hz"),
        ),
        (
            "Frequency",
            "Frequency",
            "value / 2",
            "440Hz",
            quantity("220Hz"),
        ),
    ] {
        parity(
            &expression(input, output, body),
            &quantity(value),
            &expected,
        );
    }
}
#[test]
fn inexact_ratios_and_zero_division_refuse_in_both_evaluators() {
    for (output, body) in [("Scalar", "value / 3cm"), ("Scalar", "value / 0cm")] {
        let program = expression("Distance", output, body);
        let input = quantity("1cm");
        assert!(program.evaluate(&input).is_err());
        assert!(PreparedPortableExpressionEvaluator::new(&program)
            .unwrap()
            .evaluate(&input)
            .is_err());
    }
}
#[test]
fn incompatible_dimensions_and_affine_points_are_rejected_before_play() {
    for (input, output, body) in [
        ("Distance", "Distance", "value + 1Hz"),
        ("Distance", "Scalar", "value / 1Hz"),
        ("Temperature", "Temperature", "value * 2"),
        ("Distance", "Distance", "value * 1cm"),
    ] {
        let source =
            format!("plot invalid (\n >> value: {input}\n result: {output} >>\n) = ({body})\n");
        let checked =
            check_syntax_document(&parse_syntax_document(&source), &StartupCatalog::new()).unwrap();
        assert!(
            expand_canonical_plot_for_authoring(&checked, "invalid", &ProfileCatalog::new())
                .is_err(),
            "{body}"
        );
    }
}

#[test]
fn named_input_is_only_sugar_and_preserves_the_authored_evidence() {
    let named = expression("U8", "U8", "value + 1");
    let anonymous = expression("U8", "U8", ". + 1");
    assert_eq!(
        named.canonical_bytes().unwrap(),
        anonymous.canonical_bytes().unwrap()
    );
    parity(&named, &[41], &[42]);
    let source = "plot named (\n >> reading: U8\n result: U8 >>\n) = (reading + 1)\n";
    let checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    let sugar = &checked.source_sugar_expansions[0];
    assert_eq!(sugar.authored, "= (reading + 1)");
    assert_eq!(
        sugar.canonical_replacement.as_deref(),
        Some("{\n    reading >> (. + 1) >> result\n}")
    );
}
#[test]
fn controller_distance_is_an_ordinary_quantity_expression() {
    let program = expression("Scalar", "Distance", "value * 30cm");
    parity(
        &program,
        &Scalar::from_raw_microunits(500_000).encode(),
        &quantity("15cm"),
    );
}

#[test]
fn quantity_arithmetic_preserves_scientific_coordinates_without_narrowing_intermediates() {
    let program = expression("Frequency", "Frequency", "value + value");
    for exponent in [100, -100] {
        let input = Quantity::from_decimal(1, exponent, conduit_core::Unit::Hertz).unwrap();
        let expected = Quantity::from_decimal(2, exponent, conduit_core::Unit::Hertz).unwrap();
        parity(&program, &input.encode(), &expected.encode());
    }
}

fn projected_quantity_program(
    body: &str,
    reference_kind: &str,
) -> Result<PortableExpressionProgram, String> {
    use conduit_core::{kind_id, StructuredFieldType, StructuredInfoType};
    let input_type = StructuredInfoType::record(
        kind_id("test/DistancePair@1"),
        vec![
            StructuredFieldType::new(
                "distance",
                StructuredInfoType::leaf(kind_id(conduit_core::DISTANCE_INFO_ID)).unwrap(),
            )
            .unwrap(),
            StructuredFieldType::new(
                "reference",
                StructuredInfoType::leaf(kind_id(reference_kind)).unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let mut startup = StartupCatalog::new();
    startup
        .insert_structured_type("DistancePair", input_type)
        .unwrap();
    let source =
        format!("plot ratio (\n >> reading: DistancePair\n ratio: Scalar >>\n) = ({body})\n");
    let syntax = parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &startup).map_err(|error| format!("{error:?}"))?;
    let expanded = expand_canonical_plot_for_authoring(&checked, "ratio", &ProfileCatalog::new())
        .map_err(|error| format!("{error:?}"))?;
    let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!("program")
    };
    PortableExpressionProgram::from_canonical_hex(encoded).map_err(|error| format!("{error:?}"))
}

#[test]
fn projected_quantities_infer_ratios_and_nested_scaling_without_literal_hints() {
    use conduit_core::{StructuredFieldValue, StructuredInfoTypeShape, StructuredInfoValue};
    for (body, expected) in [
        ("reading.distance / reading.reference", 500_000),
        ("(reading.distance * 0.5) / reading.reference", 250_000),
        ("(0.5 * reading.distance) / reading.reference", 250_000),
    ] {
        let program = projected_quantity_program(body, conduit_core::DISTANCE_INFO_ID).unwrap();
        let StructuredInfoTypeShape::Record { fields, .. } = program.input_type.shape() else {
            panic!("record")
        };
        let input = StructuredInfoValue::record(
            program.input_type.clone(),
            vec![
                StructuredFieldValue::new(
                    "distance",
                    StructuredInfoValue::leaf(fields[0].value_type().clone(), quantity("15cm"))
                        .unwrap(),
                )
                .unwrap(),
                StructuredFieldValue::new(
                    "reference",
                    StructuredInfoValue::leaf(fields[1].value_type().clone(), quantity("30cm"))
                        .unwrap(),
                )
                .unwrap(),
            ],
        )
        .unwrap()
        .canonical_bytes()
        .unwrap();
        parity(
            &program,
            &input,
            &Scalar::from_raw_microunits(expected).encode(),
        );
    }
    assert!(projected_quantity_program(
        "reading.distance / reading.reference",
        conduit_core::FREQUENCY_INFO_ID
    )
    .is_err());
}
