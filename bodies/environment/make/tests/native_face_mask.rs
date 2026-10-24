//! Replay actual native MCU Body Faces through the ordinary timed Mask. This
//! verifies interpretation and correlation, not physical display delivery.
use conduit_face::Face;
use conduit_seven_segment_mask::{
    DigitWiring, MaskDisposition, PreparedEnvironmentFace, SegmentPatternForm, SevenSegmentLook,
    TimedSevenSegmentMask, Tm1637Profile, WireBit,
};

#[test]
fn physical_body_faces_feed_temperature_and_humidity_pages_without_replacement() {
    assert_pages([
        include_str!("../../evidence/2026-10-23-native-observations/physical-face-1.json"),
        include_str!("../../evidence/2026-10-23-native-observations/physical-face-2.json"),
        include_str!("../../evidence/2026-10-23-native-observations/physical-face-3.json"),
    ]);
}

#[test]
fn shared_projection_faces_feed_the_same_temperature_and_humidity_mask() {
    assert_pages([
        include_str!("../../evidence/2026-10-23-owner-shared-face/physical-face-1.json"),
        include_str!("../../evidence/2026-10-23-owner-shared-face/physical-face-2.json"),
        include_str!("../../evidence/2026-10-23-owner-shared-face/physical-face-3.json"),
    ]);
}
fn assert_pages(sources: [&str; 3]) {
    let profile = Tm1637Profile {
        digits: core::array::from_fn(|index| DigitWiring {
            address: index as u8,
            segment_bits: [0, 1, 2, 3, 4, 5, 6],
        }),
        decimal_points: [None; 4],
        colon: Some(WireBit { address: 1, bit: 7 }),
    };
    let mask = TimedSevenSegmentMask::prepare(
        SevenSegmentLook::fahrenheit().with_colon_fraction(),
        3000,
        6000,
    )
    .unwrap();
    for (index, source) in sources.into_iter().enumerate() {
        let parent: Face = serde_json::from_str(source).unwrap();
        let before = parent.clone();
        assert_eq!(parent.revision, index as u64 + 1);
        assert!(parent.basis.body_id.is_some() && parent.basis.wake_id.is_some());
        assert!(parent.subjects.len() > 4 && parent.properties.len() > 16);
        let selected = PreparedEnvironmentFace::prepare(&parent, "environment").unwrap();
        let temperature = mask.render(&selected, 0, &profile).unwrap();
        let humidity = mask.render(&selected, 3000, &profile).unwrap();
        assert_eq!(
            temperature.disposition,
            MaskDisposition::Temperature { decimal_places: 1 }
        );
        assert_eq!(humidity.disposition, MaskDisposition::Humidity);
        assert_eq!(
            SegmentPatternForm::encode(temperature.show.digits()[3]),
            [0x71]
        );
        assert_eq!(
            SegmentPatternForm::encode(humidity.show.digits()[3]),
            [0x76]
        );
        assert!(*temperature.show.colon());
        assert!(!*humidity.show.colon());
        for page in [temperature, humidity] {
            assert_eq!(page.face_content_id, parent.identity.as_str());
            assert_eq!(page.face_revision, parent.revision);
            assert_eq!(page.show.decimal_points(), &[false; 4]);
            profile.map(&page.show).unwrap();
        }
        assert_eq!(parent, before);
    }
}
