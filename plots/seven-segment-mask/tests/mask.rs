use conduit_core::{
    kind_id, CheckedValueContract, Quantity, QUANTITY_ENCODED_LEN, RATIO_INFO_ID,
    TEMPERATURE_INFO_ID,
};
use conduit_face::{Face, FaceBasis, FaceProperty, FacePropertyValue, FaceRole, FaceSubject};
use conduit_seven_segment_mask::*;

#[path = "../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

fn property(name: &str, value: FacePropertyValue) -> FaceProperty {
    FaceProperty {
        subject: "environment".into(),
        name: name.into(),
        value,
    }
}
fn quantity(name: &str, kind: &str, literal: &str) -> FaceProperty {
    property(
        name,
        FacePropertyValue::TypedValue {
            contract: CheckedValueContract::new(kind_id(kind), QUANTITY_ENCODED_LEN as u32, vec![])
                .unwrap(),
            bytes: Quantity::parse_plot_literal(literal)
                .unwrap()
                .encode()
                .to_vec(),
        },
    )
}
fn face(temperature: &str, humidity: &str, fresh: bool, available: bool) -> Face {
    Face::new(
        7,
        FaceBasis {
            body_id: None,
            wake_id: None,
            source_document_id: None,
            checked_plot_id: None,
            expanded_plot_id: None,
            plan_id: None,
            active_play_id: None,
            sign_ids: vec![],
        },
        vec![FaceSubject {
            identity: "environment".into(),
            role: FaceRole::Info,
            name: "Room conditions".into(),
        }],
        vec![],
        vec![
            quantity("temperature", TEMPERATURE_INFO_ID, temperature),
            quantity("relative-humidity", RATIO_INFO_ID, humidity),
            property("fresh", FacePropertyValue::Flag(fresh)),
            property(
                "sensor-status",
                FacePropertyValue::Text(
                    if available {
                        "available"
                    } else {
                        "unavailable"
                    }
                    .into(),
                ),
            ),
            property("minute-of-day", FacePropertyValue::Count(21 * 60 + 37)),
            property("clock-fresh", FacePropertyValue::Flag(true)),
        ],
        vec![],
    )
    .unwrap()
}
fn text(show: &SevenSegmentShow) -> String {
    let mut text = String::new();
    let numerals = [0x3f, 0x06, 0x5b, 0x4f, 0x66, 0x6d, 0x7d, 0x07, 0x7f, 0x6f];
    for (i, digit) in show.digits().iter().enumerate() {
        let bits = SegmentPatternForm::encode(*digit)[0];
        let glyph = match bits {
            0 => ' ',
            0x39 => 'C',
            0x71 => 'F',
            0x76 => 'H',
            0x40 => '-',
            _ => char::from(
                b'0' + numerals
                    .iter()
                    .position(|n| *n == bits)
                    .expect("known digit") as u8,
            ),
        };
        text.push(glyph);
        if show.decimal_points()[i] {
            text.push('.');
        }
        if i == 1 && *show.colon() {
            text.push(':');
        }
    }
    text
}

#[test]
fn fahrenheit_look_changes_expression_without_changing_face_quantity() {
    let face = face("19.8°C", "59%", true, true);
    let before = face.clone();
    let view = PreparedEnvironmentFace::prepare(&face, "environment").unwrap();
    let output = render(
        &view,
        SevenSegmentSelection::Temperature,
        &SevenSegmentLook::fahrenheit(),
    );
    assert_eq!(text(&output.show), "67.6F");
    assert_eq!(output.face_content_id, face.identity.as_str());
    assert_eq!(output.face_revision, 7);
    assert_eq!(
        text(
            &render(
                &view,
                SevenSegmentSelection::Temperature,
                &SevenSegmentLook::celsius()
            )
            .show
        ),
        "19.8C"
    );
    assert_eq!(
        text(
            &render(
                &view,
                SevenSegmentSelection::Humidity,
                &SevenSegmentLook::fahrenheit()
            )
            .show
        ),
        " 59H"
    );
    assert_eq!(
        text(
            &render(
                &view,
                SevenSegmentSelection::Clock,
                &SevenSegmentLook::fahrenheit()
            )
            .show
        ),
        "21:37"
    );
    assert_eq!(face, before);
}

#[test]
fn rounding_sign_precision_fitting_and_overflow_are_explicit() {
    for (temperature, expected, places) in [
        ("19.85°F", "19.9F", 1),
        ("-9.94°F", "-9.9F", 1),
        ("-9.95°F", "-10F", 0),
        ("-10.8°F", "-11F", 0),
        ("-0.04°F", " 0.0F", 1),
        ("99.96°F", "100F", 0),
        ("999.4°F", "999F", 0),
    ] {
        let face = face(temperature, "59%", true, true);
        let view = PreparedEnvironmentFace::prepare(&face, "environment").unwrap();
        let output = render(
            &view,
            SevenSegmentSelection::Temperature,
            &SevenSegmentLook::fahrenheit(),
        );
        assert_eq!(text(&output.show), expected, "{temperature}");
        assert_eq!(
            output.disposition,
            MaskDisposition::Temperature {
                decimal_places: places
            }
        );
    }
    for temperature in ["999.5°F", "-99.5°F"] {
        let face = face(temperature, "59%", true, true);
        let view = PreparedEnvironmentFace::prepare(&face, "environment").unwrap();
        let output = render(
            &view,
            SevenSegmentSelection::Temperature,
            &SevenSegmentLook::fahrenheit(),
        );
        assert_eq!(text(&output.show), "----");
        assert_eq!(
            output.disposition,
            MaskDisposition::Unavailable(MaskUnavailable::Overflow)
        );
    }
    let face = face("68.09°F", "59.5%", true, true);
    let view = PreparedEnvironmentFace::prepare(&face, "environment").unwrap();
    assert_eq!(
        text(
            &render(
                &view,
                SevenSegmentSelection::Temperature,
                &SevenSegmentLook::celsius()
            )
            .show
        ),
        "20.1C"
    );
    assert_eq!(
        text(
            &render(
                &view,
                SevenSegmentSelection::Humidity,
                &SevenSegmentLook::fahrenheit()
            )
            .show
        ),
        " 60H"
    );
}

#[test]
fn unavailable_and_stale_never_manifest_old_measurements_as_current() {
    for (fresh, available, why) in [
        (false, true, MaskUnavailable::Stale),
        (true, false, MaskUnavailable::Sensor),
    ] {
        let face = face("19.8°C", "59%", fresh, available);
        let view = PreparedEnvironmentFace::prepare(&face, "environment").unwrap();
        for selection in [
            SevenSegmentSelection::Temperature,
            SevenSegmentSelection::Humidity,
        ] {
            let output = render(&view, selection, &SevenSegmentLook::fahrenheit());
            assert_eq!(text(&output.show), "----");
            assert_eq!(output.disposition, MaskDisposition::Unavailable(why));
        }
    }
    let face = face("19.8°C", "101%", true, true);
    assert!(matches!(
        PreparedEnvironmentFace::prepare(&face, "environment"),
        Err(FaceProjectionRefusal::InvalidQuantity)
    ));
    let mut face = self::face("19.8°C", "59%", true, true);
    face.properties
        .retain(|p| p.name != "minute-of-day" && p.name != "clock-fresh");
    let basis = face.basis.clone();
    let face = face.with_basis(basis).unwrap();
    let view = PreparedEnvironmentFace::prepare(&face, "environment").unwrap();
    assert_eq!(
        render(
            &view,
            SevenSegmentSelection::Clock,
            &SevenSegmentLook::fahrenheit()
        )
        .disposition,
        MaskDisposition::Unavailable(MaskUnavailable::MissingValue)
    );
}

#[test]
fn rendering_and_timed_selection_reuse_fixed_storage() {
    let rational_face = face("68.09°F", "59.5%", true, true);
    let rational_view = PreparedEnvironmentFace::prepare(&rational_face, "environment").unwrap();
    let face = face("19.8°C", "59%", true, true);
    let view = PreparedEnvironmentFace::prepare(&face, "environment").unwrap();
    let look = SevenSegmentLook::fahrenheit();
    let alternation = Alternation::new(2000, 12000).unwrap();
    let mut profile = profile();
    profile.colon = profile.decimal_points[1];
    let ((), observed) = allocation_probe::observe(|| {
        for elapsed in [0, 1999, 2000, 3999, 4000, 11999] {
            let output =
                render_for_profile(&view, alternation.select(elapsed).unwrap(), &look, &profile)
                    .unwrap();
            assert!(!matches!(
                output.disposition,
                MaskDisposition::Unavailable(_)
            ));
        }
        let output = render(
            &rational_view,
            SevenSegmentSelection::Temperature,
            &SevenSegmentLook::celsius(),
        );
        assert!(!matches!(
            output.disposition,
            MaskDisposition::Unavailable(_)
        ));
    });
    assert_eq!(observed.allocations, 0);
    assert_eq!(observed.reallocations, 0);
    assert_eq!(
        alternation.select(12000),
        Err(TimingRefusal::OutsideLifetime)
    );
    assert!(Alternation::new(0, 12000).is_err());
}

fn profile() -> Tm1637Profile {
    Tm1637Profile {
        digits: core::array::from_fn(|i| DigitWiring {
            address: i as u8,
            segment_bits: [0, 1, 2, 3, 4, 5, 6],
        }),
        decimal_points: core::array::from_fn(|i| {
            Some(WireBit {
                address: i as u8,
                bit: 7,
            })
        }),
        colon: None,
    }
}
#[test]
fn wiring_exposes_unwired_and_shared_annunciators_and_digit_permutations() {
    let face = face("19.8°C", "59%", true, true);
    let view = PreparedEnvironmentFace::prepare(&face, "environment").unwrap();
    let show = render(
        &view,
        SevenSegmentSelection::Temperature,
        &SevenSegmentLook::fahrenheit(),
    )
    .show;
    let mut profile = profile();
    assert_eq!(profile.map(&show).unwrap(), [0x7d, 0x87, 0x7d, 0x71, 0, 0]);
    profile.digits.swap(0, 3);
    assert_eq!(profile.map(&show).unwrap(), [0x71, 0x87, 0x7d, 0x7d, 0, 0]);
    profile.decimal_points[1] = None;
    assert_eq!(profile.map(&show), Err(WiringRefusal::UnwiredAnnunciator));
    profile.decimal_points[1] = Some(WireBit { address: 1, bit: 7 });
    profile.colon = profile.decimal_points[1];
    assert_eq!(
        profile.map(&show),
        Err(WiringRefusal::SharedControlConflict)
    );
    profile.digits[0].segment_bits[1] = profile.digits[0].segment_bits[0];
    assert_eq!(profile.validate(), Err(WiringRefusal::InvalidProfile));
}

#[test]
fn compact_pattern_form_round_trips_exactly_seven_bits() {
    for byte in 0..128 {
        let pattern = SegmentPatternForm::decode(&[byte]).unwrap();
        assert_eq!(SegmentPatternForm::encode(pattern), [byte]);
    }
    for byte in 128..=255 {
        assert!(SegmentPatternForm::decode(&[byte]).is_err());
    }
    assert!(SegmentPatternForm::decode(&[]).is_err());
    assert!(SegmentPatternForm::decode(&[0, 0]).is_err());
}

#[test]
fn mask_fits_decimal_and_colon_capabilities_before_show_publication() {
    let face = face("19.8°C", "59%", true, true);
    let view = PreparedEnvironmentFace::prepare(&face, "environment").unwrap();
    let look = SevenSegmentLook::fahrenheit();
    let mut profile = profile();
    // A separate controller position can wire a colon independently of all dots.
    profile.colon = Some(WireBit { address: 4, bit: 7 });
    let output =
        render_for_profile(&view, SevenSegmentSelection::Temperature, &look, &profile).unwrap();
    assert_eq!(text(&output.show), "67.6F");
    let clock = render_for_profile(&view, SevenSegmentSelection::Clock, &look, &profile).unwrap();
    assert_eq!(text(&clock.show), "21:37");
    assert_eq!(profile.map(&clock.show).unwrap()[4], 0x80);
    // Aliased dots/colon cannot express opposing states: reduce precision.
    profile.colon = profile.decimal_points[1];
    let output =
        render_for_profile(&view, SevenSegmentSelection::Temperature, &look, &profile).unwrap();
    assert_eq!(text(&output.show), " 68F");
    assert_eq!(
        output.disposition,
        MaskDisposition::Temperature { decimal_places: 0 }
    );
    let clock = render_for_profile(&view, SevenSegmentSelection::Clock, &look, &profile).unwrap();
    assert_eq!(text(&clock.show), "----");
    assert_eq!(
        clock.disposition,
        MaskDisposition::Unavailable(MaskUnavailable::Wiring(
            WiringRefusal::SharedControlConflict
        ))
    );
}
