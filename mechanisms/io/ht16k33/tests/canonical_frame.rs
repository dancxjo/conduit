use conduit_ht16k33::{frame::*, Address};
#[path = "../build/reference.rs"]
mod reference;
#[test]
fn fixed_frame_matches_ordinary_composition_for_every_address_and_ram_byte() {
    let schema = conduit_core::StructuredInfoType::from_canonical_bytes(include_bytes!(concat!(
        env!("OUT_DIR"),
        "/ht16k33-frame.type"
    )))
    .unwrap();
    let mut encoder = PreparedDisplayFrame::prepare().unwrap();
    for address in 0x70..=0x77 {
        for byte in 0..=255 {
            let ram = core::array::from_fn(|n| byte ^ n as u8);
            let expected = reference::encode(&schema, address, ram);
            assert_eq!(
                encoder.encode(Address::new(address).unwrap(), ram).unwrap(),
                expected
            );
        }
    }
}
#[test]
fn malformed_and_wrong_type_frames_refuse_without_becoming_bus_output() {
    let mut encoder = PreparedDisplayFrame::prepare().unwrap();
    let frame = encoder
        .encode(Address::new(0x76).unwrap(), [0xff; 16])
        .unwrap();
    for end in 0..frame.len() {
        assert!(validate_display_frame(&frame[..end]).is_err());
    }
    let mut trailing = frame.to_vec();
    trailing.push(0);
    assert!(validate_display_frame(&trailing).is_err());
    let mut wrong = frame.to_vec();
    wrong[0] ^= 0xff;
    assert!(validate_display_frame(&wrong).is_err());
}

#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

#[test]
fn construction_and_encoding_keep_only_fixed_inline_bytes() {
    let ((), cost) = allocation_probe::observe(|| {
        let mut encoder = PreparedDisplayFrame::prepare().unwrap();
        assert_eq!(core::mem::size_of_val(&encoder), DISPLAY_FRAME_BYTES);
        for byte in 0..=255 {
            let value = encoder
                .encode(Address::new(0x70).unwrap(), [byte; 16])
                .unwrap();
            assert_eq!(value.len(), DISPLAY_FRAME_BYTES);
            assert!(validate_display_frame(value).is_ok());
        }
    });
    assert_eq!(cost.allocation_attempts, 0);
    assert_eq!(cost.reallocations, 0);
}

fn acknowledgement(count: u8) -> Vec<u8> {
    use conduit_core::{
        validate_canonical_structured_value, PreparedStructuredComposer, StructuredInfoType,
        StructuredInfoTypeShape,
    };
    let schema = StructuredInfoType::from_canonical_bytes(include_bytes!(concat!(
        env!("OUT_DIR"),
        "/ht16k33-acknowledgement.type"
    )))
    .unwrap();
    let StructuredInfoTypeShape::Record { fields, .. } = schema.shape() else {
        panic!("acknowledgement record")
    };
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].name(), "transactions");
    let mut scalar = PreparedStructuredComposer::new(fields[0].value_type(), 64).unwrap();
    scalar.leaf(&[count]).unwrap();
    let mut record = PreparedStructuredComposer::new(&schema, 2048).unwrap();
    record
        .record(&[validate_canonical_structured_value(scalar.encoded()).unwrap()])
        .unwrap();
    record.encoded().to_vec()
}

#[test]
fn only_one_selected_write_transaction_can_be_acknowledged() {
    for count in 0..=255 {
        assert_eq!(
            validate_write_acknowledgement(&acknowledgement(count)).is_ok(),
            count == 1
        );
    }
    let mut frame = PreparedDisplayFrame::prepare().unwrap();
    assert!(validate_write_acknowledgement(
        frame.encode(Address::new(0x70).unwrap(), [0; 16]).unwrap()
    )
    .is_err());
}

#[test]
fn write_acknowledgement_rejects_every_truncation_mutation_and_trailing_byte_without_allocation() {
    const ACK: &[u8] = include_bytes!(concat!(
        env!("OUT_DIR"),
        "/ht16k33-write-acknowledgement.value"
    ));
    assert_eq!(ACK, acknowledgement(1));
    let ((), cost) = allocation_probe::observe(|| {
        assert!(validate_write_acknowledgement(ACK).is_ok());
        for end in 0..ACK.len() {
            assert!(validate_write_acknowledgement(&ACK[..end]).is_err());
        }
        let mut changed = *include_bytes!(concat!(
            env!("OUT_DIR"),
            "/ht16k33-write-acknowledgement.value"
        ));
        for index in 0..changed.len() {
            changed[index] ^= 1;
            assert!(validate_write_acknowledgement(&changed).is_err());
            changed[index] ^= 1;
        }
        let mut trailing = [0; ACK.len() + 1];
        trailing[..ACK.len()].copy_from_slice(ACK);
        assert!(validate_write_acknowledgement(&trailing).is_err());
    });
    assert_eq!(cost.allocation_attempts, 0);
    assert_eq!(cost.reallocations, 0);
}
