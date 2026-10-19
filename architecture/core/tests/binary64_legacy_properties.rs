//! Retain canonical property bytes for every legacy opcode and rounding mode.
use conduit_core::*;
use std::fmt::Write;
fn work() -> ExactNumericWork {
    ExactNumericWork::new(100_000_000)
}
#[test]
fn legacy_operation_properties_round_trip_in_all_rounding_modes() {
    let expected: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/binary64_legacy_properties.json")).unwrap();
    assert_eq!(expected.as_array().unwrap().len(), 4);
    let tenth = ExactRational::ratio(1, 10, &mut work()).unwrap();
    let p = [
        Binary64Operation::Constant(Box::new(tenth)),
        Binary64Operation::IeeeConstant(IeeeF64::from_bits(1f64.to_bits())),
        Binary64Operation::Add { left: 0, right: 1 },
        Binary64Operation::Subtract { left: 2, right: 1 },
        Binary64Operation::Multiply { left: 3, right: 1 },
        Binary64Operation::Divide { left: 4, right: 1 },
        Binary64Operation::Negate { input: 5 },
    ];
    for (index, rounding) in [
        Binary64Rounding::NearestTiesEven,
        Binary64Rounding::TowardZero,
        Binary64Rounding::TowardPositive,
        Binary64Rounding::TowardNegative,
    ]
    .into_iter()
    .enumerate()
    {
        let prepared = PreparedNumericalRealization::prepare_binary64(
            &MathematicalValue::rational(tenth.negated()),
            &p,
            rounding,
            ExactRational::ONE,
            Binary64AlgorithmLimits {
                maximum_steps: 32,
                maximum_retained_bytes: 1_000_000,
            },
            &mut work(),
        )
        .unwrap();
        let property = prepared.property().unwrap();
        assert_eq!(
            PreparedNumericalRealization::from_property(&property, &mut work()).unwrap(),
            prepared
        );
        let mut encoded = String::new();
        for byte in property.canonical_value() {
            write!(encoded, "{byte:02x}").unwrap();
        }
        assert_eq!(expected[index]["mode"], index);
        assert_eq!(expected[index]["hex"].as_str().unwrap(), encoded);
        println!("LEGACY_BINARY64 mode={index} hex={encoded}");
    }
}
