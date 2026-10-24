use super::*;
use conduit_core::Quantity;
use conduit_environment_face::SensorStatus;
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
pub(super) mod allocation_probe;
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

#[test]
fn recorder_http_mask_projects_the_same_sealed_face_without_allocating() {
    use conduit_environment_face::{
        PreparedRecordingFace, RecordingRecoveryFailure, RecoveredRecordingRange,
    };
    let mut owner = PreparedRecordingFace::prepare(basis(), 2, 128, 4).unwrap();
    owner
        .publish_recovered(RecoveredRecordingRange {
            chunks: 1,
            samples: 2,
            first_sequence: 12,
            last_sequence: 14,
        })
        .unwrap();
    let ((json, html), memory) = allocation_probe::observe(|| {
        (
            recording_response(owner.current(), true).unwrap(),
            recording_response(owner.current(), false).unwrap(),
        )
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    let show: serde_json::Value = serde_json::from_str(body(&json)).unwrap();
    assert_eq!(show["face_identity"], owner.current().identity.as_str());
    assert_eq!(show["face_revision"], owner.current().revision);
    assert_eq!(show["samples"], 2);
    assert_eq!(show["missing_sequences"], 1);
    assert_eq!(show["recovery_status"], "recovered");
    assert!(body(&html).contains("Sequences 12 through 14; 1 missing"));
    assert!(show.get("durable").is_none());
    assert!(show.get("fresh").is_none());
    owner
        .publish_refusal(RecordingRecoveryFailure::Capacity)
        .unwrap();
    let refused = recording_response(owner.current(), true).unwrap();
    let refused: serde_json::Value = serde_json::from_str(body(&refused)).unwrap();
    assert_eq!(refused["recovery_failure"], "capacity");
    assert_eq!(refused["last_sequence"], 14);
    assert_ne!(show["face_identity"], refused["face_identity"]);
}
#[test]
fn recorder_mask_refuses_a_foreign_or_semantically_inconsistent_face() {
    use conduit_environment_face::{PreparedRecordingFace, RecoveredRecordingRange};
    assert_eq!(
        recording_response(
            PreparedObservationFace::prepare(basis(), 2, 100)
                .unwrap()
                .current(),
            true
        ),
        Err(RecordingMaskRefusal::UnsupportedFace)
    );
    let mut owner = PreparedRecordingFace::prepare(basis(), 2, 128, 4).unwrap();
    let empty = recording_response(owner.current(), true).unwrap();
    assert!(body(&empty).contains("\"has_range\":false"));
    owner
        .publish_recovered(RecoveredRecordingRange {
            chunks: 1,
            samples: 2,
            first_sequence: 12,
            last_sequence: 14,
        })
        .unwrap();
    let mut forged = owner.current().clone();
    forged
        .properties
        .iter_mut()
        .find(|p| p.name == "missing-sequences")
        .unwrap()
        .value = conduit_face::FacePropertyValue::Count(0);
    forged.reseal_prepared().unwrap();
    assert_eq!(
        recording_response(&forged, true),
        Err(RecordingMaskRefusal::UnsupportedFace)
    );
    forged.properties.retain(|p| p.name != "samples");
    forged.reseal_prepared().unwrap();
    assert_eq!(
        recording_response(&forged, true),
        Err(RecordingMaskRefusal::UnsupportedFace)
    );
}

#[test]
fn external_face_mask_keeps_environment_namespace_and_refuses_unsealed_input() {
    let owner = PreparedObservationFace::prepare(basis(), 2, 30000).unwrap();
    let mut face = owner.current().clone();
    let mut other = face
        .subjects
        .iter()
        .find(|subject| subject.identity == "environment")
        .unwrap()
        .clone();
    other.identity = "other".into();
    face.subjects.push(other);
    let mut property = face
        .properties
        .iter()
        .find(|property| property.subject == "environment" && property.name == "fresh")
        .unwrap()
        .clone();
    property.subject = "other".into();
    property.value = conduit_face::FacePropertyValue::Flag(true);
    face.properties.insert(0, property);
    face.reseal_prepared().unwrap();
    let response = face_response(&face, true, 0, true).unwrap();
    let json: serde_json::Value = serde_json::from_str(body(&response)).unwrap();
    assert_eq!(json["fresh"], false);
    assert_eq!(json["face_identity"], face.identity.as_str());
    face.properties[0].value = conduit_face::FacePropertyValue::Flag(false);
    assert!(matches!(
        face_response(&face, true, 0, true),
        Err(conduit_seven_segment_mask::FaceProjectionRefusal::InvalidFace)
    ));
}
