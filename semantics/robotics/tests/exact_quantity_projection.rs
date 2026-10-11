use conduit_core::{InfoDecodeError, Quantity, QuantityConversionRefusal as R};
use conduit_robotics::{BatteryObservation, RangeObservation};

fn quantity(source: &str) -> Quantity {
    let admitted = Quantity::parse_plot_literal(source).unwrap();
    Quantity::decode(&admitted.encode()).unwrap()
}

#[test]
fn prefixed_sensor_values_preserve_observation_wire_and_identity() {
    let expected = RangeObservation::new(1_500, 250).unwrap();
    for distance in ["1.5m", "0.0015km", "1500000µm", "1500000000nm"] {
        for age in ["0.25s", "250ms", "250000000ns"] {
            let actual =
                RangeObservation::from_quantities(quantity(distance), quantity(age)).unwrap();
            assert_eq!(actual, expected);
            assert_eq!(actual.encode(), expected.encode());
            assert_eq!(actual.semantic_digest(), expected.semantic_digest());
            assert_eq!(RangeObservation::decode(&actual.encode()), Ok(expected));
        }
    }
    let expected = BatteryObservation::new(750, 12_500).unwrap();
    for voltage in ["12.5V", "0.0125kV", "12500000µV", "12500000000nV"] {
        let actual =
            BatteryObservation::from_quantities(quantity("75%"), quantity(voltage)).unwrap();
        assert_eq!(actual.encode(), expected.encode());
        assert_eq!(actual.semantic_digest(), expected.semantic_digest());
        assert_eq!(BatteryObservation::decode(&actual.encode()), Ok(expected));
    }
}

#[test]
fn sensor_projection_refuses_unrepresentable_values_and_keeps_consumer_bounds() {
    for (source, refusal) in [
        ("1qm", R::Inexact),
        ("1Qm", R::Overflow),
        ("1kV", R::IncompatibleDimensions),
    ] {
        assert_eq!(
            RangeObservation::from_quantities(quantity(source), quantity("1ms")),
            Err(InfoDecodeError::QuantityConversion(refusal))
        );
    }
    assert!(matches!(
        RangeObservation::from_quantities(quantity("1.001km"), quantity("1ms")),
        Err(InfoDecodeError::OutOfRange {
            field: "distance-mm",
            ..
        })
    ));
    assert!(matches!(
        RangeObservation::from_quantities(quantity("1m"), quantity("61s")),
        Err(InfoDecodeError::OutOfRange {
            field: "age-ms",
            ..
        })
    ));
    assert_eq!(
        BatteryObservation::from_quantities(quantity("75%"), quantity("1µV")),
        Err(InfoDecodeError::QuantityConversion(R::Inexact))
    );
    assert!(matches!(
        BatteryObservation::from_quantities(quantity("75%"), quantity("0.061kV")),
        Err(InfoDecodeError::OutOfRange {
            field: "millivolts",
            ..
        })
    ));
}
