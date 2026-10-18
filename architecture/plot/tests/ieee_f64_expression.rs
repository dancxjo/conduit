//! Raw bit identity is distinct from the checked finite arithmetic profile.
use conduit_core::*;
use conduit_plot::*;
#[path = "common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

#[test]
fn warm_prepared_binary64_operations_reuse_their_admitted_output_storage() {
    for (operator, expected) in [
        ("+", 0.75f64),
        ("-", 0.25),
        ("*", 0.125),
        ("/", 2.0),
        ("%", 0.0),
    ] {
        let p = binary(operator, "F64");
        let input = pair(&p, 0.5f64.to_bits(), 0.25f64.to_bits());
        let mut prepared = PreparedPortableExpressionEvaluator::new(&p).unwrap();
        let capacity = prepared.output_capacity();
        let ((), allocations) = allocation_probe::observe(|| {
            for _ in 0..64 {
                assert_eq!(prepared.evaluate(&input).unwrap(), expected.to_le_bytes());
            }
        });
        assert_eq!((allocations.allocations, allocations.reallocations), (0, 0));
        assert_eq!(prepared.output_capacity(), capacity);
    }
}

#[test]
fn binary64_precision_and_finite_ordering_do_not_become_binary32_or_bit_order() {
    let wider = program("plot proof (\n >> value: U16\n result: F64 >>\n) = ((0x4170000000000000 + 0x3ff0000000000000) - 0x4170000000000000)\n").unwrap();
    assert_eq!(
        wider.evaluate(&0u16.to_le_bytes()).unwrap(),
        1f64.to_le_bytes()
    );
    assert_eq!(
        PreparedPortableExpressionEvaluator::new(&wider)
            .unwrap()
            .evaluate(&0u16.to_le_bytes())
            .unwrap(),
        1f64.to_le_bytes()
    );
    for (operator, left, right, result) in [
        ("<", -1.0f64, 1.0f64, true),
        ("<=", -0.0, 0.0, true),
        (">", 2.0, 1.0, true),
        (">=", 1.0, 1.0, true),
    ] {
        let p = binary(operator, "Boolean");
        let input = pair(&p, left.to_bits(), right.to_bits());
        let expected = InfoBool::new(result).encode();
        let mut prepared = PreparedPortableExpressionEvaluator::new(&p).unwrap();
        assert_eq!(p.evaluate(&input).unwrap(), expected);
        assert_eq!(prepared.evaluate(&input).unwrap(), expected);
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let input = pair(&p, bad.to_bits(), right.to_bits());
            assert!(p.evaluate(&input).is_err());
            assert!(prepared.evaluate(&input).is_err());
        }
    }
}

fn program(source: &str) -> Result<PortableExpressionProgram, String> {
    let checked = check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new())
        .map_err(|why| format!("{why:?}"))?;
    let expanded = expand_canonical_plot_for_authoring(&checked, "proof", &ProfileCatalog::new())
        .map_err(|why| format!("{why:?}"))?;
    let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!()
    };
    Ok(PortableExpressionProgram::from_canonical_hex(encoded).unwrap())
}
fn binary(operator: &str, output: &str) -> PortableExpressionProgram {
    program(&format!("type Pair = {{\n left: F64\n right: F64\n}}\nplot proof (\n >> value: Pair\n result: {output} >>\n) = (.left {operator} .right)\n")).unwrap()
}
fn pair(program: &PortableExpressionProgram, left: u64, right: u64) -> Vec<u8> {
    let StructuredInfoTypeShape::Record { fields, .. } = program.input_type.shape() else {
        panic!()
    };
    StructuredInfoValue::record(
        program.input_type.clone(),
        fields
            .iter()
            .map(|field| {
                let bits = if field.name() == "left" { left } else { right };
                StructuredFieldValue::new(
                    field.name(),
                    StructuredInfoValue::leaf(
                        field.value_type().clone(),
                        bits.to_le_bytes().to_vec(),
                    )
                    .unwrap(),
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
fn raw_binary64_equality_preserves_nan_payloads_signed_zero_and_infinities() {
    let cases = [
        0,
        1 << 63,
        1,
        0x7ff0_0000_0000_0000,
        0xfff0_0000_0000_0000,
        0x7ff0_0000_0000_0001,
        0x7ff8_0000_0000_0001,
        0x7ff8_0000_0000_0002,
    ];
    for operator in ["==", "!="] {
        let p = binary(operator, "Boolean");
        let mut prepared = PreparedPortableExpressionEvaluator::new(&p).unwrap();
        for left in cases {
            for right in cases {
                let input = pair(&p, left, right);
                let expected = InfoBool::new((left == right) != (operator == "!=")).encode();
                assert_eq!(p.evaluate(&input).unwrap(), expected);
                assert_eq!(prepared.evaluate(&input).unwrap(), expected);
            }
        }
    }
}
#[test]
fn contextual_binary64_bits_are_exact_and_do_not_change_integer_literal_context() {
    for bits in [
        0u64,
        1 << 63,
        1,
        0x3fb9_9999_9999_999a,
        0x7ff0_0000_0000_0000,
        0x7ff8_0000_0000_0001,
    ] {
        for output in ["F64", "U64"] {
            let p = program(&format!(
                "plot proof (\n >> value: U16\n result: {output} >>\n) = (0x{bits:016x})\n"
            ))
            .unwrap();
            assert_eq!(p.evaluate(&0u16.to_le_bytes()).unwrap(), bits.to_le_bytes());
            assert_eq!(
                PreparedPortableExpressionEvaluator::new(&p)
                    .unwrap()
                    .evaluate(&0u16.to_le_bytes())
                    .unwrap(),
                bits.to_le_bytes()
            );
        }
    }
    for literal in ["0.1", "0x1234", "0x12345678901234567"] {
        assert!(program(&format!(
            "plot proof (\n >> value: U16\n result: F64 >>\n) = ({literal})\n"
        ))
        .is_err());
    }
}
#[test]
fn finite_binary64_operations_preserve_intermediate_rounding_and_refuse_invalid_domains() {
    for (operator, expected) in [
        ("+", 0.75f64),
        ("-", 0.25),
        ("*", 0.125),
        ("/", 2.0),
        ("%", 0.0),
    ] {
        let p = binary(operator, "F64");
        let input = pair(&p, 0.5f64.to_bits(), 0.25f64.to_bits());
        let mut prepared = PreparedPortableExpressionEvaluator::new(&p).unwrap();
        assert_eq!(p.evaluate(&input).unwrap(), expected.to_le_bytes());
        assert_eq!(prepared.evaluate(&input).unwrap(), expected.to_le_bytes());
        for (left, right) in [(f64::INFINITY, 1.0), (f64::NAN, 1.0), (f64::MAX, f64::MAX)] {
            if left.is_finite() && !matches!(operator, "+" | "*") {
                continue;
            }
            let bad = pair(&p, left.to_bits(), right.to_bits());
            assert!(p.evaluate(&bad).is_err());
            assert!(prepared.evaluate(&bad).is_err());
            assert_eq!(prepared.evaluate(&input).unwrap(), expected.to_le_bytes());
        }
        if matches!(operator, "/" | "%") {
            for zero in [0.0f64, -0.0] {
                assert!(prepared
                    .evaluate(&pair(&p, 1.0f64.to_bits(), zero.to_bits()))
                    .is_err());
            }
        }
    }
    let p = program("plot proof (\n >> value: U16\n result: F64 >>\n) = ((0x4340000000000000 + 0x3ff0000000000000) - 0x4340000000000000)\n").unwrap();
    assert_eq!(p.evaluate(&0u16.to_le_bytes()).unwrap(), 0f64.to_le_bytes());
    assert_eq!(
        PreparedPortableExpressionEvaluator::new(&p)
            .unwrap()
            .evaluate(&0u16.to_le_bytes())
            .unwrap(),
        0f64.to_le_bytes()
    );
}
