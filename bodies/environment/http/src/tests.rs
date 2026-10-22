use super::*;
use conduit_core::Quantity;
use conduit_environment_face::SensorStatus;
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn body(response: &str) -> &str {
    response.split_once("\r\n\r\n").unwrap().1
}
fn basis() -> FaceBasis {
    FaceBasis {
        body_id: None,
        wake_id: None,
        source_document_id: Some("source/environment".into()),
        checked_plot_id: Some("checked/environment".into()),
        expanded_plot_id: Some("expanded/environment".into()),
        plan_id: Some("plan/environment".into()),
        active_play_id: Some("play/environment".into()),
        sign_ids: alloc::vec![],
    }
}
#[test]
fn http_mask_consumes_exact_face_revisions_without_allocating() {
    let q = |text| Quantity::parse_plot_literal(text).unwrap();
    initialize(basis());
    assert!(body(&response(0, false)).contains("Waiting for a valid sensor reading"));
    let reading = (q("19°C"), q("55%"));
    let ((current, stale, failed), allocation) = allocation_probe::observe(|| {
        critical_section::with(|cs| {
            PUBLICATION
                .borrow(cs)
                .borrow_mut()
                .as_deref_mut()
                .unwrap()
                .face
                .publish(Some(reading), SensorStatus::Available, 1000)
                .unwrap()
        });
        set_ap_ready();
        let current = response(1000, true);
        publish(1, &[], 2000);
        let failed = response(2000, true);
        let unavailable = seven_segment_snapshot(
            2000,
            conduit_seven_segment_mask::SevenSegmentSelection::Temperature,
            &fixture_display_profile(),
        )
        .unwrap();
        assert_eq!(
            unavailable.disposition,
            conduit_seven_segment_mask::MaskDisposition::Unavailable(
                conduit_seven_segment_mask::MaskUnavailable::Sensor,
            )
        );
        assert!(body(&response(2000, false)).contains("Last known reading"));
        (current, response(31001, true), failed)
    });
    assert_eq!((allocation.allocations, allocation.reallocations), (0, 0));
    let current_json: serde_json::Value = serde_json::from_str(body(&current)).unwrap();
    assert_eq!(current_json["temperature_f"], 66.2);
    assert_eq!(current_json["relative_humidity_percent"], 55);
    assert_eq!(current_json["fresh"], true);
    assert_eq!(current_json["current"], true);
    assert_eq!(current_json["ap_ready"], true);
    assert_eq!(current_json["face_identity"].as_str().unwrap().len(), 64);
    let stale_json: serde_json::Value = serde_json::from_str(body(&stale)).unwrap();
    assert_eq!(stale_json["fresh"], false);
    assert_ne!(stale_json["face_identity"], current_json["face_identity"]);
    let failed_json: serde_json::Value = serde_json::from_str(body(&failed)).unwrap();
    assert_eq!(failed_json["status"], "capture-timeout");
    assert_eq!(failed_json["fresh"], true);
    assert_eq!(failed_json["current"], false);
    assert_eq!(failed_json["sampled_at_boot_ms"], 1000);
    assert_eq!(failed_json["temperature_f"], 66.2);
    assert!(body(&response(32000, false)).contains("Last known reading"));
    let length: usize = current
        .split("Content-Length: ")
        .nth(1)
        .unwrap()
        .split("\r\n")
        .next()
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(length, body(&current).len());
    critical_section::with(|cs| {
        PUBLICATION
            .borrow(cs)
            .borrow_mut()
            .as_deref_mut()
            .unwrap()
            .face
            .publish(Some((q("-10°C"), q("0%"))), SensorStatus::Available, 33000)
            .unwrap()
    });
    let negative: serde_json::Value = serde_json::from_str(body(&response(33000, true))).unwrap();
    assert_eq!(negative["temperature_f"], 14);
    use conduit_seven_segment_mask::{
        MaskDisposition, MaskUnavailable, SevenSegmentSelection as Select,
    };
    let ((show, stale_show), allocation) = allocation_probe::observe(|| {
        let show =
            seven_segment_snapshot(33000, Select::Temperature, &fixture_display_profile()).unwrap();
        let stale_show =
            seven_segment_snapshot(63001, Select::Temperature, &fixture_display_profile()).unwrap();
        (show, stale_show)
    });
    assert_eq!((allocation.allocations, allocation.reallocations), (0, 0));
    assert_eq!(
        core::str::from_utf8(&show.face_identity).unwrap(),
        negative["face_identity"].as_str().unwrap()
    );
    assert_eq!(
        show.face_revision,
        negative["face_revision"].as_u64().unwrap()
    );
    assert!(matches!(
        show.disposition,
        MaskDisposition::Temperature { .. }
    ));
    assert_eq!(
        stale_show.disposition,
        MaskDisposition::Unavailable(MaskUnavailable::Stale)
    );
    assert_ne!(show.face_identity, stale_show.face_identity);
}

fn fixture_display_profile() -> conduit_seven_segment_mask::Tm1637Profile {
    use conduit_seven_segment_mask::{DigitWiring, Tm1637Profile, WireBit};
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
        colon: Some(WireBit { address: 4, bit: 0 }),
    }
}
