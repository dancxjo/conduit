//! A real denied read stays unverified and recovers the same retained Body.
use super::super::super::super::{resume_service, state};
use super::*;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

struct RestorePermissions(PathBuf, std::fs::Permissions);
impl Drop for RestorePermissions {
    fn drop(&mut self) {
        std::fs::set_permissions(&self.0, self.1.clone()).expect("restore fixture permissions");
    }
}

#[test]
fn denied_selected_read_retains_finite_refusal_and_body_before_explicit_recovery() {
    let (state_root, checkpoint_root, body_id) = failed_large_todo_read_fixture();
    let selector = std::fs::read_dir(&checkpoint_root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.extension().is_some_and(|ext| ext == "current"))
        .unwrap();
    let guard = RestorePermissions(
        selector.clone(),
        std::fs::metadata(&selector).unwrap().permissions(),
    );
    std::fs::set_permissions(&selector, std::fs::Permissions::from_mode(0o000)).unwrap();
    assert_eq!(
        std::fs::File::open(&selector).unwrap_err().kind(),
        std::io::ErrorKind::PermissionDenied
    );
    let failed = resume_service(resumed_todo_host(&state_root), &state_root)
        .err()
        .unwrap();
    assert_eq!(failed, "todo-committed-inaccessible");
    let retained = state::execution(&state_root).unwrap().unwrap();
    assert_eq!(retained["verified"], false);
    assert_eq!(retained["refusal"], "todo-committed-inaccessible");
    assert!(retained["restored_fore_sha256"].is_null());
    assert_eq!(
        retained["read_kernel_failure"],
        serde_json::json!({"code":"host_call_failed", "detail":10})
    );
    let biography = state::load(&state_root).unwrap().unwrap();
    assert_eq!(biography.body_id.as_str(), body_id);
    assert_eq!(biography.body.state, conduit_body::BodyState::Lulled);
    drop(guard);
    let recovered = resume_service(
        resumed_todo_host_on_boot(&state_root, "boot/todo-permission-restored"),
        &state_root,
    )
    .unwrap();
    assert_eq!(recovered.session.evidence().body_id.as_str(), body_id);
    assert!(recovered.has_verified_todo());
    assert!(recovered
        .local_face_snapshot()
        .unwrap()
        .subjects
        .iter()
        .any(|subject| subject.name == "I".repeat(23)));
    std::fs::remove_dir_all(state_root).unwrap();
}

struct RestoreSelector(PathBuf, PathBuf);
impl Drop for RestoreSelector {
    fn drop(&mut self) {
        if self.0.is_dir() {
            std::fs::remove_dir(&self.0).expect("remove temporary selector directory");
        }
        std::fs::rename(&self.1, &self.0).expect("restore selected checkpoint file");
    }
}

#[test]
fn temporary_read_storage_error_stays_unverified_then_recovers_same_body() {
    let (state_root, checkpoint_root, body_id) = failed_large_todo_read_fixture();
    let selector = std::fs::read_dir(&checkpoint_root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.extension().is_some_and(|ext| ext == "current"))
        .unwrap();
    let backup = selector.with_extension("saved-selector");
    std::fs::rename(&selector, &backup).unwrap();
    let guard = RestoreSelector(selector.clone(), backup);
    std::fs::create_dir(&selector).unwrap();
    assert_eq!(
        std::fs::read(&selector).unwrap_err().kind(),
        std::io::ErrorKind::IsADirectory
    );
    let failed = resume_service(
        resumed_todo_host_on_boot(&state_root, "boot/todo-storage-failed"),
        &state_root,
    )
    .err()
    .unwrap();
    assert_eq!(failed, "todo-committed-storage-unavailable");
    let retained = state::execution(&state_root).unwrap().unwrap();
    assert_eq!(retained["verified"], false);
    assert_eq!(retained["refusal"], "todo-committed-storage-unavailable");
    assert!(retained["restored_fore_sha256"].is_null());
    assert_eq!(
        retained["read_kernel_failure"],
        serde_json::json!({"code":"host_call_failed", "detail":8})
    );
    assert_eq!(
        state::load(&state_root).unwrap().unwrap().body_id.as_str(),
        body_id
    );
    drop(guard);
    let recovered = resume_service(
        resumed_todo_host_on_boot(&state_root, "boot/todo-storage-restored"),
        &state_root,
    )
    .unwrap();
    assert_eq!(recovered.session.evidence().body_id.as_str(), body_id);
    assert!(recovered.has_verified_todo());
    std::fs::remove_dir_all(state_root).unwrap();
}
