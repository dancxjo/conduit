//! Distinct selected-Full refusal classes through its actual Rust ABI. These
//! CPU fixtures exercise kernel/HostCall/ack custody, not a browser or live Line.
use super::super::full_profile_pool as pool;
use super::super::*;
use super::FullPreparationRefusal as R;
use conduit_browser_mask_offer::full_profile as profile;
#[path = "fixture.rs"]
mod fixture;
use fixture::Fixture;

#[test]
fn malformed_owned_face_and_unsupported_grammar_keep_the_acknowledged_current() {
    let f = Fixture::new();
    let current = f.available();
    let original = String::from_utf8(f.frame(&f.face, &f.route)).unwrap();
    assert!(original.contains("\"revision\":3"));
    let malformed = original.replacen("\"revision\":3", "\"revision\":{}", 1);
    assert_eq!(
        f.submit(&f.basis(), malformed.as_bytes()),
        R::Malformed.status()
    );
    assert_eq!(f.current(), current);
    let (face, route) = f.unsupported_face();
    face.validate().unwrap();
    route.verify_seal().unwrap();
    assert_eq!(
        f.submit(&f.basis(), &f.frame(&face, &route)),
        R::Unsupported.status()
    );
    assert_eq!(f.current(), current);
    let mut look = f.basis();
    look["visual_constraints"] =
        serde_json::json!({"minimum_text_scale_percent":100,"forced_colors":true});
    assert_eq!(
        f.submit(&look, &f.frame(&f.face, &f.route)),
        R::Unsupported.status()
    );
    assert_eq!(f.current(), current);
}
#[test]
fn foreign_basis_and_broken_seal_are_distinct_and_preserve_current() {
    let f = Fixture::new();
    let current = f.available();
    let mut foreign = f.basis();
    foreign["host_id"] = "host/foreign-browser".into();
    assert_eq!(
        f.submit(&foreign, &f.frame(&f.face, &f.route)),
        R::Identity.status()
    );
    assert_eq!(f.current(), current);
    let mut changed = f.route.clone();
    changed.route_plan_id = "plan/foreign-seal".into();
    assert_eq!(
        f.submit(&f.basis(), &f.frame(&f.face, &changed)),
        R::InvalidSeal.status()
    );
    assert_eq!(f.current(), current);
}
#[test]
fn exhausted_slots_and_byte_allowance_report_pressure_without_retiring_current() {
    let f = Fixture::new();
    let current = f.available();
    let extra = pool::Lease::acquire_current().unwrap();
    let before = extra.admitted();
    assert_eq!(
        extra.admit(profile::PREPARATION_BYTES as usize + 1),
        Err(R::Pressure)
    );
    assert_eq!(extra.admitted(), before);
    assert_eq!(
        f.submit(&f.basis(), &f.frame(&f.face, &f.route)),
        R::Pressure.status()
    );
    assert_eq!(f.current(), current);
    drop(extra);
    let replacement = pool::Lease::acquire_current().unwrap();
    assert!(replacement.matches(&f.route.mask_host.host_id, &f.route.mask_host.boot_id));
    drop(replacement);
    assert_eq!(f.current(), current);
}
#[test]
fn prepared_show_lifecycle_refusal_preserves_the_original_acknowledgement() {
    let f = Fixture::new();
    let prepared = f.install();
    assert_eq!(
        f.submit(&f.basis(), &f.frame(&f.face, &f.route)),
        R::Lifecycle.status()
    );
    f.acknowledge(&prepared);
    let current: serde_json::Value = serde_json::from_slice(&f.current()).unwrap();
    assert_eq!(current["face"]["identity"], f.face.identity.as_str());
    assert_eq!(
        current["face_revision_decimal"],
        f.face.revision.to_string()
    );
}
#[test]
fn closed_refusal_codes_remain_distinct_from_unclassified_preparation_errors() {
    use conduit_face::json_model_preflight::JsonModelRefusal as J;
    assert_eq!(R::from(J::Capacity), R::Pressure);
    assert_eq!(R::from(J::Json), R::Malformed);
    assert_eq!(R::from(J::Unsupported), R::Unsupported);

    let values = [
        R::Malformed,
        R::Unsupported,
        R::Identity,
        R::InvalidSeal,
        R::Pressure,
        R::Lifecycle,
        R::Preparation,
    ];
    let statuses = values.map(R::status);
    assert_eq!(statuses, [-270, -271, -272, -273, -274, -275, -276]);
    for (i, a) in values.iter().enumerate() {
        for b in &values[..i] {
            assert_ne!(a.code(), b.code());
        }
    }
    assert_eq!(
        R::from(String::from("unclassified shared helper")),
        R::Preparation
    );
}

#[test]
fn full_initialized_unselected_legacy_frame_keeps_original_refusal_codes() {
    let f = Fixture::new();
    let prepared = f.install();
    let legacy = f.legacy_frame();
    assert!(legacy.len() <= conduit_face::MAX_OWNER_FACE_RESPONSE_BYTES);
    // A real, unselected Legacy Plan/route reaches the unchanged Legacy pending
    // guard. Selector parsing itself still uses an admitted Full-Host lease;
    // there is no unadmitted discriminator or promise under slot exhaustion.
    assert_eq!(f.submit(&f.basis(), &legacy), -8);
    f.acknowledge(&prepared);
    let current = f.current();
    assert_eq!(f.submit(&f.basis(), br#"{"kind":"#), -2);
    assert_eq!(f.current(), current);
}

#[test]
fn bounded_output_capacity_and_serializer_failure_have_distinct_refusals() {
    assert_eq!(super::bounded_json(&"abc", 5).unwrap(), b"\"abc\"");
    assert_eq!(super::bounded_json(&"abc", 4), Err(R::Pressure));
    struct Fails;
    impl serde::Serialize for Fails {
        fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
            Err(serde::ser::Error::custom("fixture serialization refusal"))
        }
    }
    assert_eq!(super::bounded_json(&Fails, 128), Err(R::Preparation));
}
