//! Source command synthesis with complete recurring input/output storage prepared.
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_environment_runtime::display_packets::{DisplayPacketRefusal, PreparedDisplayPackets};
#[test]
fn source_packets_preserve_registers_and_refuse_invalid_brightness_without_allocating() {
    let mut encoder = PreparedDisplayPackets::prepare().unwrap();
    let ((), measured) = allocation_probe::observe(|| {
        let batch = encoder
            .encode([0x7d, 0xff, 0x66, 0x71, 0, 0], 3, true)
            .unwrap();
        assert_eq!(
            batch.packets(),
            [
                &[0x40][..],
                &[0xc0, 0x7d, 0xff, 0x66, 0x71, 0, 0][..],
                &[0x8b][..]
            ]
        );
        assert_eq!(
            encoder.encode([0; 6], 8, true),
            Err(DisplayPacketRefusal::InvalidBrightness)
        );
        let disabled = encoder.encode([0; 6], 7, false).unwrap();
        assert_eq!(disabled.packets()[2], &[0x87]);
        assert_eq!(
            encoder.encode([0; 6], 255, false),
            Err(DisplayPacketRefusal::InvalidBrightness)
        );
    });
    assert_eq!((measured.allocations, measured.reallocations), (0, 0));
}
#[test]
fn canonical_packets_match_the_driver_type_and_preserve_all_six_registers() {
    use conduit_core::{
        validate_canonical_structured_value as value, PreparedStructuredValueValidator,
    };
    let contract = conduit_tm1637_wire::WriteContract::prepare().unwrap();
    let validator = PreparedStructuredValueValidator::new(contract.packets_type(), 1024).unwrap();
    let mut encoder = PreparedDisplayPackets::prepare().unwrap();
    let registers = [0x80, 0x01, 0x42, 0xff, 0x7e, 0x5a];
    let ((), memory) = allocation_probe::observe(|| {
        for enabled in [false, true] {
            for brightness in 0..=7 {
                let canonical = encoder
                    .encode_canonical(registers, brightness, enabled)
                    .unwrap();
                validator.validate(canonical).unwrap();
                let packets = value(canonical)
                    .unwrap()
                    .record_field("packets")
                    .unwrap()
                    .unwrap();
                assert_eq!(packets.collection_length().unwrap(), 3);
                for (i, expected) in [
                    [0x40, 0, 0, 0, 0, 0, 0],
                    [0xc0, 0x80, 0x01, 0x42, 0xff, 0x7e, 0x5a],
                    [
                        (if enabled { 0x88 } else { 0x80 }) | brightness,
                        0,
                        0,
                        0,
                        0,
                        0,
                        0,
                    ],
                ]
                .iter()
                .enumerate()
                {
                    let packet = packets.collection_index(i as u16).unwrap().unwrap();
                    assert_eq!(
                        packet
                            .record_field("length")
                            .unwrap()
                            .unwrap()
                            .primitive_bytes("value/u8")
                            .unwrap(),
                        &[if i == 1 { 7 } else { 1 }]
                    );
                    let mut wire = [0; 7];
                    assert_eq!(
                        packet
                            .record_field("wire")
                            .unwrap()
                            .unwrap()
                            .copy_octet_collection("value/u8", &mut wire)
                            .unwrap(),
                        7
                    );
                    assert_eq!(&wire, expected);
                }
            }
        }
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
}
#[test]
fn frame_keeps_invalid_brightness_for_the_authored_refusal_fore() {
    use conduit_core::validate_canonical_structured_value as value;
    let mut encoder = PreparedDisplayPackets::prepare().unwrap();
    let frame = encoder.encode_frame([0; 6], 255, false).unwrap();
    assert_eq!(
        value(frame)
            .unwrap()
            .record_field("brightness")
            .unwrap()
            .unwrap()
            .primitive_bytes("value/u8")
            .unwrap(),
        &[255]
    );
    assert_eq!(
        encoder.encode_canonical([0; 6], 255, false),
        Err(DisplayPacketRefusal::InvalidBrightness)
    );
}
