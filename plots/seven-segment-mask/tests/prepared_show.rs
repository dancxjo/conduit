use conduit_face::Face;
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
fn face() -> Face {
    serde_json::from_str(include_str!(
        "../../../bodies/environment/evidence/2026-10-23-owner-shared-face/physical-face-1.json"
    ))
    .unwrap()
}
#[test]
fn resolved_pages_match_reference_looks_and_expiry_without_allocating() {
    let face = face();
    let selected = PreparedEnvironmentFace::prepare(&face, "environment").unwrap();
    for dots in [false, true] {
        for look in [
            SevenSegmentLook::fahrenheit().with_colon_fraction(),
            SevenSegmentLook::celsius(),
        ] {
            let mask = TimedSevenSegmentMask::prepare(look, 3000, 6000).unwrap();
            let profile = profile(dots);
            let ((), memory) = allocation_probe::observe(|| {
                let prepared = mask.prepare_show(&selected, &profile).unwrap();
                assert_eq!(prepared.face_content_id(), face.identity.as_str());
                assert_eq!(prepared.face_revision(), face.revision);
                for elapsed in [0, 1, 2999, 3000, 5999, 6000, 6001, u64::MAX] {
                    let reference = mask.render(&selected, elapsed, &profile).unwrap();
                    let actual = prepared.at(elapsed);
                    assert_eq!(actual.show, reference.show);
                    assert_eq!(actual.disposition, reference.disposition);
                    assert_eq!(actual.face_content_id, reference.face_content_id);
                    assert_eq!(actual.face_revision, reference.face_revision);
                }
            });
            assert_eq!((memory.allocations, memory.reallocations), (0, 0));
        }
    }
}
#[test]
fn prepared_material_owns_correlation_after_the_face_read_ends() {
    let expected = face().identity;
    let prepared = {
        let face = face();
        let selected = PreparedEnvironmentFace::prepare(&face, "environment").unwrap();
        TimedSevenSegmentMask::prepare(
            SevenSegmentLook::fahrenheit().with_colon_fraction(),
            3000,
            6000,
        )
        .unwrap()
        .prepare_show(&selected, &profile(false))
        .unwrap()
    };
    assert_eq!(prepared.face_content_id(), expected.as_str());
    assert_eq!(prepared.at(3000).disposition, MaskDisposition::Humidity);
    eprintln!(
        "resolved_alternating_show_inline_bytes={}",
        core::mem::size_of_val(&prepared)
    );
    assert!(core::mem::size_of_val(&prepared) <= 256);
}

#[test]
fn unavailable_pages_and_invalid_wiring_keep_the_existing_refusals() {
    for sensor_lost in [false, true] {
        let mut face = face();
        face.properties
            .iter_mut()
            .find(|p| p.name == "fresh")
            .unwrap()
            .value = conduit_face::FacePropertyValue::Flag(false);
        if sensor_lost {
            face.properties
                .iter_mut()
                .find(|p| p.name == "sensor-status")
                .unwrap()
                .value = conduit_face::FacePropertyValue::Text("unavailable".into());
        }
        face.revision += 1;
        face.reseal_prepared().unwrap();
        let selected = PreparedEnvironmentFace::prepare(&face, "environment").unwrap();
        let mask = TimedSevenSegmentMask::prepare(
            SevenSegmentLook::fahrenheit().with_colon_fraction(),
            3000,
            6000,
        )
        .unwrap();
        let mut wiring = profile(false);
        let prepared = mask.prepare_show(&selected, &wiring).unwrap();
        for elapsed in [0, 3000] {
            assert_eq!(
                prepared.at(elapsed).disposition,
                MaskDisposition::Unavailable(if sensor_lost {
                    MaskUnavailable::Sensor
                } else {
                    MaskUnavailable::Stale
                })
            );
        }
        wiring.digits[1].address = wiring.digits[0].address;
        assert!(matches!(
            mask.prepare_show(&selected, &wiring),
            Err(ShowPreparationRefusal::Wiring(_))
        ));
    }
}
