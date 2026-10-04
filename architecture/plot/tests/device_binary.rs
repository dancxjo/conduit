use conduit_core::{
    kind_id, ConfigurationValue, StructuredFieldValue, StructuredInfoType, StructuredInfoValue,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, PreparedPortableExpressionEvaluator, ProfileCatalog, StartupCatalog,
};

const SOURCE: &str = include_str!("../../../plots/device-protocols/binary.conduit");

fn program(entry: &str) -> PortableExpressionProgram {
    let syntax = parse_syntax_document(SOURCE);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let expanded = expand_canonical_plot_for_authoring(&checked, entry, &ProfileCatalog::new())
        .unwrap()
        .expanded;
    assert_eq!(expanded.gears.len(), 1, "{entry}");
    let ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value else {
        panic!("checked pure expression");
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}

fn record(p: &PortableExpressionProgram, fields: &[(&str, u64)]) -> Vec<u8> {
    StructuredInfoValue::record(
        p.input_type.clone(),
        fields
            .iter()
            .map(|(name, value)| {
                StructuredFieldValue::new(
                    *name,
                    StructuredInfoValue::leaf(
                        StructuredInfoType::leaf(kind_id("value/u64")).unwrap(),
                        value.to_le_bytes().to_vec(),
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
fn every_binary_plot_checks_and_expands() {
    let syntax = parse_syntax_document(SOURCE);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    for plot in &checked.plots {
        expand_canonical_plot_for_authoring(&checked, &plot.name, &ProfileCatalog::new())
            .unwrap_or_else(|error| panic!("{}: {error:?}", plot.name));
    }
}

#[test]
fn endian_decoding_and_signed_extremes_keep_exact_values() {
    for bits in [8, 16, 24, 32] {
        let mask = (1_u64 << bits) - 1;
        for word in [0_u64, 1, 0x12345678, u64::MAX] {
            let p = program(&format!("binary-u{bits}-le"));
            assert_eq!(
                p.evaluate(&word.to_le_bytes()).unwrap(),
                (word & mask).to_le_bytes()
            );
        }
        for signed in [
            -(1_i128 << (bits - 1)),
            -1,
            0,
            1,
            (1_i128 << (bits - 1)) - 1,
        ] {
            let encode = program(&format!("binary-encode-i{bits}-le-word"));
            let word = encode.evaluate(&signed.to_le_bytes()).unwrap();
            let decode = program(&format!("binary-i{bits}-le"));
            assert_eq!(decode.evaluate(&word).unwrap(), signed.to_le_bytes());
        }
        if bits > 8 {
            let p = program(&format!("binary-u{bits}-be"));
            let encode = program(&format!("binary-encode-u{bits}-be-word"));
            for word in [0, 1, 0x12345678 & mask, mask] {
                let packed = encode.evaluate(&word.to_le_bytes()).unwrap();
                assert_eq!(p.evaluate(&packed).unwrap(), word.to_le_bytes());
            }
        }
    }
}

#[test]
fn generic_crc_steps_match_standard_check_vectors_without_growth() {
    let crc8 = program("binary-crc8-bit");
    let crc16 = program("binary-crc16-bit");
    let mut e8 = PreparedPortableExpressionEvaluator::new(&crc8).unwrap();
    let mut e16 = PreparedPortableExpressionEvaluator::new(&crc16).unwrap();
    let (cap8, cap16) = (e8.output_capacity(), e16.output_capacity());
    // CRC-8/SMBUS: poly 0x07, init 0, xorout 0, non-reflected.
    // CRC-16/MODBUS: reflected poly 0xa001, init 0xffff, xorout 0.
    for _ in 0..100 {
        let (mut c8, mut c16) = (0_u64, 65535_u64);
        for byte in b"123456789" {
            c8 ^= u64::from(*byte);
            c16 ^= u64::from(*byte);
            for _ in 0..8 {
                c8 = u64::from_le_bytes(
                    e8.evaluate(&record(&crc8, &[("accumulator", c8), ("polynomial", 7)]))
                        .unwrap()
                        .try_into()
                        .unwrap(),
                );
                c16 = u64::from_le_bytes(
                    e16.evaluate(&record(
                        &crc16,
                        &[("accumulator", c16), ("polynomial", 0xa001)],
                    ))
                    .unwrap()
                    .try_into()
                    .unwrap(),
                );
            }
        }
        assert_eq!(c8, 0xf4);
        assert_eq!(c16, 0x4b37);
        assert_eq!((e8.output_capacity(), e16.output_capacity()), (cap8, cap16));
    }
}

#[test]
fn bitfield_and_integer_overflow_refuse_instead_of_wrapping() {
    let p = program("binary-bitfield");
    assert_eq!(
        p.evaluate(&record(
            &p,
            &[("word", u64::MAX), ("shift", 63), ("mask", 1)]
        ))
        .unwrap(),
        1_u64.to_le_bytes()
    );
    assert!(p
        .evaluate(&record(&p, &[("word", 1), ("shift", 64), ("mask", 1)]))
        .is_err());
    let p = program("binary-encode-i8-le-overflow");
    assert_eq!(p.evaluate(&128_i128.to_le_bytes()).unwrap(), [1]);
    assert_eq!(p.evaluate(&(-129_i128).to_le_bytes()).unwrap(), [1]);
    assert_eq!(p.evaluate(&127_i128.to_le_bytes()).unwrap(), [0]);
}
