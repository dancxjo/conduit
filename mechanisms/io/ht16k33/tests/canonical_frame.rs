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
