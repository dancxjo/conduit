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
pub(super) fn basis() -> FaceBasis {
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
        assert!(body(&response(2000, false)).contains("Last known reading"));
        (current, response(31001, true), failed)
    });
    assert_eq!((allocation.allocations, allocation.reallocations), (0, 0));
    let current_json: serde_json::Value = serde_json::from_str(body(&current)).unwrap();
    assert_eq!(current_json["temperature_f"], 66.2);
    assert_eq!(current_json["relative_humidity_percent"], 55);
    assert_eq!(current_json["age_basis"], "sampled");
    assert!(current_json["observed_at_boot_ms"].is_null());
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

#[test]
fn prepared_http_show_owns_only_finite_presentation_and_preserves_observed_time() {
    let show = {
        let mut owner = PreparedObservationFace::prepare_observed(basis(), 2, 30000).unwrap();
        owner
            .publish(
                Some((
                    Quantity::parse_plot_literal("19°C").unwrap(),
                    Quantity::parse_plot_literal("55%").unwrap(),
                )),
                SensorStatus::Available,
                1000,
            )
            .unwrap();
        let before = owner.current().clone();
        let (show, memory) = allocation_probe::observe(|| {
            let show = PreparedHttpShow::prepare(owner.current()).unwrap();
            assert_eq!(show.face_identity(), before.identity.as_str());
            assert_eq!(show.face_revision(), before.revision);
            assert_eq!(owner.current(), &before);
            show
        });
        assert_eq!((memory.allocations, memory.reallocations), (0, 0));
        show
    }; // The immutable Show can leave this borrow; it grants no live route.
    let ((first, second), memory) = allocation_probe::observe(|| {
        (
            show.response(true, 1200, true),
            show.response(true, 1600, true),
        )
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    let first: serde_json::Value = serde_json::from_str(body(&first)).unwrap();
    let second: serde_json::Value = serde_json::from_str(body(&second)).unwrap();
    assert_eq!(first["face_identity"], second["face_identity"]);
    assert_eq!(first["face_revision"], second["face_revision"]);
    assert_eq!(first["temperature_f"], 66.2);
    assert!(first["sampled_at_boot_ms"].is_null());
    assert_eq!(first["observed_at_boot_ms"], 1000);
    assert_eq!(first["age_basis"], "observed");
    assert_eq!(first["age_ms"], 200);
    assert_eq!(second["age_ms"], 600);
    assert_eq!(show.time_coordinate(), HttpShowTime::ObservedAtBootMs(1000));
    std::println!(
        "prepared_http_show_cpu inline_bytes={} heap_bytes=0 pointer_bits={}",
        core::mem::size_of::<PreparedHttpShow>(),
        usize::BITS
    );
}

#[test]
fn warming_show_does_not_invent_a_zero_acquisition_or_observation_coordinate() {
    let owner = PreparedObservationFace::prepare(basis(), 2, 30000).unwrap();
    let show = PreparedHttpShow::prepare(owner.current()).unwrap();
    assert_eq!(show.time_coordinate(), HttpShowTime::Unavailable);
    let response = show.response(true, 1000, true);
    let json: serde_json::Value = serde_json::from_str(body(&response)).unwrap();
    assert!(json["sampled_at_boot_ms"].is_null());
    assert!(json["observed_at_boot_ms"].is_null());
    assert!(json["age_ms"].is_null());
    assert_eq!(json["age_basis"], "unavailable");
}

#[test]
fn sealed_available_face_requires_temperature_but_humidity_remains_optional() {
    let mut owner = PreparedObservationFace::prepare(basis(), 2, 30000).unwrap();
    owner
        .publish(
            Some((
                Quantity::parse_plot_literal("19°C").unwrap(),
                Quantity::parse_plot_literal("55%").unwrap(),
            )),
            SensorStatus::Available,
            1000,
        )
        .unwrap();
    // The generic environmental contribution permits temperature-only readings.
    let mut temperature_only = owner.current().clone();
    temperature_only.properties.retain(|property| {
        !(property.subject == "environment" && property.name == "relative-humidity")
    });
    temperature_only.reseal_prepared().unwrap();
    temperature_only.validate().unwrap();
    let response = face_response(&temperature_only, true, 1000, true).unwrap();
    let json: serde_json::Value = serde_json::from_str(body(&response)).unwrap();
    assert_eq!(json["current"], true);
    assert_eq!(json["temperature_f"], 66.2);
    assert!(json["relative_humidity_percent"].is_null());

    // A valid Face seal authenticates its bytes, not the environmental domain.
    let mut missing_temperature = owner.current().clone();
    missing_temperature
        .properties
        .retain(|property| !(property.subject == "environment" && property.name == "temperature"));
    missing_temperature.reseal_prepared().unwrap();
    missing_temperature.validate().unwrap();
    assert!(matches!(
        PreparedHttpShow::prepare(&missing_temperature),
        Err(conduit_seven_segment_mask::FaceProjectionRefusal::MissingProperty)
    ));

    let mut unrenderable = owner.current().clone();
    let property = unrenderable
        .properties
        .iter_mut()
        .find(|property| property.subject == "environment" && property.name == "temperature")
        .unwrap();
    let conduit_face::FacePropertyValue::TypedValue { bytes, .. } = &mut property.value else {
        panic!("typed temperature");
    };
    *bytes = Quantity::parse_plot_literal("0.0000001°C")
        .unwrap()
        .encode()
        .to_vec();
    unrenderable.reseal_prepared().unwrap();
    unrenderable.validate().unwrap();
    assert!(matches!(
        PreparedHttpShow::prepare(&unrenderable),
        Err(conduit_seven_segment_mask::FaceProjectionRefusal::InvalidQuantity)
    ));
}
