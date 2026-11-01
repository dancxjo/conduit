use super::*;
fn root() -> std::path::PathBuf {
    let path = std::env::temp_dir().join(crate::durable_host::fresh_identity(
        "root-signer-test",
        "private",
    ));
    fs::create_dir_all(&path).unwrap();
    path
}
#[test]
fn private_preparation_is_not_publication_and_exact_retry_keeps_identity() {
    let root = root();
    let first = prepare(&root, "body/one", "host/owner").unwrap();
    assert!(selected(&root, "body/one", "host/owner").unwrap().is_none());
    assert!(prepare(&root, "body/other", "host/owner").is_err());
    assert!(prepare(&root, "body/one", "host/other").is_err());
    assert!(prepare(&root, "body/one", "host/owner").unwrap() == first);
    commit(&root, &first, "body/one", "host/owner").unwrap();
    assert!(selected(&root, "body/one", "host/owner").unwrap() == Some(first.clone()));
    assert!(prepare(&root, "body/one", "host/owner").unwrap() == first);
    let public = serde_json::to_value(first).unwrap();
    assert!(public.get("seed").is_none());
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn selected_missing_or_changed_private_custody_never_rekeys() {
    let root = root();
    let binding = prepare(&root, "body/one", "host/owner").unwrap();
    commit(&root, &binding, "body/one", "host/owner").unwrap();
    let bytes = fs::read(root.join(PRIVATE)).unwrap();
    fs::remove_file(root.join(PRIVATE)).unwrap();
    assert!(selected(&root, "body/one", "host/owner").is_err());
    assert!(prepare(&root, "body/one", "host/owner").is_err());
    assert!(!root.join(PRIVATE).exists());
    let mut replacement: Private = serde_json::from_slice(&bytes).unwrap();
    replacement.seed = [13; 32];
    replacement.public_key = SigningKey::from_bytes(&replacement.seed)
        .verifying_key()
        .to_bytes();
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(root.join(PRIVATE))
        .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .unwrap();
    }
    file.write_all(&serde_json::to_vec(&replacement).unwrap())
        .unwrap();
    assert!(selected(&root, "body/one", "host/owner").is_err());
    assert!(prepare(&root, "body/one", "host/owner").is_err());
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn partial_or_exposed_or_symlink_private_records_refuse_without_overwrite() {
    let root = root();
    let binding = prepare(&root, "body/one", "host/owner").unwrap();
    fs::write(root.join(PRIVATE), b"interrupted").unwrap();
    assert!(prepare(&root, "body/one", "host/owner").is_err());
    assert_eq!(fs::read(root.join(PRIVATE)).unwrap(), b"interrupted");
    assert!(commit(&root, &binding, "body/one", "host/owner").is_err());
    fs::remove_file(root.join(PRIVATE)).unwrap();
    prepare(&root, "body/one", "host/owner").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::{symlink, PermissionsExt};
        fs::set_permissions(root.join(PRIVATE), fs::Permissions::from_mode(0o644)).unwrap();
        assert!(prepare(&root, "body/one", "host/owner").is_err());
        fs::rename(root.join(PRIVATE), root.join("body/real-private")).unwrap();
        symlink(root.join("body/real-private"), root.join(PRIVATE)).unwrap();
        assert!(prepare(&root, "body/one", "host/owner").is_err());
    }
    fs::remove_dir_all(root).unwrap();
}
