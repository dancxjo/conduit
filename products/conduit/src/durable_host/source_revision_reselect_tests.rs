use super::*;
const SOURCE: &str = include_str!("../../../../plots/hello/main.conduit");

fn fixture() -> (std::path::PathBuf, Installation, Selection, Request) {
    let (manifest, state) = super::super::super::tests::fixture();
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
    let source = crate::plot_source::parse(SOURCE).unwrap();
    let document = source.check().unwrap();
    let plot = source
        .expand_checked_for_authoring(&document, "hello")
        .unwrap();
    let body = conduit_body::Body::born(
        plot.expanded.source_document_id,
        plot.expanded.checked_plot_id,
        1,
        "sign/reselection-test/born".into(),
    )
    .unwrap();
    let expected = BodyBiographyEvidence::born(
        body.clone(),
        conduit_body::BodyMembership::new(body.body_id.clone()).unwrap(),
        "Reselection test".into(),
    )
    .unwrap();
    let biography = serde_json::to_vec_pretty(&expected).unwrap();
    let biography_path = state.join("biography.json");
    fs::write(&biography_path, &biography).unwrap();
    installation.body_state = Some(super::super::super::BodyBinding {
        body_id: body.body_id.as_str().into(),
        biography_sha256: digest(&biography),
        biography_path: biography_path.display().to_string(),
    });
    let selection: Selection = serde_json::from_value(serde_json::json!({
        "host_id": installation.host_id,
        "configuration_id": configuration.configuration_id(),
        "release_bundle_sha256": format!("sha256:{}", "1".repeat(64)),
        "expected": expected,
        "file": state.join("source.conduit"),
        "snapshot": {"owner":"reselection-test", "generation":1,
            "sha256":format!("{:x}", Sha256::digest(SOURCE.as_bytes())),
            "source_document_id":conduit_plot::canonical_source_document_id(SOURCE).0},
        "entries":["hello"], "maximum_source_bytes":65536,
        "capture_preparation_bytes":131072, "capture_retained_bytes":65536,
        "caller_workspace_bytes":1048576,
    }))
    .unwrap();
    fs::write(&selection.file, SOURCE).unwrap();
    let original = serde_json::to_vec_pretty(&selection).unwrap();
    fs::write(state.join(SELECTION_FILE), &original).unwrap();
    write_json_atomic(&state.join("installation.json"), &installation).unwrap();
    let request = Request {
        original_policy_sha256: digest(&original),
        previous_release_bundle_sha256: selection.release_bundle_sha256.clone(),
        installed_release_bundle_sha256: installation.release_bundle_sha256.clone(),
        expected,
        retire_actions: false,
    };
    (state, installation, selection, request)
}

#[test]
fn reselection_preserves_bounds_file_compiler_policy_and_committed_body() {
    let (state, installation, selection, request) = fixture();
    let original = fs::read(state.join(SELECTION_FILE)).unwrap();
    let (new, retired) = candidate(&original, &request, &installation, &request.expected).unwrap();
    verify_capture_and_workset(&new, SOURCE).unwrap();
    let mut before = serde_json::to_value(&selection).unwrap();
    before["release_bundle_sha256"] = serde_json::json!(installation.release_bundle_sha256);
    assert_eq!(serde_json::to_value(new).unwrap(), before);
    assert_eq!(
        retired.admissions + retired.returns + retired.plays + retired.continuation_outputs,
        0
    );
    assert_eq!(fs::read(state.join(SELECTION_FILE)).unwrap(), original);
    fs::remove_dir_all(state.parent().unwrap()).unwrap();
}

#[test]
fn reselection_refuses_changed_digest_release_host_configuration_and_body() {
    let (state, installation, selection, request) = fixture();
    let original = fs::read(state.join(SELECTION_FILE)).unwrap();
    for field in ["host_id", "configuration_id", "release_bundle_sha256"] {
        let mut value = serde_json::to_value(&selection).unwrap();
        value[field] = serde_json::json!("changed");
        let changed = serde_json::to_vec(&value).unwrap();
        let mut reviewed = Request {
            original_policy_sha256: digest(&changed),
            ..Request {
                original_policy_sha256: request.original_policy_sha256.clone(),
                previous_release_bundle_sha256: request.previous_release_bundle_sha256.clone(),
                installed_release_bundle_sha256: request.installed_release_bundle_sha256.clone(),
                expected: request.expected.clone(),
                retire_actions: false,
            }
        };
        assert!(candidate(&changed, &reviewed, &installation, &request.expected).is_err());
        reviewed.original_policy_sha256 = "stale".into();
        assert!(candidate(&original, &reviewed, &installation, &request.expected).is_err());
    }
    let mut changed_body = request.expected.clone();
    changed_body.friendly_name = "Changed biography".into();
    assert!(candidate(&original, &request, &installation, &changed_body).is_err());
    assert_eq!(fs::read(state.join(SELECTION_FILE)).unwrap(), original);
    fs::remove_dir_all(state.parent().unwrap()).unwrap();
}

#[test]
fn reselection_refuses_changed_file_snapshot_or_checked_entries() {
    let (state, _, mut selection, _) = fixture();
    assert!(verify_capture_and_workset(&selection, &format!("{SOURCE}\n# changed")).is_err());
    selection.entries = vec!["missing".into()];
    assert!(verify_capture_and_workset(&selection, SOURCE).is_err());
    fs::remove_dir_all(state.parent().unwrap()).unwrap();
}

#[test]
fn reselection_refuses_live_host_and_missing_policy_without_mutation() {
    let (state, _, _, _) = fixture();
    let original = fs::read(state.join(SELECTION_FILE)).unwrap();
    let lock = owner_lock::acquire(&state).unwrap();
    assert!(reselect(&state, Path::new("/does-not-exist"))
        .unwrap_err()
        .contains("owned"));
    assert_eq!(fs::read(state.join(SELECTION_FILE)).unwrap(), original);
    drop(lock);
    fs::remove_file(state.join(SELECTION_FILE)).unwrap();
    let request = state.join("request.json");
    fs::write(&request, b"{}").unwrap();
    assert!(reselect(&state, &request).is_err());
    assert!(!state.join(SELECTION_FILE).exists());
    fs::remove_dir_all(state.parent().unwrap()).unwrap();
}

#[test]
fn reselection_audit_is_private_exact_and_never_overwritten() {
    use std::os::unix::fs::PermissionsExt;
    let (state, _, _, _) = fixture();
    let original = fs::read(state.join(SELECTION_FILE)).unwrap();
    let audit = retain_original(&state, &original).unwrap();
    assert_eq!(retain_original(&state, &original).unwrap(), audit);
    assert_eq!(fs::read(&audit).unwrap(), original);
    assert_eq!(
        fs::metadata(&audit).unwrap().permissions().mode() & 0o777,
        0o400
    );
    assert_eq!(
        fs::metadata(audit.parent().unwrap())
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    fs::remove_dir_all(state.parent().unwrap()).unwrap();
}

#[test]
fn reselection_requires_consent_to_retire_and_does_not_replay_actions() {
    let (state, installation, mut selection, mut request) = fixture();
    selection.plays.push(
        serde_json::from_value(serde_json::json!({
            "port":"/dev/selected-test", "member_basis":"/private/basis.json",
            "report_output":"/private/report.json", "wait_milliseconds":1000,
            "maximum_program_bytes":100, "maximum_total_program_bytes":200,
            "assigned_encoded_bytes":100, "assigned_runtime_state_bytes":100,
            "assigned_counts":vec![1; conduit_core::ASSIGNED_PLAN_COUNT_KINDS],
            "maximum_fore_ports":1, "maximum_document_bytes":100,
            "image": {"maximum_nodes":1, "maximum_cords":1, "maximum_routes":1,
                "maximum_route_targets":1, "maximum_host_calls":1, "maximum_resources":1,
                "maximum_sign_expectations":1, "maximum_configuration_entries":1,
                "maximum_ports_per_node":1, "maximum_remote_endpoints":1,
                "maximum_cord_value_slots":1, "maximum_cord_value_bytes":100,
                "maximum_sign_items":1, "maximum_sign_bytes":100}
        }))
        .unwrap(),
    );
    let original = serde_json::to_vec(&selection).unwrap();
    request.original_policy_sha256 = digest(&original);
    assert!(
        candidate(&original, &request, &installation, &request.expected)
            .err()
            .is_some_and(|e| e.contains("explicit retirement"))
    );
    request.retire_actions = true;
    let (new, retired) = candidate(&original, &request, &installation, &request.expected).unwrap();
    assert_eq!(retired.plays, 1);
    assert!(new.plays.is_empty());
    assert!(!state.join("report.json").exists());
    fs::remove_dir_all(state.parent().unwrap()).unwrap();
}

#[test]
fn reselection_refuses_committed_workset_mismatch() {
    let (state, installation, mut selection, mut request) = fixture();
    let body = conduit_body::Body::born(
        "source/other".into(),
        "checked/other".into(),
        1,
        "sign/other".into(),
    )
    .unwrap();
    selection.expected = BodyBiographyEvidence::born(
        body.clone(),
        conduit_body::BodyMembership::new(body.body_id.clone()).unwrap(),
        "Other".into(),
    )
    .unwrap();
    let original = serde_json::to_vec(&selection).unwrap();
    request.original_policy_sha256 = digest(&original);
    assert!(candidate(&original, &request, &installation, &request.expected).is_err());
    selection.expected = request.expected.clone();
    selection.expected.body.workset = body.workset;
    assert!(verify_capture_and_workset(&selection, SOURCE)
        .unwrap_err()
        .contains("committed"));
    fs::remove_dir_all(state.parent().unwrap()).unwrap();
}

#[test]
fn reselection_commit_preserves_body_keys_compiler_and_failure_keeps_original() {
    let (state, installation, _, request) = fixture();
    let original = fs::read(state.join(SELECTION_FILE)).unwrap();
    let body = fs::read(state.join("biography.json")).unwrap();
    let installed = fs::read(state.join("installation.json")).unwrap();
    fs::write(
        state.join("control.token"),
        b"unchanged private control fixture",
    )
    .unwrap();
    fs::write(
        state.join("source-compiler-selection.json"),
        b"unchanged qualified compiler fixture",
    )
    .unwrap();
    let (selection, _) = candidate(&original, &request, &installation, &request.expected).unwrap();
    // Real atomic-writer failure, before its rename: audit is allowed, policy is not changed.
    let temporary = state.join(SELECTION_FILE).with_extension("tmp");
    fs::create_dir(&temporary).unwrap();
    assert!(commit_selection(&state, &original, &selection).is_err());
    assert_eq!(fs::read(state.join(SELECTION_FILE)).unwrap(), original);
    fs::remove_dir(temporary).unwrap();
    let (audit, replacement) = commit_selection(&state, &original, &selection).unwrap();
    assert_eq!(fs::read(audit).unwrap(), original);
    assert_eq!(fs::read(state.join(SELECTION_FILE)).unwrap(), replacement);
    assert_eq!(fs::read(state.join("biography.json")).unwrap(), body);
    assert_eq!(
        fs::read(state.join("installation.json")).unwrap(),
        installed
    );
    assert_eq!(
        fs::read(state.join("control.token")).unwrap(),
        b"unchanged private control fixture"
    );
    assert_eq!(
        fs::read(state.join("source-compiler-selection.json")).unwrap(),
        b"unchanged qualified compiler fixture"
    );
    fs::remove_dir_all(state.parent().unwrap()).unwrap();
}
