use super::*;
use std::os::unix::fs::PermissionsExt;
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(0);
fn state() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "conduit-source-execution-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&path).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    path
}
fn pending(state: &Path, bytes: &[u8]) {
    let directory = inbox(state).unwrap();
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(directory.join(PENDING))
        .unwrap();
    file.write_all(bytes).unwrap();
    file.sync_all().unwrap();
}
#[test]
fn claim_is_once_even_for_invalid_request_and_restart() {
    let state = state();
    pending(&state, b"invalid request");
    assert_eq!(claim(&state).unwrap().unwrap(), b"invalid request");
    assert!(claim(&state).unwrap().is_none());
    assert!(state.join(INBOX).join(CLAIMED).exists());
    assert!(!state.join(INBOX).join(PENDING).exists());
    // A new pending attempt cannot replace a crash-retained claim.
    pending(&state, b"replacement");
    assert!(claim(&state).unwrap().is_none());
    assert_eq!(
        std::fs::read(state.join(INBOX).join(CLAIMED)).unwrap(),
        b"invalid request"
    );
    std::fs::remove_dir_all(state).unwrap();
}
#[test]
fn no_pending_request_has_no_effect() {
    let state = state();
    assert!(claim(&state).unwrap().is_none());
    assert!(!state.join(INBOX).join(CLAIMED).exists());
    std::fs::remove_dir_all(state).unwrap();
}
#[test]
fn refuses_symlink_and_public_pending_without_claiming() {
    let state = state();
    let directory = inbox(&state).unwrap();
    let outside = state.join("outside");
    std::fs::write(&outside, b"not selected").unwrap();
    std::os::unix::fs::symlink(&outside, directory.join(PENDING)).unwrap();
    assert!(claim(&state).is_err());
    std::fs::remove_file(directory.join(PENDING)).unwrap();
    std::fs::write(directory.join(PENDING), b"public").unwrap();
    std::fs::set_permissions(
        directory.join(PENDING),
        std::fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    assert!(claim(&state).is_err());
    assert!(!directory.join(CLAIMED).exists());
    std::fs::remove_dir_all(state).unwrap();
}
#[test]
fn rejects_public_or_symlink_inbox() {
    let state = state();
    let outside = state.join("other");
    std::fs::create_dir(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, state.join(INBOX)).unwrap();
    assert!(inbox(&state).is_err());
    std::fs::remove_file(state.join(INBOX)).unwrap();
    std::fs::set_permissions(&state, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(inbox(&state).is_err());
    std::fs::remove_dir_all(state).unwrap();
}

// Pure scope/selection tests. This private test basis is NOT a production
// constructor: the public runtime entrance requires original retained guards.
fn scoped_fixture() -> (VerifiedExecutionBasis, Request) {
    let text = include_str!("../../../../plots/hello/main.conduit");
    let source = crate::plot_source::parse(text).unwrap();
    let document = source.check().unwrap();
    let plot = source
        .expand_checked_for_authoring(&document, "hello")
        .unwrap();
    let body = conduit_body::Body::born(
        plot.expanded.source_document_id,
        plot.expanded.checked_plot_id,
        1,
        "sign/execution/born".into(),
    )
    .unwrap();
    let expected = BodyBiographyEvidence::born(
        body.clone(),
        conduit_body::BodyMembership::new(body.body_id.clone()).unwrap(),
        "Test".into(),
    )
    .unwrap();
    let policy: Selection = serde_json::from_value(serde_json::json!({
        "host_id":"root", "configuration_id":"configuration/root",
        "release_bundle_sha256":format!("sha256:{}", "1".repeat(64)),
        "expected":expected, "file":"/checked/source.conduit",
        "snapshot":{"owner":"execution-test", "generation":1,
            "sha256":format!("{:x}", Sha256::digest(text.as_bytes())),
            "source_document_id":conduit_plot::canonical_source_document_id(text).0},
        "entries":["hello"], "maximum_source_bytes":65536,
        "capture_preparation_bytes":131072, "capture_retained_bytes":65536,
        "caller_workspace_bytes":1048576
    }))
    .unwrap();
    let image: ImageProjectionSelection = serde_json::from_value(serde_json::json!({
        "maximum_nodes":1,"maximum_cords":1,"maximum_routes":1,"maximum_route_targets":1,
        "maximum_host_calls":1,"maximum_resources":1,"maximum_sign_expectations":1,
        "maximum_configuration_entries":1,"maximum_ports_per_node":1,"maximum_remote_endpoints":1,
        "maximum_cord_value_slots":1,"maximum_cord_value_bytes":1,"maximum_sign_items":1,"maximum_sign_bytes":1
    })).unwrap();
    let play = UsbPlaySelection {
        direct_fore_carrier: None,
        carrier: Default::default(),
        port: "/selected/usb".into(),
        member_basis: "/selected/member.json".into(),
        report_output: "/selected/report.json".into(),
        wait_milliseconds: 1000,
        image,
        maximum_program_bytes: 1,
        maximum_total_program_bytes: 1,
        assigned_encoded_bytes: 1,
        assigned_runtime_state_bytes: 1,
        assigned_counts: [0; conduit_core::ASSIGNED_PLAN_COUNT_KINDS],
        maximum_fore_ports: 0,
        maximum_document_bytes: 1,
    };
    let basis = VerifiedExecutionBasis {
        digest: format!("sha256:{}", "2".repeat(64)),
        root_host: "root".into(),
        root_boot: "boot/current".into(),
        root_generation: 7,
        current: expected.clone(),
        policy,
    };
    let request = Request {
        schema: SCHEMA.into(),
        policy_sha256: basis.digest.clone(),
        root_host: basis.root_host.clone(),
        root_boot: basis.root_boot.clone(),
        root_offer_generation: basis.root_generation,
        expected,
        release_bundle_sha256: basis.policy.release_bundle_sha256.clone(),
        configuration_id: basis.policy.configuration_id.clone(),
        snapshot: basis.policy.snapshot.clone(),
        entries: basis.policy.entries.clone(),
        expires_at_millis: 2000,
        plays: vec![play],
        returns: vec![],
        record_read_probe: None,
    };
    (basis, request)
}
#[test]
fn exact_scope_preserves_original_preparation_without_source_commit() {
    let (basis, request) = scoped_fixture();
    let before = serde_json::to_value(&basis.policy).unwrap();
    let selection = selected(request, &basis, 1000).unwrap();
    assert_eq!(selection.snapshot, basis.policy.snapshot);
    assert_eq!(
        selection.caller_workspace_bytes,
        basis.policy.caller_workspace_bytes
    );
    assert_eq!(
        selection.capture_preparation_bytes,
        basis.policy.capture_preparation_bytes
    );
    assert_eq!(
        selection.capture_retained_bytes,
        basis.policy.capture_retained_bytes
    );
    assert!(selection.admissions.is_empty());
    assert!(selection.continuation_outputs.is_empty());
    assert_eq!(serde_json::to_value(&basis.policy).unwrap(), before);
}
#[test]
fn refuses_copied_or_stale_root_policy_body_snapshot_and_entries() {
    let (basis, request) = scoped_fixture();
    let value = serde_json::to_value(request).unwrap();
    for (field, replacement) in [
        ("root_host", serde_json::json!("another")),
        ("root_boot", serde_json::json!("boot/old")),
        ("root_offer_generation", serde_json::json!(6)),
        (
            "policy_sha256",
            serde_json::json!(format!("sha256:{}", "3".repeat(64))),
        ),
        ("configuration_id", serde_json::json!("configuration/old")),
        (
            "release_bundle_sha256",
            serde_json::json!(format!("sha256:{}", "4".repeat(64))),
        ),
        ("entries", serde_json::json!(["other"])),
    ] {
        let mut changed = value.clone();
        changed[field] = replacement;
        assert!(
            selected(serde_json::from_value(changed).unwrap(), &basis, 1000).is_err(),
            "{field}"
        );
    }
    let mut changed = value.clone();
    changed["snapshot"]["generation"] = serde_json::json!(2);
    assert!(selected(serde_json::from_value(changed).unwrap(), &basis, 1000).is_err());
    let (other, _) = scoped_fixture();
    let mut request: Request = serde_json::from_value(value).unwrap();
    request.expected.friendly_name = "not current biography".into();
    assert!(selected(request, &other, 1000).is_err());
}
#[test]
fn finite_expiry_refuses_expired_far_future_and_empty_execution() {
    let (basis, request) = scoped_fixture();
    let value = serde_json::to_value(request).unwrap();
    for expires in [1000, 999, MAX_LIFETIME_MS + 1001, u64::MAX] {
        let mut changed = value.clone();
        changed["expires_at_millis"] = serde_json::json!(expires);
        assert!(selected(serde_json::from_value(changed).unwrap(), &basis, 1000).is_err());
    }
    let mut changed = value;
    changed["plays"] = serde_json::json!([]);
    assert!(selected(serde_json::from_value(changed).unwrap(), &basis, 1000).is_err());
}

#[test]
fn enqueue_is_private_no_clobber_and_never_adopts_source() {
    let state = state();
    let (_, mut request) = scoped_fixture();
    request.expires_at_millis = current_time_millis().unwrap() + 100_000;
    let selected = state.join("reviewed.json");
    std::fs::write(&selected, serde_json::to_vec(&request).unwrap()).unwrap();
    let source_policy = state.join(SELECTION_FILE);
    std::fs::write(&source_policy, b"original policy remains untouched").unwrap();
    submit(&state, &selected).unwrap();
    let pending_path = state.join(INBOX).join(PENDING);
    assert_eq!(
        std::fs::metadata(&pending_path).unwrap().mode() & 0o777,
        0o600
    );
    let bytes = std::fs::read(&pending_path).unwrap();
    assert_eq!(bytes, std::fs::read(&selected).unwrap());
    assert!(submit(&state, &selected).is_err());
    assert_eq!(std::fs::read(&pending_path).unwrap(), bytes);
    assert_eq!(claim(&state).unwrap().unwrap(), bytes);
    assert!(submit(&state, &selected).is_err());
    assert_eq!(
        std::fs::read(&source_policy).unwrap(),
        b"original policy remains untouched"
    );
    std::fs::remove_dir_all(state).unwrap();
}

#[test]
fn original_policy_fingerprint_covers_funding_and_excludes_one_shots() {
    let (basis, request) = scoped_fixture();
    let original = basis.policy.execution_policy_fingerprint().unwrap();
    let value = serde_json::to_value(&basis.policy).unwrap();
    for (field, replacement) in [
        ("host_id", serde_json::json!("another-root")),
        ("configuration_id", serde_json::json!("another-config")),
        (
            "release_bundle_sha256",
            serde_json::json!("another-release"),
        ),
        ("file", serde_json::json!("/other/source.conduit")),
        ("maximum_source_bytes", serde_json::json!(65535)),
        ("capture_preparation_bytes", serde_json::json!(131073)),
        ("capture_retained_bytes", serde_json::json!(65535)),
        ("caller_workspace_bytes", serde_json::json!(1048577)),
        ("entries", serde_json::json!(["another"])),
    ] {
        let mut changed = value.clone();
        changed[field] = replacement;
        let changed: Selection = serde_json::from_value(changed).unwrap();
        assert_ne!(
            changed.execution_policy_fingerprint().unwrap(),
            original,
            "{field}"
        );
    }
    let mut changed = basis.policy;
    changed.expected.friendly_name = "later biography".into();
    changed.plays = request.plays;
    assert_eq!(changed.execution_policy_fingerprint().unwrap(), original);
}
