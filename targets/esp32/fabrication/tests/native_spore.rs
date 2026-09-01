#[path = "../../firmware/spore.rs"]
mod firmware_spore;

#[test]
fn firmware_parser_recovers_the_bounded_body_provision() {
    let mut bytes = [0xff_u8; firmware_spore::SPORE_READ_BYTES];
    bytes[..16].copy_from_slice(b"CONDUIT_SPORE@1\0");
    bytes[16] = 1;
    bytes[19..27].copy_from_slice(&2_000_000_000_000_u64.to_le_bytes());
    bytes[27..59].fill(7);
    bytes[59..91].fill(9);
    let mut cursor = 91;
    for field in ["spore/one", "image/one", "invitation/one", "body/one"] {
        bytes[cursor] = field.len() as u8;
        cursor += 1;
        bytes[cursor..cursor + field.len()].copy_from_slice(field.as_bytes());
        cursor += field.len();
    }
    bytes[17..19].copy_from_slice(&(cursor as u16).to_le_bytes());

    let parsed = firmware_spore::parse(&bytes)
        .expect("the firmware parser must recover the native provision");
    assert_eq!(parsed.spore_id, "spore/one");
    assert_eq!(parsed.image_id, "image/one");
    assert_eq!(parsed.invitation_id, "invitation/one");
    assert_eq!(parsed.body_id, "body/one");
    assert_eq!(parsed.expires_at_millis, 2_000_000_000_000);
    assert_eq!(parsed.nonce, &[7; 32]);
    assert_eq!(parsed.secret, &[9; 32]);

    bytes[0] = b'X';
    assert!(firmware_spore::parse(&bytes).is_none());
}

#[test]
fn provisioning_region_is_one_reserved_minimum_flash_sector() {
    assert_eq!(
        u64::from(firmware_spore::SPORE_FLASH_ADDRESS),
        conduit_host_esp32_fabrication::NATIVE_SPORE_REGION_START,
    );
    assert_eq!(
        firmware_spore::SPORE_REGION_BYTES as u64,
        conduit_host_esp32_fabrication::NATIVE_SPORE_REGION_BYTES,
    );
}
