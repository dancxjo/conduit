//! Actual target Mask/cache code with a real semantic Face and CPU clock input.
//! Native admission, sockets, radio and Body membership are not fixtures' claims.
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "../../../../targets/esp32/proof/tm1637/src/native_face_http.rs"]
mod native;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_core::Quantity;
use conduit_environment_face::{PreparedObservationFace, SensorFailure, SensorStatus};
use conduit_face::FaceBasis;

#[test]
fn target_cache_preserves_face_time_expiry_failure_and_withdrawal_without_allocation() {
    let mut face = PreparedObservationFace::prepare_observed(
        FaceBasis {
            body_id: None,
            wake_id: None,
            source_document_id: Some("source/native-mask-fixture".into()),
            checked_plot_id: Some("checked/native-mask-fixture".into()),
            expanded_plot_id: Some("expanded/native-mask-fixture".into()),
            plan_id: Some("plan/native-mask-fixture".into()),
            active_play_id: Some("play/native-mask-fixture".into()),
            sign_ids: vec![],
        },
        3,
        30,
    )
    .unwrap();
    let (mut publisher, control) = native::prepare(3, 30).unwrap();
    assert!(native::response(1000, true).is_none());
    let values = (
        Quantity::parse_plot_literal("19°C").unwrap(),
        Quantity::parse_plot_literal("55%").unwrap(),
    );
    let ((first, failed, stale), measured) = allocation_probe::observe(|| {
        face.publish(Some(values), SensorStatus::Available, 1000)
            .unwrap();
        publisher.publish(face.current(), 1000).unwrap();
        let first = native::response(1010, true).unwrap();
        assert!(first.contains(face.current().identity.as_str()));
        face.publish(
            None,
            SensorStatus::Unavailable(SensorFailure::Checksum),
            1005,
        )
        .unwrap();
        publisher.publish(face.current(), 1005).unwrap();
        let failed = native::response(1030, true).unwrap();
        // A failed later capture cannot extend the previous quantities' freshness.
        assert!(native::response(1031, true).is_none());
        face.publish(
            None,
            SensorStatus::Unavailable(SensorFailure::Checksum),
            1040,
        )
        .unwrap();
        publisher.publish(face.current(), 1040).unwrap();
        let stale = native::response(1041, true).unwrap();
        control.withdraw();
        assert!(native::response(1041, true).is_none());
        assert_eq!(
            publisher.publish(face.current(), 1041),
            Err("http-mask-publication")
        );
        (first, failed, stale)
    });
    assert_eq!((measured.allocations, measured.reallocations), (0, 0));
    let parse = |text: &str| {
        serde_json::from_str::<serde_json::Value>(text.split_once("\r\n\r\n").unwrap().1).unwrap()
    };
    let first = parse(&first);
    assert!(first["sampled_at_boot_ms"].is_null());
    assert_eq!(first["observed_at_boot_ms"], 1000);
    assert_eq!(first["temperature_f"], 66.2);
    let failed = parse(&failed);
    assert_eq!(failed["status"], "checksum");
    assert_eq!(failed["fresh"], true);
    assert_eq!(failed["current"], false);
    assert_eq!(parse(&stale)["fresh"], false);
}
