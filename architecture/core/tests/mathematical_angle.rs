use conduit_core::*;
fn work() -> ExactNumericWork {
    ExactNumericWork::new(100_000_000)
}
fn angle() -> MathematicalAngle {
    let mut b = MathematicalValueBuilder::new();
    let x = b
        .insert(
            MathematicalNode::Rational(ExactRational::ratio(3, 4, &mut work()).unwrap()),
            &mut work(),
        )
        .unwrap();
    let root = b
        .insert(MathematicalNode::ArctangentTurns(x), &mut work())
        .unwrap();
    MathematicalAngle::from_turn_coordinate(b.finish(root, &mut work()).unwrap())
}
fn bytes(value: &MathematicalAngle) -> Vec<u8> {
    let mut b = vec![0; MATHEMATICAL_ANGLE_MAX_ENCODED_BYTES];
    let n = value.encode(&mut b).unwrap();
    b.truncate(n);
    b
}
#[test]
fn exact_angle_round_trip_retains_unit_and_symbolic_coordinate() {
    let source = angle();
    let encoded = bytes(&source);
    let mut validation = ExactNumericWork::new(MATHEMATICAL_VALUE_VALIDATION_WORK);
    let view = MathematicalAngleView::decode(&encoded, &mut validation).unwrap();
    validate_primitive_info(MATHEMATICAL_ANGLE_ID, &encoded).unwrap();
    assert_eq!(view.unit().dimension(), QuantityDimension::Angle);
    assert_eq!(view.unit(), Unit::Turn);
    assert_eq!(view.role(), QuantityRole::Linear);
    assert_eq!(view.coordinate().as_rational(&mut work()).unwrap(), None);
    assert_eq!(
        MathematicalAngle::decode(&encoded, &mut work()).unwrap(),
        source
    );
    assert_eq!(
        view.semantic_digest(),
        semantic_digest(MATHEMATICAL_ANGLE_ID, &encoded)
    );
    // Coordinate bytes alone carry no Angle transport header.
    assert!(MathematicalAngleView::decode(view.coordinate().encoded(), &mut work()).is_err());
}
#[test]
fn foreign_units_versions_truncation_and_tiny_output_refuse() {
    let original = bytes(&angle());
    for unit in [Unit::Radian, Unit::resolve("m").unwrap()] {
        let mut forged = original.clone();
        forged[1..1 + UNIT_ENCODED_LEN].copy_from_slice(&unit.encode());
        assert!(matches!(
            validate_primitive_info(MATHEMATICAL_ANGLE_ID, &forged),
            Err(PrimitiveInfoRefusal::MathematicalAngle(_))
        ));
        assert_eq!(
            MathematicalAngleView::decode(&forged, &mut work()).unwrap_err(),
            MathematicalAngleRefusal::UnsupportedCoordinateUnit
        );
    }
    let mut version = original.clone();
    version[0] = 2;
    assert_eq!(
        MathematicalAngleView::decode(&version, &mut work()).unwrap_err(),
        MathematicalAngleRefusal::UnsupportedVersion
    );
    for end in [0, 1, UNIT_ENCODED_LEN, original.len() - 1] {
        assert!(MathematicalAngleView::decode(&original[..end], &mut work()).is_err());
    }
    assert!(angle().encode(&mut [0; 1]).is_err());
}
