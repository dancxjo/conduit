#![cfg(feature = "display-mask")]
//! Preparation measurement over the real component, not an MCU admission bound.
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_environment_runtime::display_mask::PreparedDisplayMask;
use conduit_seven_segment_mask::*;
#[test]
fn measure_mask_preparation_and_reuse_its_frame_storage_for_both_pages() {
    let face: conduit_face::Face = serde_json::from_str(include_str!(
        "../../evidence/2026-10-24-native-face-execution/native-source-face-1.json"
    ))
    .unwrap();
    let (mut mask, preparation) = allocation_probe::observe(|| {
        PreparedDisplayMask::prepare(
            Tm1637Profile {
                digits: core::array::from_fn(|n| DigitWiring {
                    address: n as u8,
                    segment_bits: [0, 1, 2, 3, 4, 5, 6],
                }),
                decimal_points: [None; 4],
                colon: Some(WireBit { address: 1, bit: 7 }),
            },
            SevenSegmentLook::fahrenheit().with_colon_fraction(),
            3000,
            57600000,
            3,
        )
        .unwrap()
    });
    assert_eq!((preparation.allocations, preparation.reallocations), (0, 0));
    let ((), recurring) = allocation_probe::observe(|| {
        for (elapsed, disposition) in [
            (0, MaskDisposition::Temperature { decimal_places: 1 }),
            (3000, MaskDisposition::Humidity),
        ] {
            let frame = mask.render(&face, elapsed).unwrap();
            assert_eq!(frame.rendered.disposition, disposition);
            assert_eq!(frame.rendered.face_content_id, face.identity.as_str());
            conduit_environment_runtime::display_packets::validate_display_frame(frame.canonical)
                .unwrap();
        }
    });
    assert_eq!((recurring.allocations, recurring.reallocations), (0, 0));
    println!("cpu-mask-preparation pointer_bits={} owner_bytes={} peak_bytes={} requested_bytes={} allocations={}",usize::BITS,core::mem::size_of::<PreparedDisplayMask>(),preparation.peak_bytes,preparation.requested_bytes,preparation.allocations);
}
