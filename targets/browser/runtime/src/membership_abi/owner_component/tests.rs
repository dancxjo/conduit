use super::*;
use serde_json::{json, Value};

fn initialize(boot: &str) {
    let host = "browser/component-test";
    let mut bytes = host.as_bytes().to_vec();
    bytes.extend_from_slice(boot.as_bytes());
    bytes.extend_from_slice(&[7; KEY_BYTES]);
    INPUT.with(|input| input.borrow_mut()[..bytes.len()].copy_from_slice(&bytes));
    assert_eq!(
        conduit_browser_membership_initialize(host.len() as u32, boot.len() as u32),
        STATUS_READY
    );
}
fn request(boot: &str) -> Value {
    let descriptor = owner_component_descriptor().unwrap();
    json!({"host_id":"browser/component-test","boot_id":boot,"component":{
        "identity":descriptor.identity,"version":descriptor.version,"tag":descriptor.tag,"bundle_sha256":descriptor.bundle_sha256
    }})
}
fn admit(value: Value) -> i32 {
    let bytes = serde_json::to_vec(&value).unwrap();
    INPUT.with(|input| input.borrow_mut()[..bytes.len()].copy_from_slice(&bytes));
    conduit_browser_membership_owner_component_admit(bytes.len() as u32)
}
fn exported() -> HostAdvertisement {
    assert_eq!(conduit_browser_membership_advertisement(), STATUS_READY);
    OUTPUT.with(|output| {
        serde_json::from_slice(&output.borrow()[..conduit_browser_membership_output_len() as usize])
            .unwrap()
    })
}
fn has_component(host: &HostAdvertisement) -> bool {
    host.capabilities
        .iter()
        .any(|offer| offer.kind_id.as_str() == conduit_face::COMPONENT_MASK_KIND)
}

#[test]
fn optional_component_is_absent_until_exact_current_incarnation_admission() {
    initialize("boot/first");
    let generic =
        current_advertisement("browser/component-test".into(), "boot/first".into()).unwrap();
    assert!(!has_component(&generic));
    assert!(generic
        .capabilities
        .contains(&crate::installed_browser::dom_mask::offer()));
    assert_eq!(
        conduit_browser_membership_owner_component_descriptor(),
        STATUS_READY
    );
    let descriptor: Value = OUTPUT.with(|output| {
        serde_json::from_slice(&output.borrow()[..conduit_browser_membership_output_len() as usize])
            .unwrap()
    });
    assert_eq!(
        descriptor["bundle_sha256"],
        owner_component_descriptor().unwrap().bundle_sha256
    );
    assert_eq!(admit(request("boot/first")), STATUS_READY);
    let selected = exported();
    assert!(selected
        .capabilities
        .contains(&owner_component_offer().unwrap()));
    assert!(!selected
        .capabilities
        .contains(&crate::installed_browser::dom_mask::configured_offer()));
    assert_eq!(
        selected,
        current_advertisement(selected.host_id.clone(), selected.boot_id.clone()).unwrap()
    );
    assert_eq!(selected, exported());
    assert_eq!(admit(request("boot/first")), ERROR_COMPONENT);
}

#[test]
fn invalid_unknown_and_stale_admissions_leave_the_profile_generic() {
    initialize("boot/current");
    for (key, value) in [
        ("identity", json!("foreign")),
        ("version", json!(2)),
        ("tag", json!("foreign-tag")),
        ("bundle_sha256", json!("0".repeat(64))),
    ] {
        let mut invalid = request("boot/current");
        invalid["component"][key] = value;
        assert_eq!(admit(invalid), ERROR_COMPONENT);
    }
    let mut unknown = request("boot/current");
    unknown["unknown"] = json!(true);
    assert_eq!(admit(unknown), ERROR_COMPONENT);
    let mut wrong_host = request("boot/current");
    wrong_host["host_id"] = json!("foreign/host");
    assert_eq!(admit(wrong_host), ERROR_COMPONENT);
    assert_eq!(admit(request("boot/stale")), ERROR_COMPONENT);
    assert_eq!(
        conduit_browser_membership_owner_component_admit(0),
        ERROR_INPUT
    );
    INPUT.with(|input| input.borrow_mut()[0] = b'{');
    assert_eq!(
        conduit_browser_membership_owner_component_admit(1),
        ERROR_COMPONENT
    );
    assert!(!has_component(&exported()));
}

#[test]
fn first_advertisement_freezes_generic_and_initialize_resets_the_profile() {
    initialize("boot/old");
    assert!(!has_component(&exported()));
    assert_eq!(admit(request("boot/old")), ERROR_COMPONENT);
    initialize("boot/new");
    assert_eq!(admit(request("boot/old")), ERROR_COMPONENT);
    assert_eq!(admit(request("boot/new")), STATUS_READY);
    assert!(has_component(&exported()));
    initialize("boot/third");
    assert!(!has_component(&exported()));
    assert!(current_advertisement("browser/component-test".into(), "boot/new".into()).is_err());
    clear_identity_and_output();
    assert_eq!(
        conduit_browser_membership_owner_component_descriptor(),
        ERROR_NOT_INITIALIZED
    );
    assert_eq!(admit(request("boot/third")), ERROR_NOT_INITIALIZED);
}

#[test]
fn ordinary_plan_selects_only_the_current_admitted_component_artifact() {
    initialize("boot/plan");
    let host =
        || current_advertisement("browser/component-test".into(), "boot/plan".into()).unwrap();
    let source = conduit_face::TODO_COMPONENT_MASK_SOURCE;
    let root = "todo-mask-flat-compact";
    assert!(crate::installed_browser::dom_mask::planned_mask(&host(), source, root).is_err());
    assert_eq!(admit(request("boot/plan")), STATUS_READY);
    let planned = crate::installed_browser::dom_mask::planned_mask(&host(), source, root).unwrap();
    let expected = owner_component_offer().unwrap();
    let renderer = planned
        .plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .find(|placement| placement.kind_id.as_str() == conduit_face::COMPONENT_MASK_KIND)
        .unwrap();
    assert_eq!(
        renderer.implementation_id,
        expected.implementation.implementation_id
    );
    assert_eq!(renderer.artifact_id, expected.implementation.artifact_id);
}
