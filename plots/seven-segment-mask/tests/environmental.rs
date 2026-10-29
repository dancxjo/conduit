use conduit_core::{
    kind_id, CheckedValueContract, Quantity, QUANTITY_ENCODED_LEN, QUANTITY_INFO_ID, RATIO_INFO_ID,
    TEMPERATURE_INFO_ID,
};
use conduit_face::{Face, FaceBasis, FaceProperty, FacePropertyValue, FaceRole, FaceSubject};
use conduit_seven_segment_mask::*;
#[path = "../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

fn profile(dots: bool) -> Tm1637Profile {
    Tm1637Profile {
        digits: core::array::from_fn(|n| DigitWiring {
            address: n as u8,
            segment_bits: [0, 1, 2, 3, 4, 5, 6],
        }),
        decimal_points: core::array::from_fn(|n| {
            dots.then_some(WireBit {
                address: n as u8,
                bit: 7,
            })
        }),
        colon: Some(WireBit { address: 1, bit: 7 }),
    }
}
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
fn environment_face(temperature: &str, humidity: &str, fresh: bool, available: bool) -> Face {
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
fn barometric_face(humidity: bool, pressure: Option<&str>, fresh: bool, available: bool) -> Face {
    let mut original = environment_face("20°C", "44%", fresh, available);
    if !humidity {
        original
            .properties
            .retain(|p| p.name != "relative-humidity");
    }
    if let Some(pressure) = pressure {
        original
            .properties
            .push(quantity("atmospheric-pressure", QUANTITY_INFO_ID, pressure));
    }
    Face::new(
        original.revision,
        original.basis,
        original.subjects,
        original.relationships,
        original.properties,
        original.text,
    )
    .unwrap()
}
fn digits(show: &SevenSegmentShow) -> [u8; 4] {
    (*show.digits()).map(|digit| SegmentPatternForm::encode(digit)[0])
}
#[test]
fn typed_bme_bmp_and_dht_select_only_supported_pages_with_exact_hpa_rounding() {
    for (humidity, pressure, expected) in [
        (
            true,
            Some("101234Pa"),
            vec![
                MaskDisposition::Temperature { decimal_places: 0 },
                MaskDisposition::Humidity,
                MaskDisposition::PressureHectopascal,
            ],
        ),
        (
            false,
            Some("101234Pa"),
            vec![
                MaskDisposition::Temperature { decimal_places: 0 },
                MaskDisposition::PressureHectopascal,
            ],
        ),
        (
            true,
            None,
            vec![
                MaskDisposition::Temperature { decimal_places: 0 },
                MaskDisposition::Humidity,
            ],
        ),
    ] {
        let face = barometric_face(humidity, pressure, true, true);
        let view = PreparedEnvironmentalFace::prepare(&face, "environment").unwrap();
        let mask = TimedEnvironmentalMask::prepare(
            SevenSegmentLook::fahrenheit().with_whole_degrees(),
            3000,
            18000,
        )
        .unwrap();
        let (prepared, preparation_cost) =
            allocation_probe::observe(|| mask.prepare_show(&view, &profile(false)).unwrap());
        assert_eq!(
            (preparation_cost.allocations, preparation_cost.reallocations),
            (0, 0)
        );
        for tick in 0..6 {
            let elapsed = tick * 3000;
            let (rendered, cost) =
                allocation_probe::observe(|| mask.render(&view, elapsed, &profile(false)).unwrap());
            assert_eq!((cost.allocations, cost.reallocations), (0, 0));
            assert_eq!(
                rendered.disposition,
                expected[tick as usize % expected.len()]
            );
            let resolved = prepared.at(elapsed);
            assert_eq!(resolved.disposition, rendered.disposition);
            assert_eq!(digits(&resolved.show), digits(&rendered.show));
            assert_eq!(resolved.face_content_id, face.identity.as_str());
            assert_eq!(resolved.face_revision, face.revision);
            assert!(!rendered.show.colon());
            assert_eq!(rendered.show.decimal_points(), &[false; 4]);
            if rendered.disposition == MaskDisposition::PressureHectopascal {
                assert_eq!(digits(&rendered.show), [0x06, 0x3f, 0x06, 0x5b]);
            }
        }
        assert_eq!(
            mask.render(&view, 18000, &profile(false))
                .unwrap()
                .disposition,
            MaskDisposition::Unavailable(MaskUnavailable::OutsideLifetime)
        );
        assert_eq!(
            prepared.at(18000).disposition,
            MaskDisposition::Unavailable(MaskUnavailable::OutsideLifetime)
        );
    }
}
#[test]
fn pressure_uses_exact_units_rounding_full_four_digits_and_overflow_refusal() {
    for (literal, expected) in [
        ("100050Pa", Some([0x06, 0x3f, 0x3f, 0x06])),
        ("1atm", Some([0x06, 0x3f, 0x06, 0x4f])),
        ("999900Pa", Some([0x6f; 4])),
        ("999950Pa", None),
    ] {
        let face = barometric_face(false, Some(literal), true, true);
        let view = PreparedEnvironmentalFace::prepare(&face, "environment").unwrap();
        let mask =
            TimedEnvironmentalMask::prepare(SevenSegmentLook::fahrenheit(), 3000, 6000).unwrap();
        let output = mask.render(&view, 3000, &profile(false)).unwrap();
        match expected {
            Some(expected) => {
                assert_eq!(digits(&output.show), expected);
                assert_eq!(output.disposition, MaskDisposition::PressureHectopascal);
            }
            None => assert_eq!(
                output.disposition,
                MaskDisposition::Unavailable(MaskUnavailable::Overflow)
            ),
        }
    }
}
#[test]
fn pressure_obeys_freshness_sensor_state_and_exact_quantity_family() {
    for (fresh, available, refusal) in [
        (false, true, MaskUnavailable::Stale),
        (true, false, MaskUnavailable::Sensor),
    ] {
        let face = barometric_face(true, Some("101234Pa"), fresh, available);
        let view = PreparedEnvironmentalFace::prepare(&face, "environment").unwrap();
        let mask =
            TimedEnvironmentalMask::prepare(SevenSegmentLook::fahrenheit(), 3000, 9000).unwrap();
        assert_eq!(
            mask.render(&view, 6000, &profile(false))
                .unwrap()
                .disposition,
            MaskDisposition::Unavailable(refusal)
        );
        assert_eq!(
            mask.prepare_show(&view, &profile(false))
                .unwrap()
                .at(6000)
                .disposition,
            MaskDisposition::Unavailable(refusal)
        );
    }
    for invalid in ["0Pa", "-1Pa", "20°C", "44%"] {
        let face = barometric_face(true, Some(invalid), true, true);
        assert!(
            matches!(
                PreparedEnvironmentalFace::prepare(&face, "environment"),
                Err(FaceProjectionRefusal::InvalidQuantity)
            ),
            "{invalid}"
        );
    }
    assert!(matches!(
        TimedEnvironmentalMask::prepare(SevenSegmentLook::fahrenheit(), 0, 1),
        Err(TimingRefusal::InvalidBounds)
    ));
}
