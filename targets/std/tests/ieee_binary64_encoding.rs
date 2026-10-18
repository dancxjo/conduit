//! Canonical typed output and warm storage proof for the private IEEE Back.
#[path = "../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "../src/numerical_host/ieee_encoding.rs"]
mod encoding;
use conduit_core::*;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

fn flags(bits: u8) -> IeeeBinary64ExceptionFlags {
    IeeeBinary64ExceptionFlags {
        inexact: bits & 1 != 0,
        underflow: bits & 2 != 0,
        overflow: bits & 4 != 0,
        divide_by_zero: bits & 8 != 0,
        invalid: bits & 16 != 0,
    }
}
fn inspect(bytes: &[u8], expected_type: &[u8], bits: u8, quiet: Option<bool>, value: u64) {
    let result = validate_canonical_structured_value(bytes).unwrap();
    assert_eq!(result.type_bytes(), expected_type);
    let f = result.record_field("flags").unwrap().unwrap();
    for (name, bit) in [
        ("inexact", 1),
        ("underflow", 2),
        ("overflow", 4),
        ("divide-by-zero", 8),
        ("invalid", 16),
    ] {
        assert_eq!(
            f.record_field(name)
                .unwrap()
                .unwrap()
                .primitive_bytes(BOOL_INFO_ID)
                .unwrap(),
            InfoBool::new(bits & bit != 0).encode()
        );
    }
    let v = result.record_field("value").unwrap().unwrap();
    if let Some(expected) = quiet {
        assert_eq!(
            v.primitive_bytes(BOOL_INFO_ID).unwrap(),
            InfoBool::new(expected).encode()
        );
    } else {
        assert_eq!(v.primitive_bytes(F64_INFO_ID).unwrap(), value.to_le_bytes());
    }
}
#[test]
fn every_flag_combination_and_ieee_carrier_class_encodes_without_growth() {
    let mut arithmetic = encoding::PreparedIeeeEncoding::new(false).unwrap();
    let mut comparison = encoding::PreparedIeeeEncoding::new(true).unwrap();
    assert!(!arithmetic.result_bytes().is_empty());
    assert!(!comparison.result_bytes().is_empty());
    let capacity = [
        arithmetic.prepared_heap_bytes(),
        comparison.prepared_heap_bytes(),
    ];
    let values = [
        0,
        1u64 << 63,
        1,
        0x0010_0000_0000_0000,
        0x3ff0_0000_0000_0000,
        0x7ff0_0000_0000_0000,
        0xfff0_0000_0000_0000,
        0x7ff8_0000_0000_0123,
        0xfff0_0000_0000_0042,
    ];
    let arithmetic_type = conduit_semantic_catalog::ieee_binary64_result_type()
        .canonical_bytes()
        .unwrap();
    let comparison_type = conduit_semantic_catalog::ieee_binary64_comparison_type()
        .canonical_bytes()
        .unwrap();
    let ((), allocations) = allocation_probe::observe(|| {
        for bits in 0..32 {
            for value in values {
                inspect(
                    arithmetic
                        .arithmetic(IeeeBinary64Result {
                            value: IeeeF64::from_bits(value),
                            flags: flags(bits),
                        })
                        .unwrap(),
                    &arithmetic_type,
                    bits,
                    None,
                    value,
                );
            }
            for value in [false, true] {
                inspect(
                    comparison.comparison(value, flags(bits)).unwrap(),
                    &comparison_type,
                    bits,
                    Some(value),
                    0,
                );
            }
        }
        assert_eq!(
            arithmetic.comparison(false, flags(0)),
            Err(StructuredInfoRefusal::WrongType)
        );
        assert_eq!(
            comparison.arithmetic(IeeeBinary64Result {
                value: IeeeF64::from_bits(0),
                flags: flags(0)
            }),
            Err(StructuredInfoRefusal::WrongType)
        );
    });
    assert_eq!((allocations.allocations, allocations.reallocations), (0, 0));
    assert_eq!(
        capacity,
        [
            arithmetic.prepared_heap_bytes(),
            comparison.prepared_heap_bytes()
        ]
    );
}
