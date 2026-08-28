use conduit_bluetooth::{
    encode_fragment, fragment_count, BleFramingError, BleGattProfile, BleReassembler,
    MAXIMUM_BLE_GATT_PACKET_BYTES,
};

#[test]
fn maximum_frame_round_trips_through_exact_finite_fragments() {
    let profile = BleGattProfile::FIRST;
    let mut frame = [0_u8; 2_048];
    for (index, byte) in frame.iter_mut().enumerate() {
        *byte = index as u8;
    }
    assert_eq!(fragment_count(frame.len(), profile), Ok(12));

    let mut reassembler = BleReassembler::new(profile);
    let mut packet = [0_u8; MAXIMUM_BLE_GATT_PACKET_BYTES];
    for index in 0..12 {
        let length = encode_fragment(&frame, 0, index, profile, &mut packet).unwrap();
        let result = reassembler.admit(&packet[..length]).unwrap();
        if index == 11 {
            assert_eq!(result, Some(frame.as_slice()));
        } else {
            assert_eq!(result, None);
        }
    }
}

#[test]
fn malformed_reordered_duplicate_and_oversized_fragments_fail_distinctly() {
    let profile = BleGattProfile::FIRST;
    let frame = [7_u8; 400];
    let mut packet = [0_u8; MAXIMUM_BLE_GATT_PACKET_BYTES];
    let first = encode_fragment(&frame, 0, 0, profile, &mut packet).unwrap();
    let first_packet = packet[..first].to_vec();
    let second = encode_fragment(&frame, 0, 1, profile, &mut packet).unwrap();

    let mut reordered = BleReassembler::new(profile);
    assert_eq!(
        reordered.admit(&packet[..second]),
        Err(BleFramingError::ReorderedFragment)
    );

    let mut duplicate = BleReassembler::new(profile);
    assert_eq!(duplicate.admit(&first_packet), Ok(None));
    assert_eq!(
        duplicate.admit(&first_packet),
        Err(BleFramingError::ReorderedFragment)
    );

    assert_eq!(
        fragment_count(2_049, profile),
        Err(BleFramingError::OversizedFrame)
    );
    assert_eq!(
        encode_fragment(&frame, 0, 0, profile, &mut [0_u8; 6]),
        Err(BleFramingError::OutputTooSmall)
    );
    assert_eq!(
        conduit_bluetooth::decode_fragment(&[0_u8; 7]),
        Err(BleFramingError::InvalidHeader)
    );
}
