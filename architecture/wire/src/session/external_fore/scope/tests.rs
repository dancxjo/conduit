use super::*;
fn bounds() -> ForeCarrierScopeBounds {
    ForeCarrierScopeBounds {
        maximum_lines: 2,
        maximum_frame_bytes: 512,
        maximum_payload_bytes: 460,
        maximum_buffered_bytes: 1024,
        maximum_in_flight_items: 1,
        maximum_frames: 100,
    }
}
fn scope(bounds: ForeCarrierScopeBounds) -> [u8; 32] {
    fore_carrier_scope_commitment(&[7; 32], "image/1", "config/1", bounds)
}
#[test]
fn fixed_independent_transcript_vectors() {
    // Computed independently with Python hashlib and struct.pack('<BIIQHI').
    let full = scope(bounds());
    assert_eq!(
        full,
        [
            201, 3, 99, 55, 214, 9, 89, 254, 111, 128, 57, 249, 128, 131, 112, 135, 20, 110, 41,
            145, 151, 244, 2, 187, 89, 39, 207, 214, 169, 171, 41, 146
        ]
    );
    assert_eq!(
        external_fore_scope_commitment(&full, &[8; 32], &[9; 198]),
        [
            148, 157, 119, 41, 254, 16, 95, 118, 201, 7, 236, 252, 119, 35, 161, 64, 108, 177, 66,
            241, 188, 71, 217, 223, 237, 190, 81, 118, 88, 65, 87, 110
        ]
    );
}
#[test]
fn every_installed_bound_and_identity_changes_scope() {
    let original = scope(bounds());
    let mut changes = [bounds(); 6];
    changes[0].maximum_lines += 1;
    changes[1].maximum_frame_bytes += 1;
    changes[2].maximum_payload_bytes += 1;
    changes[3].maximum_buffered_bytes += 1;
    changes[4].maximum_in_flight_items += 1;
    changes[5].maximum_frames += 1;
    for changed in changes {
        assert_ne!(original, scope(changed));
    }
    assert_ne!(
        original,
        fore_carrier_scope_commitment(&[6; 32], "image/1", "config/1", bounds())
    );
    assert_ne!(
        original,
        fore_carrier_scope_commitment(&[7; 32], "image/2", "config/1", bounds())
    );
    assert_ne!(
        original,
        fore_carrier_scope_commitment(&[7; 32], "image/1", "config/2", bounds())
    );
    assert_ne!(
        fore_carrier_scope_commitment(&[7; 32], "ab", "c", bounds()),
        fore_carrier_scope_commitment(&[7; 32], "a", "bc", bounds())
    );
    let mut wide = bounds();
    wide.maximum_buffered_bytes += 1u64 << 32;
    assert_ne!(original, scope(wide)); // Fixed-width backing, even on 32-bit MCU.
}
#[test]
fn exact_custody_full_scope_and_every_route_byte_are_pinned() {
    let full = scope(bounds());
    let row = [9; 198];
    let expected = external_fore_scope_commitment(&full, &[8; 32], &row);
    assert_ne!(
        expected,
        external_fore_scope_commitment(&[0; 32], &[8; 32], &row)
    );
    assert_ne!(
        expected,
        external_fore_scope_commitment(&full, &[0; 32], &row)
    );
    for index in 0..row.len() {
        let mut changed = row;
        changed[index] ^= 1;
        assert_ne!(
            expected,
            external_fore_scope_commitment(&full, &[8; 32], &changed),
            "row byte {index}"
        );
    }
}
