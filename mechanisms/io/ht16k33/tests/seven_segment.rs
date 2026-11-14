use conduit_ht16k33::{Profile, Refusal};

#[test]
fn every_logical_segment_maps_through_selected_physical_wiring() {
    // Deliberately non-default position, segment and decimal wiring.
    let profile = Profile::new(
        [7, 5, 3, 1],
        [13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0],
        15,
    )
    .unwrap();
    for bits in 0u8..128 {
        let ram = profile
            .encode_seven_segment([bits; 4], [false, true, false, true], false)
            .unwrap();
        let mut expected = 0u16;
        for bit in 0..6 {
            if bits & (1 << bit) != 0 {
                expected |= 1 << (13 - bit);
            }
        }
        if bits & 0x40 != 0 {
            expected |= (1 << 7) | (1 << 6);
        }
        for (position, word) in [7usize, 5, 3, 1].into_iter().enumerate() {
            let actual = u16::from_le_bytes([ram[word * 2], ram[word * 2 + 1]]);
            assert_eq!(
                actual,
                expected | if position % 2 == 1 { 1 << 15 } else { 0 }
            );
        }
        for word in [0usize, 2, 4, 6] {
            assert_eq!(&ram[word * 2..word * 2 + 2], &[0, 0]);
        }
    }
}

#[test]
fn degree_uses_both_middle_bars_and_unknown_annunciators_refuse() {
    let profile = Profile::adafruit_alpha4();
    let ram = profile
        .encode_seven_segment([0, 0, 0x63, 0], [false; 4], false)
        .unwrap();
    assert_eq!(&ram[4..6], &0x00e3u16.to_le_bytes());
    assert_eq!(
        profile.encode_seven_segment([0; 4], [false; 4], true),
        Err(Refusal::Glyph)
    );
    assert_eq!(
        profile.encode_seven_segment([0x80, 0, 0, 0], [false; 4], false),
        Err(Refusal::Glyph)
    );
}
