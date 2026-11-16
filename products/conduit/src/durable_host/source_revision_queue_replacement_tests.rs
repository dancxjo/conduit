use super::super::{candidate, verify_capture_and_fore, Request};
use super::*;
use crate::durable_host::local_fore_policy::validate_checked_source;

const SOURCE: &str = r#"plot hello (
    >> input: Text...|
    output: Text...| >>
) {
    upper: text/upper
    input >> upper
    upper >> output
}
"#;

fn fixture() -> (std::path::PathBuf, Installation, Selection, Request) {
    let (manifest, state) = super::super::super::super::tests::fixture();
    let mut installation = install(&manifest, &state).unwrap();
    let configuration = crate::joining_host_package_test_common::configuration(false);
    let (_, spore, _, _) = crate::joining_host_package_test_common::fixture_for(
        &["conduit-linux-x86_64", "install-linux-x86_64.sh"],
        false,
    );
    installation.selected_host_configuration = Some(serde_json::from_value(serde_json::json!({
        "source": conduit_host_make::canonical_host_configuration_conduit(configuration.configuration()).unwrap(),
        "configuration_id": configuration.configuration_id(), "host_image": spore.image,
    })).unwrap());
    let parsed = crate::plot_source::parse(SOURCE).unwrap();
    let document = parsed.check().unwrap();
    let plot = parsed
        .expand_checked_for_authoring(&document, "hello")
        .unwrap();
    let body = conduit_body::Body::born(
        plot.expanded.source_document_id.clone(),
        plot.expanded.checked_plot_id.clone(),
        1,
        "sign/queue-renewal/born".into(),
    )
    .unwrap();
    let expected = BodyBiographyEvidence::born(
        body.clone(),
        conduit_body::BodyMembership::new(body.body_id.clone()).unwrap(),
        "Queue renewal".into(),
    )
    .unwrap();
    let biography = serde_json::to_vec(&expected).unwrap();
    installation.body_state = Some(crate::durable_host::BodyBinding {
        body_id: body.body_id.as_str().into(),
        biography_sha256: digest(&biography),
        biography_path: state.join("biography.json").display().to_string(),
    });
    let ports: Vec<_> = plot.front.inputs().iter().chain(plot.front.outputs()).map(|port| serde_json::json!({
        "front_port_id":port.port_id,"direction":port.direction,"track":ConnectionTrack::Payload,
        "value_kind":port.value_kind,"item_capacity":1,"byte_capacity":268,
    })).collect();
    let selection: Selection = serde_json::from_value(serde_json::json!({
        "host_id":installation.host_id, "configuration_id":configuration.configuration_id(),
        "release_bundle_sha256":installation.release_bundle_sha256, "expected":expected,
        "file":state.join("source.conduit"), "snapshot":{"owner":"queue-renewal", "generation":1,
            "sha256":format!("{:x}", Sha256::digest(SOURCE.as_bytes())),
            "source_document_id":conduit_plot::canonical_source_document_id(SOURCE).0},
        "entries":["hello"], "maximum_source_bytes":65536,
        "capture_preparation_bytes":131072,"capture_retained_bytes":65536,"caller_workspace_bytes":1048576,
        "local_fore_adapters":[{"entry":"hello","host_id":"host/receiver",
            "configuration_id":format!("sha256:{}", "a".repeat(64)),"ports":ports}]
    })).unwrap();
    let request = Request {
        original_policy_sha256: digest(&serde_json::to_vec(&selection).unwrap()),
        previous_release_bundle_sha256: installation.release_bundle_sha256.clone(),
        installed_release_bundle_sha256: installation.release_bundle_sha256.clone(),
        expected: selection.expected.clone(),
        retire_actions: false,
        configuration_pin_replacements: Vec::new(),
        local_fore_queue_replacements: vec![replacement(&selection, 0)],
    };
    (state, installation, selection, request)
}
fn replacement(selection: &Selection, index: usize) -> LocalForeQueueReplacement {
    let intent = &selection.local_fore_adapters[0];
    let port = &intent.ports[index];
    LocalForeQueueReplacement {
        entry: intent.entry.clone(),
        host_id: intent.host_id.as_str().into(),
        front_port_id: port.front_port_id.as_str().into(),
        direction: port.direction,
        track: port.track,
        expected_item_capacity: port.item_capacity,
        expected_byte_capacity: port.byte_capacity,
        selected_item_capacity: port.item_capacity,
        selected_byte_capacity: 651,
    }
}
fn json(selection: &Selection) -> serde_json::Value {
    serde_json::to_value(selection).unwrap()
}

#[test]
fn same_release_queue_cas_changes_only_reviewed_preferences_and_audits_exact_delta() {
    let (state, installation, selection, request) = fixture();
    let original = serde_json::to_vec(&selection).unwrap();
    let (new, _) = candidate(&original, &request, &installation, &request.expected).unwrap();
    verify_capture_and_fore(&new, SOURCE, &selection.local_fore_adapters).unwrap();
    let mut expected = json(&selection);
    expected["local_fore_adapters"][0]["ports"][0]["byte_capacity"] = 651.into();
    assert_eq!(json(&new), expected);
    let audit = serde_json::to_value(&request.local_fore_queue_replacements).unwrap();
    assert_eq!(audit[0]["expected_byte_capacity"], 268);
    assert_eq!(audit[0]["selected_byte_capacity"], 651);
    assert_eq!(
        json(&selection)["local_fore_adapters"][0]["ports"][0]["byte_capacity"],
        268
    );
    fs::remove_dir_all(state.parent().unwrap()).unwrap();
}

#[test]
fn batch_is_atomic_and_rejects_stale_duplicate_unselected_noop_or_invalid_capacity() {
    let (state, _, selection, request) = fixture();
    let good = request.local_fore_queue_replacements[0].clone();
    let mut stale = good.clone();
    stale.expected_byte_capacity = 267;
    let mut unknown = good.clone();
    unknown.host_id = "host/unselected".into();
    let mut noop = good.clone();
    noop.selected_byte_capacity = 268;
    let mut overflow = good.clone();
    overflow.selected_byte_capacity = 16385;
    let mut zero = good.clone();
    zero.selected_item_capacity = 0;
    let mut items = good.clone();
    items.selected_item_capacity = 33;
    let mut bytes = good.clone();
    bytes.selected_byte_capacity = 0;
    let mut front = good.clone();
    front.front_port_id = "unselected".into();
    let mut track = good.clone();
    track.track = ConnectionTrack::Quiescence;
    for invalid in [
        stale, unknown, noop, overflow, zero, items, bytes, front, track,
    ] {
        let mut new = selection.clone();
        assert!(replace_queues(&mut new, &[invalid]).is_err());
        assert_eq!(json(&new), json(&selection));
    }
    let mut second = replacement(&selection, 1);
    second.expected_byte_capacity = 267;
    let mut new = selection.clone();
    assert!(replace_queues(&mut new, &[good.clone(), second]).is_err());
    assert_eq!(json(&new), json(&selection));
    let mut new = selection.clone();
    assert!(replace_queues(&mut new, &[good.clone(), good]).is_err());
    assert_eq!(json(&new), json(&selection));
    fs::remove_dir_all(state.parent().unwrap()).unwrap();
}

#[test]
fn malformed_old_capacity_or_ambiguous_old_boundary_cannot_be_repaired() {
    let (state, _, selection, request) = fixture();
    for duplicate in [false, true] {
        let mut old = selection.clone();
        if duplicate {
            let port = old.local_fore_adapters[0].ports[0].clone();
            old.local_fore_adapters[0].ports.push(port);
        } else {
            old.local_fore_adapters[0].ports[0].byte_capacity = 0;
        }
        let before = json(&old);
        assert!(replace_queues(&mut old, &request.local_fore_queue_replacements).is_err());
        assert_eq!(json(&old), before);
    }
    fs::remove_dir_all(state.parent().unwrap()).unwrap();
}

#[test]
fn original_and_proposed_source_contracts_are_checked_without_fabricated_offers() {
    let (state, _, selection, _) = fixture();
    let parsed = crate::plot_source::parse(SOURCE).unwrap();
    let document = parsed.check().unwrap();
    let plots = vec![parsed
        .expand_checked_for_authoring(&document, "hello")
        .unwrap()];
    validate_checked_source(&plots, &selection.local_fore_adapters).unwrap();
    let mut missing = selection.local_fore_adapters.clone();
    missing[0]
        .ports
        .retain(|port| port.direction != PortDirection::Input);
    assert!(validate_checked_source(&plots, &missing).is_err());
    let mut wrong = selection.local_fore_adapters.clone();
    wrong[0].ports[0].value_kind = "kind/wrong".into();
    assert!(validate_checked_source(&plots, &wrong).is_err());
    // A valid proposed policy cannot cure malformed original Source bindings.
    assert!(verify_capture_and_fore(&selection, SOURCE, &wrong).is_err());
    let mut absent = selection.local_fore_adapters.clone();
    absent[0].entry = "absent".into();
    assert!(validate_checked_source(&plots, &absent).is_err());
    fs::remove_dir_all(state.parent().unwrap()).unwrap();
}

#[test]
fn empty_and_noop_same_release_refuse_and_json_request_rejects_malformed_changes() {
    let (state, installation, selection, mut request) = fixture();
    let original = serde_json::to_vec(&selection).unwrap();
    request.local_fore_queue_replacements.clear();
    assert!(candidate(&original, &request, &installation, &request.expected).is_err());
    let mut wire = serde_json::to_value(replacement(&selection, 0)).unwrap();
    for field in ["selected_item_capacity", "expected_byte_capacity"] {
        let mut bad = wire.clone();
        bad[field] = (-1).into();
        assert!(serde_json::from_value::<LocalForeQueueReplacement>(bad).is_err());
    }
    wire["authority"] = true.into();
    assert!(serde_json::from_value::<LocalForeQueueReplacement>(wire).is_err());
    fs::remove_dir_all(state.parent().unwrap()).unwrap();
}

#[test]
fn independent_boundaries_change_explicitly_without_touching_other_policy() {
    let (state, _, selection, _) = fixture();
    let first = replacement(&selection, 0);
    let mut second = replacement(&selection, 1);
    second.selected_item_capacity = 2;
    second.selected_byte_capacity = 512;
    let mut new = selection.clone();
    replace_queues(&mut new, &[first, second]).unwrap();
    let mut expected = json(&selection);
    expected["local_fore_adapters"][0]["ports"][0]["byte_capacity"] = 651.into();
    expected["local_fore_adapters"][0]["ports"][1]["byte_capacity"] = 512.into();
    expected["local_fore_adapters"][0]["ports"][1]["item_capacity"] = 2.into();
    assert_eq!(json(&new), expected);
    fs::remove_dir_all(state.parent().unwrap()).unwrap();
}

#[test]
fn legacy_wire_defaults_empty_and_stale_policy_digest_refuses() {
    let (state, installation, selection, mut request) = fixture();
    let wire = serde_json::json!({
        "original_policy_sha256":request.original_policy_sha256,
        "previous_release_bundle_sha256":request.previous_release_bundle_sha256,
        "installed_release_bundle_sha256":request.installed_release_bundle_sha256,
        "expected":request.expected,"retire_actions":false
    });
    let legacy: Request = serde_json::from_value(wire).unwrap();
    assert!(legacy.local_fore_queue_replacements.is_empty());
    request.original_policy_sha256 = "stale".into();
    assert!(candidate(
        &serde_json::to_vec(&selection).unwrap(),
        &request,
        &installation,
        &request.expected
    )
    .is_err());
    fs::remove_dir_all(state.parent().unwrap()).unwrap();
}
