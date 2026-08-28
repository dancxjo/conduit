use conduit_core::{InfoDecodeError, Quantity, QuantityConversionRefusal, QuantityUnit};
use conduit_robotics::{
    BatteryObservation, OdometryObservation, OrientationObservation, RangeObservation,
    HALF_PI_MICRORADIANS, MAXIMUM_BATTERY_MILLIVOLTS, MAXIMUM_OBSERVATION_AGE_MS,
    MAXIMUM_ODOMETRY_MM, MAXIMUM_RANGE_MM, PI_MICRORADIANS, ROBOTICS_BATTERY_INFO_ID,
    ROBOTICS_ODOMETRY_INFO_ID, ROBOTICS_ORIENTATION_INFO_ID, ROBOTICS_RANGE_INFO_ID,
};

#[test]
fn robotics_info_codecs_are_exact_bounded_and_canonical() {
    let range = RangeObservation::new(MAXIMUM_RANGE_MM, MAXIMUM_OBSERVATION_AGE_MS).unwrap();
    assert_eq!(RangeObservation::decode(&range.encode()), Ok(range));
    assert_eq!(ROBOTICS_RANGE_INFO_ID, "robotics/range-mm-sensor-forward@1");

    let odometry =
        OdometryObservation::new(MAXIMUM_ODOMETRY_MM, -MAXIMUM_ODOMETRY_MM, PI_MICRORADIANS)
            .unwrap();
    assert_eq!(
        OdometryObservation::decode(&odometry.encode()),
        Ok(odometry)
    );
    assert_eq!(
        ROBOTICS_ODOMETRY_INFO_ID,
        "robotics/odometry-mm-start-local@1"
    );

    let battery = BatteryObservation::new(1_000, MAXIMUM_BATTERY_MILLIVOLTS).unwrap();
    assert_eq!(BatteryObservation::decode(&battery.encode()), Ok(battery));
    assert_eq!(
        ROBOTICS_BATTERY_INFO_ID,
        "robotics/battery-permille-millivolts@1"
    );

    let orientation =
        OrientationObservation::new(-PI_MICRORADIANS, HALF_PI_MICRORADIANS, PI_MICRORADIANS)
            .unwrap();
    assert_eq!(
        OrientationObservation::decode(&orientation.encode()),
        Ok(orientation)
    );
    assert_eq!(
        ROBOTICS_ORIENTATION_INFO_ID,
        "robotics/orientation-microrad-body@1"
    );

    assert_ne!(range.semantic_digest(), odometry.semantic_digest());
    assert_ne!(battery.semantic_digest(), orientation.semantic_digest());
}

#[test]
fn malformed_or_out_of_range_robotics_values_refuse_deterministically() {
    assert_eq!(
        RangeObservation::decode(&[0; 7]),
        Err(InfoDecodeError::WrongLength {
            expected: 8,
            actual: 7,
        })
    );
    assert!(matches!(
        RangeObservation::new(MAXIMUM_RANGE_MM + 1, 0),
        Err(InfoDecodeError::OutOfRange {
            field: "distance-mm",
            ..
        })
    ));
    assert!(matches!(
        BatteryObservation::new(1_001, 12_000),
        Err(InfoDecodeError::OutOfRange {
            field: "charge-permille",
            ..
        })
    ));
    assert!(matches!(
        OdometryObservation::new(0, 0, PI_MICRORADIANS + 1),
        Err(InfoDecodeError::OutOfRange {
            field: "yaw-microradians",
            ..
        })
    ));
    assert!(matches!(
        OrientationObservation::new(0, HALF_PI_MICRORADIANS + 1, 0),
        Err(InfoDecodeError::OutOfRange {
            field: "pitch-microradians",
            ..
        })
    ));
}

#[test]
fn robotics_consumes_typed_range_and_battery_without_changing_encoding() {
    let range = RangeObservation::from_quantities(
        Quantity::new(2, QuantityUnit::Meter),
        Quantity::new(1, QuantityUnit::Second),
    )
    .unwrap();
    assert_eq!(range, RangeObservation::new(2_000, 1_000).unwrap());
    assert_eq!(
        range.distance(),
        Quantity::new(2_000, QuantityUnit::Millimeter)
    );
    assert_eq!(range.age(), Quantity::new(1_000, QuantityUnit::Millisecond));

    let battery = BatteryObservation::from_quantities(
        Quantity::new(75, QuantityUnit::Percent),
        Quantity::new(12, QuantityUnit::Volt),
    )
    .unwrap();
    assert_eq!(battery, BatteryObservation::new(750, 12_000).unwrap());
    assert_eq!(battery.charge(), Quantity::new(750, QuantityUnit::Permille));
    assert_eq!(
        battery.voltage(),
        Quantity::new(12_000, QuantityUnit::Millivolt)
    );

    assert_eq!(
        RangeObservation::from_quantities(
            Quantity::new(1, QuantityUnit::Hertz),
            Quantity::new(1, QuantityUnit::Second),
        ),
        Err(InfoDecodeError::QuantityConversion(
            QuantityConversionRefusal::IncompatibleDimensions
        ))
    );
}
