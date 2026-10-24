//! Build-derived fixed encoding must equal the ordinary checked-Type composer.
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "../build/display_frame_reference.rs"]
mod reference;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_environment_runtime::display_packets::{PreparedDisplayFrame, DISPLAY_FRAME_BYTES};
#[test]
fn every_scalar_byte_matches_independent_composition_without_preparation_or_play_allocation() {
    let input = conduit_core::StructuredInfoType::from_canonical_bytes(include_bytes!(concat!(
        env!("OUT_DIR"),
        "/tm1637-frame.type"
    )))
    .unwrap();
    let (mut frame, preparation) =
        allocation_probe::observe(|| PreparedDisplayFrame::prepare().unwrap());
    assert_eq!((preparation.allocations, preparation.reallocations), (0, 0));
    assert_eq!(
        core::mem::size_of::<PreparedDisplayFrame>(),
        DISPLAY_FRAME_BYTES
    );
    for enabled in [false, true] {
        for byte in 0..=u8::MAX {
            let registers = core::array::from_fn(|n| byte.wrapping_add((n as u8).wrapping_mul(37)));
            let brightness = byte.wrapping_mul(7).wrapping_add(11);
            let mut values = [0; 8];
            values[..6].copy_from_slice(&registers);
            values[6] = brightness;
            values[7] = u8::from(enabled);
            let expected = reference::encode(&input, values);
            assert_eq!(expected.len(), DISPLAY_FRAME_BYTES);
            let ((), recurring) = allocation_probe::observe(|| {
                assert_eq!(
                    frame.encode(registers, brightness, enabled).unwrap(),
                    expected
                );
            });
            assert_eq!((recurring.allocations, recurring.reallocations), (0, 0));
        }
    }
}
