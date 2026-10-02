use conduit_body::{Body, BodyIdentityDerivation, BodyLifecycleError, BodyLifecycleEvent};
use conduit_core::{CheckedPlotId, SignId, SourceDocumentId};

fn born(sign: &str) -> Body {
    Body::born(
        SourceDocumentId::from("source/seed"),
        CheckedPlotId::from("checked/seed"),
        1,
        SignId::from(sign),
    )
    .unwrap()
}

#[test]
fn independent_birth_signs_distinguish_identical_worksets_and_sequences() {
    let one = born("host/one/boot/one/birth");
    let other_host = born("host/two/boot/one/birth");
    let other_boot = born("host/one/boot/two/birth");
    assert_eq!(
        one.identity_derivation,
        BodyIdentityDerivation::InitialWorksetBirthSignV3
    );
    assert_ne!(one.body_id, other_host.body_id);
    assert_ne!(one.body_id, other_boot.body_id);
    assert_eq!(one.body_id, born("host/one/boot/one/birth").body_id);
    one.validate().unwrap();
    other_host.validate().unwrap();
    other_boot.validate().unwrap();
}

#[test]
fn changing_v3_birth_sign_cannot_preserve_body_identity() {
    let mut body = born("host/one/boot/one/birth");
    let changed = SignId::from("host/two/boot/one/birth");
    body.sign_ids[0] = changed.clone();
    let BodyLifecycleEvent::Born { sign_id, .. } = &mut body.events[0] else {
        panic!("birth event");
    };
    *sign_id = changed;
    assert_eq!(body.validate(), Err(BodyLifecycleError::InvalidIdentity));
}

#[test]
fn retained_v2_json_validates_and_keeps_identity_through_new_transitions() {
    // Exact V2 hash captured from its length-prefixed workset/sequence encoding.
    const LEGACY: &str = r#"{
      "body_id":"e45687f0974dac699d2de4d8dc246e2a1f457bb2561770d86803a1eee0840122",
      "identity_derivation":"InitialWorksetV2",
      "workset":{"plots":[{"source_document_id":"source/seed","checked_plot_id":"checked/seed"}]},
      "workload_revision":0,"birth_sequence":1,"state":"Lulled","sign_ids":["sign/born"],
      "events":[{"Born":{"initial_workset":{"plots":[{"source_document_id":"source/seed","checked_plot_id":"checked/seed"}]},"workload_revision":0,"sign_id":"sign/born"}}]
    }"#;
    let retained: Body = serde_json::from_str(LEGACY).unwrap();
    retained.validate().unwrap();
    let (awake, _) = retained.wake(1, SignId::from("sign/wake")).unwrap();
    awake.validate().unwrap();
    assert_eq!(awake.body_id, retained.body_id);
    let restored: Body = serde_json::from_slice(&serde_json::to_vec(&awake).unwrap()).unwrap();
    restored.validate().unwrap();
    assert_eq!(
        restored.identity_derivation,
        BodyIdentityDerivation::InitialWorksetV2
    );
}

#[test]
fn v3_round_trip_retains_exact_birth_identity() {
    let original = born("host/one/boot/one/birth");
    let restored: Body = serde_json::from_slice(&serde_json::to_vec(&original).unwrap()).unwrap();
    restored.validate().unwrap();
    assert_eq!(restored, original);
}
