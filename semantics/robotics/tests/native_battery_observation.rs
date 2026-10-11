use conduit_core::{InfoDecodeError, Quantity, Unit};
use conduit_robotics::{
    BatteryObservation, MAXIMUM_BATTERY_MILLIVOLTS, ROBOTICS_BATTERY_ENCODED_LEN,
    ROBOTICS_BATTERY_INFO_ID,
};

#[test]
fn native_battery_value_preserves_bounds_projection_and_wire() {
    let value = BatteryObservation::new(750, 12_000).unwrap();
    assert_eq!(value.charge_permille(), 750);
    assert_eq!(value.millivolts(), 12_000);
    assert_eq!(value.charge(), Quantity::new(750, Unit::Permille));
    assert_eq!(value.voltage(), Quantity::new(12_000, Unit::Millivolt));
    assert_eq!(value.encode(), [0xee, 0x02, 0xe0, 0x2e]);
    assert_eq!(BatteryObservation::decode(&value.encode()), Ok(value));
    assert_eq!(
        value.semantic_digest(),
        [
            104, 217, 186, 122, 16, 250, 206, 176, 118, 225, 55, 133, 27, 51, 44, 64, 196, 190, 24,
            14, 139, 213, 173, 85, 173, 114, 45, 21, 95, 59, 28, 103,
        ]
    );
    assert_eq!(ROBOTICS_BATTERY_ENCODED_LEN, 4);
    assert_eq!(
        ROBOTICS_BATTERY_INFO_ID,
        "robotics/battery-permille-millivolts@1"
    );
}

#[test]
fn native_battery_value_refuses_each_invalid_boundary() {
    assert!(BatteryObservation::new(0, 0).is_ok());
    assert!(BatteryObservation::new(1_000, MAXIMUM_BATTERY_MILLIVOLTS).is_ok());
    assert!(BatteryObservation::new(1_001, 0).is_err());
    assert!(BatteryObservation::new(0, MAXIMUM_BATTERY_MILLIVOLTS + 1).is_err());
    assert_eq!(
        BatteryObservation::decode(&[0; 3]),
        Err(InfoDecodeError::WrongLength {
            expected: 4,
            actual: 3,
        })
    );
    assert!(BatteryObservation::decode(&[0xe9, 0x03, 0, 0]).is_err());
    assert!(BatteryObservation::decode(&[0, 0, 0x61, 0xea]).is_err());
}
