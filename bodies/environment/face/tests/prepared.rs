#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_core::Quantity;
use conduit_environment_face::{
    PreparedObservationFace, PublicationRefusal, SensorFailure, SensorStatus,
};
use conduit_face::{FaceBasis, FacePropertyValue};
fn basis() -> FaceBasis {
    FaceBasis {
        body_id: None,
        wake_id: None,
        source_document_id: Some("source/environment".into()),
        checked_plot_id: Some("checked/environment".into()),
        expanded_plot_id: Some("expanded/environment".into()),
        plan_id: Some("plan/environment".into()),
        active_play_id: Some("play/environment".into()),
        sign_ids: vec![],
    }
}
fn q(text: &str) -> Quantity {
    Quantity::parse_plot_literal(text).unwrap()
}
#[test]
fn warming_success_failure_expiry_and_recovery_are_allocation_free() {
    let mut owner = PreparedObservationFace::prepare(basis(), 4, 30000).unwrap();
    let reading = (q("19.8°C"), q("59%"));
    assert!(owner
        .current()
        .properties
        .iter()
        .all(|p| p.name != "temperature"));
    let ((first, stale, recovered), allocations) = allocation_probe::observe(|| {
        owner.current().validate().unwrap();
        owner
            .publish(
                None,
                SensorStatus::Unavailable(SensorFailure::CaptureTimeout),
                10,
            )
            .unwrap();
        owner
            .publish(Some(reading), SensorStatus::Available, 100)
            .unwrap();
        let first = owner.at(30100).unwrap().revision;
        assert!(owner
            .current()
            .properties
            .iter()
            .any(|p| p.name == "fresh" && p.value == FacePropertyValue::Flag(true)));
        let stale = owner.at(30101).unwrap().revision;
        assert!(owner
            .current()
            .properties
            .iter()
            .any(|p| p.name == "fresh" && p.value == FacePropertyValue::Flag(false)));
        assert_eq!(owner.at(30200).unwrap().revision, stale);
        owner
            .publish(
                None,
                SensorStatus::Unavailable(SensorFailure::Checksum),
                30300,
            )
            .unwrap();
        let FacePropertyValue::TypedValue { bytes, .. } = &owner
            .current()
            .properties
            .iter()
            .find(|p| p.name == "temperature")
            .unwrap()
            .value
        else {
            unreachable!()
        };
        assert_eq!(Quantity::decode(bytes).unwrap(), reading.0);
        owner
            .publish(Some(reading), SensorStatus::Available, 30400)
            .unwrap();
        assert!(owner
            .current()
            .properties
            .iter()
            .all(|p| p.name != "sensor-failure"));
        owner.current().validate().unwrap();
        (first, stale, owner.current().revision)
    });
    assert!(first < stale && stale < recovered);
    assert_eq!((allocations.allocations, allocations.reallocations), (0, 0));
    assert_eq!(
        owner.publish(Some(reading), SensorStatus::Available, 30500),
        Err(PublicationRefusal::Exhausted)
    );
}
#[test]
fn invalid_publication_is_atomic_and_does_not_invent_quantities() {
    let mut owner = PreparedObservationFace::prepare(basis(), 2, 30000).unwrap();
    let prior = owner.current().clone();
    for (reading, status, expected) in [
        (
            None,
            SensorStatus::Available,
            PublicationRefusal::InvalidStatus,
        ),
        (
            Some((q("20°C"), q("101%"))),
            SensorStatus::Available,
            PublicationRefusal::InvalidHumidity,
        ),
        (
            Some((q("1m"), q("50%"))),
            SensorStatus::Available,
            PublicationRefusal::InvalidTemperature,
        ),
    ] {
        assert_eq!(owner.publish(reading, status, 100), Err(expected));
        assert_eq!(owner.current(), &prior);
    }
    owner
        .publish(Some((q("20°C"), q("50%"))), SensorStatus::Available, 100)
        .unwrap();
    let prior = owner.current().clone();
    assert_eq!(
        owner.publish(None, SensorStatus::Unavailable(SensorFailure::Checksum), 99),
        Err(PublicationRefusal::ClockReversed)
    );
    assert_eq!(owner.current(), &prior);
}

#[test]
fn committed_capture_failures_keep_their_distinct_face_meaning() {
    let mut owner = PreparedObservationFace::prepare(basis(), 4, 30000).unwrap();
    let ((), allocation) = allocation_probe::observe(|| {
        for (index, expected) in [
            (0, "invalid-payload"),
            (1, "capture-timeout"),
            (2, "provider-failed"),
            (3, "geometry-refused"),
        ] {
            owner.publish_result(index, &[], index as u64).unwrap();
            let FacePropertyValue::Text(reason) = &owner
                .current()
                .properties
                .iter()
                .find(|p| p.name == "sensor-failure")
                .unwrap()
                .value
            else {
                unreachable!()
            };
            assert_eq!(reason, expected);
            assert!(owner
                .current()
                .properties
                .iter()
                .all(|p| p.name != "temperature"));
            owner.current().validate().unwrap();
        }
    });
    assert_eq!((allocation.allocations, allocation.reallocations), (0, 0));
    let prior = owner.current().clone();
    assert_eq!(
        owner.publish_result(4, &[], 4),
        Err(PublicationRefusal::InvalidStatus)
    );
    assert_eq!(owner.current(), &prior);
}

#[test]
fn a_recent_retained_observation_stays_fresh_when_the_latest_capture_fails() {
    let mut owner = PreparedObservationFace::prepare(basis(), 2, 30000).unwrap();
    owner
        .publish(Some((q("20°C"), q("50%"))), SensorStatus::Available, 1000)
        .unwrap();
    let ((), allocation) = allocation_probe::observe(|| {
        owner.publish_result(1, &[], 2000).unwrap();
        let face = owner.at(2000).unwrap();
        assert!(face
            .properties
            .iter()
            .any(|p| p.name == "fresh" && p.value == FacePropertyValue::Flag(true)));
        let FacePropertyValue::Text(status) = &face
            .properties
            .iter()
            .find(|p| p.name == "sensor-status")
            .unwrap()
            .value
        else {
            unreachable!()
        };
        assert_eq!(status, "unavailable");
        face.validate().unwrap();
    });
    assert_eq!((allocation.allocations, allocation.reallocations), (0, 0));
}
