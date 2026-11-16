use super::*;
use std::os::unix::fs::PermissionsExt;
#[test]
fn recovered_claim_skips_old_policy_but_absent_claim_keeps_normal_validation() {
    let root = std::env::temp_dir().join(super::super::fresh_identity("held-startup", "test"));
    fs::create_dir_all(&root).unwrap();
    let policy = root.join("source-check-selection.json");
    let malformed = b"invalid old Source policy -- must remain untouched";
    fs::write(&policy, malformed).unwrap();
    assert!(source_selection_for_startup(&root).is_err());
    let directory = root.join("source-execution");
    fs::create_dir(&directory).unwrap();
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
    let claim = directory.join("claimed.json");
    fs::write(&claim, b"opaque unresolved operation").unwrap();
    fs::set_permissions(&claim, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(source_selection_for_startup(&root).unwrap().is_none());
    assert_eq!(fs::read(&policy).unwrap(), malformed);
    // Unsafe marker cannot turn malformed policy into a permissive startup.
    fs::set_permissions(&claim, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(source_selection_for_startup(&root).is_err());
    fs::remove_file(&claim).unwrap();
    assert!(source_selection_for_startup(&root).is_err());
    fs::remove_dir_all(root).unwrap();
}
