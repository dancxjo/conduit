use conduit_bluetooth::{BleGattProfile, BleProfileError, BluetoothHostStackBuffering};
use conduit_core::{LineReliability, LineSecurity};

#[test]
fn every_first_profile_buffer_is_finite_and_validated() {
    BleGattProfile::FIRST.validate().unwrap();

    let mut invalid = BleGattProfile::FIRST;
    invalid.negotiated_att_mtu = 3;
    assert_eq!(invalid.validate(), Err(BleProfileError::InvalidMtu));

    let mut invalid = BleGattProfile::FIRST;
    invalid.maximum_gatt_packet_bytes = 183;
    assert_eq!(invalid.validate(), Err(BleProfileError::InvalidFrameLimit));

    let mut invalid = BleGattProfile::FIRST;
    invalid.maximum_gatt_packet_bytes = 7;
    assert_eq!(
        conduit_bluetooth::fragment_count(1, invalid),
        Err(conduit_bluetooth::BleFramingError::OversizedFrame)
    );

    let mut invalid = BleGattProfile::FIRST;
    invalid.maximum_frame_bytes = 2_101;
    assert_eq!(invalid.validate(), Err(BleProfileError::InvalidFrameLimit));

    let mut invalid = BleGattProfile::FIRST;
    invalid.maximum_in_flight_items = 0;
    assert_eq!(invalid.validate(), Err(BleProfileError::InvalidItemLimit));

    let mut invalid = BleGattProfile::FIRST;
    invalid.maximum_payload_bytes = 2_049;
    assert_eq!(
        invalid.validate(),
        Err(BleProfileError::InvalidPayloadLimit)
    );

    assert_eq!(
        BleGattProfile::FIRST.host_stack_buffering,
        BluetoothHostStackBuffering::ExternallyManagedUnmeasured
    );
    assert_eq!(
        BleGattProfile::FIRST
            .link_limits()
            .unwrap()
            .maximum_buffered_bytes,
        BleGattProfile::FIRST.implementation_staging_bytes
    );
    assert_eq!(
        BleGattProfile::line_contract().reliability,
        LineReliability::BestEffort
    );
    assert_eq!(
        BleGattProfile::line_contract().security,
        LineSecurity::PlaintextNetwork
    );

    let mut invalid = BleGattProfile::FIRST;
    invalid.maximum_reconnect_attempts = 1;
    assert_eq!(
        invalid.validate(),
        Err(BleProfileError::AutomaticReconnectForbidden)
    );
}
