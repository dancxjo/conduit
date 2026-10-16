use conduit_core::*;
fn work() -> ExactNumericWork {
    ExactNumericWork::new(100_000_000)
}
fn prepared() -> PreparedNumericalRealization {
    PreparedNumericalRealization::prepare(
        &MathematicalValue::parse_literal("tau", &mut work()).unwrap(),
        NumericalAlgorithm::EnclosedBinary64 {
            rounding: Binary64Rounding::NearestTiesEven,
        },
        ExactRational::ratio(1, 1_000_000_000_000, &mut work()).unwrap(),
        &mut work(),
    )
    .unwrap()
}
#[test]
fn full_symbolic_certificate_recomputes_and_roundtrips_without_rational_pi() {
    let value = prepared();
    assert_eq!(value.source().as_rational(), None);
    let property = value.property().unwrap();
    let decoded = PreparedNumericalRealization::from_property(&property, &mut work()).unwrap();
    assert_eq!(decoded, value);
    assert_eq!(decoded.property().unwrap(), property);
    let NumericalTarget::EnclosedBinary64(target) = value.target() else {
        panic!("enclosed target");
    };
    assert_eq!(target.source(), value.source());
    assert_eq!(target.maximum_error(), value.maximum_error());
    assert!(target.rounded().steps().len() == 1);
}
#[test]
fn forged_machine_target_or_work_budget_cannot_survive_certificate_verification() {
    let property = prepared().property().unwrap();
    let value = StructuredInfoValue::from_canonical_bytes(property.canonical_value()).unwrap();
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        panic!("certificate leaf");
    };
    for target in [true, false] {
        let mut bytes = bytes.to_vec();
        if target {
            let last = bytes.len() - 1;
            bytes[last] ^= 1;
        } else {
            bytes[3..11].fill(0);
        }
        let forged = StructuredInfoValue::leaf(value.value_type().clone(), bytes).unwrap();
        let property = StructuredConfigurationValue::new(
            numerical_realization_profile(),
            forged.canonical_bytes().unwrap(),
        )
        .unwrap();
        assert!(PreparedNumericalRealization::from_property(&property, &mut work()).is_err());
    }
}
#[test]
fn zero_tolerance_refuses_symbolic_source_before_play() {
    let source = MathematicalValue::parse_literal("pi", &mut work()).unwrap();
    assert!(PreparedNumericalRealization::prepare(
        &source,
        NumericalAlgorithm::EnclosedBinary64 {
            rounding: Binary64Rounding::NearestTiesEven
        },
        ExactRational::ZERO,
        &mut work()
    )
    .is_err());
    assert!(PreparedNumericalRealization::from_property(
        &prepared().property().unwrap(),
        &mut ExactNumericWork::new(0)
    )
    .is_err());
}
