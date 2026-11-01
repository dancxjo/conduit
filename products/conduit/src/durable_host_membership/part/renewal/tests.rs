use super::*;
use std::os::unix::fs::{symlink, PermissionsExt};
struct Fixture {
    root: std::path::PathBuf,
    record: Record,
    next: MembershipCredential,
    original: Vec<u8>,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "native-renewal-{}",
            crate::birth_identity::fresh_uuid().unwrap()
        ));
        fs::create_dir_all(root.join("body")).unwrap();
        super::super::super::super::restrict_directory(&root).unwrap();
        let record = super::super::tests::fixture();
        let pending =
            serde_json::from_value(serde_json::to_value(&record.pending).unwrap()).unwrap();
        super::super::retain(
            &root,
            pending,
            record.receipt.clone(),
            record.receipt.credential.host_id.as_str(),
        )
        .unwrap();
        let original = fs::read(root.join("body/native-part-private.json")).unwrap();
        let path = root.join("body/membership-credential.json");
        super::super::super::super::write_json_atomic(&path, &record.receipt.credential).unwrap();
        let installation:Installation=serde_json::from_value(serde_json::json!({
   "schema":crate::durable_host::INSTALL_SCHEMA,"host_id":record.receipt.credential.host_id,
   "release_source_identity":"source/renewal-test","release_bundle_sha256":digest(b"renewal-fixture"),
   "product_executable":"unused","body_state":null,
   "joined_body_state":{"body_id":record.receipt.credential.body_id,"part_id":record.receipt.credential.part_id,
    "credential_path":path.display().to_string(),"credential_sha256":digest(&encoded(&record.receipt.credential).unwrap())}
  })).unwrap();
        super::super::super::super::write_json_atomic(
            &root.join("installation.json"),
            &installation,
        )
        .unwrap();
        fs::write(root.join("runtime.json"), b"untouched-current-boot").unwrap();
        let mut next = record.receipt.credential.clone();
        next.credential_id = serde_json::from_value(serde_json::json!("09".repeat(32))).unwrap();
        next.boot_id = "boot/renewed".into();
        next.issued_at_millis += 1;
        Self {
            root,
            record,
            next,
            original,
        }
    }
    fn installed(&self) -> (Installation, MembershipCredential) {
        let i = read_installation(&self.root.join("installation.json")).unwrap();
        super::super::super::validate(i.joined_body_state.as_ref().unwrap(), &i).unwrap();
        let c = serde_json::from_slice(
            &fs::read(&i.joined_body_state.as_ref().unwrap().credential_path).unwrap(),
        )
        .unwrap();
        (i, c)
    }
    fn commit(&self, stop: u8) -> Result<(), String> {
        transaction(
            &self.root,
            &self.record,
            &self.record.receipt.credential,
            &self.next,
            |at| {
                if at == stop {
                    Err("injected interruption".into())
                } else {
                    Ok(())
                }
            },
        )
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
#[test]
fn native_part_renewal_every_commit_boundary_reopens_and_retries_exactly() {
    for stop in 1..=4 {
        let f = Fixture::new();
        assert!(f.commit(stop).is_err());
        let (i, c) = f.installed();
        assert_eq!(
            c,
            if stop < 3 {
                f.record.receipt.credential.clone()
            } else {
                f.next.clone()
            }
        );
        reconcile(&f.root, &f.record, &i, &c).unwrap();
        f.commit(0).unwrap();
        f.commit(0).unwrap();
        let (i, c) = f.installed();
        assert_eq!(c, f.next);
        reconcile(&f.root, &f.record, &i, &c).unwrap();
        assert_eq!(
            fs::read(f.root.join("body/native-part-private.json")).unwrap(),
            f.original
        );
        assert_eq!(
            fs::read(f.root.join("runtime.json")).unwrap(),
            b"untouched-current-boot"
        );
        let r = read(&f.root, &f.record).unwrap();
        assert!(r.prepared.is_none());
        assert_eq!(r.current, f.next);
    }
}
#[test]
fn native_part_renewal_conflict_and_private_bounds_refuse_without_selection() {
    let f = Fixture::new();
    assert!(f.commit(1).is_err());
    let before = fs::read(f.root.join("installation.json")).unwrap();
    let mut other = f.next.clone();
    other.credential_id = serde_json::from_value(serde_json::json!("0a".repeat(32))).unwrap();
    assert!(transaction(
        &f.root,
        &f.record,
        &f.record.receipt.credential,
        &other,
        |_| Ok(())
    )
    .is_err());
    assert_eq!(fs::read(f.root.join("installation.json")).unwrap(), before);
    let path = f.root.join("body").join(FILE);
    let bytes = fs::read(&path).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(f.commit(0).is_err());
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    fs::remove_file(&path).unwrap();
    symlink("nonexistent", &path).unwrap();
    assert!(f.commit(0).is_err());
    fs::remove_file(&path).unwrap();
    fs::write(&path, vec![b' '; 65537]).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(f.commit(0).is_err());
    fs::write(&path, &bytes).unwrap();
    let public = public_path(&f.root, &f.next);
    symlink(f.root.join("installation.json"), &public).unwrap();
    assert!(f.commit(0).is_err());
    assert_eq!(fs::read(f.root.join("installation.json")).unwrap(), before);
}
#[test]
fn native_part_renewal_third_selected_value_and_private_anchor_refuse() {
    let f = Fixture::new();
    assert!(f.commit(1).is_err());
    let (mut i, _) = f.installed();
    let mut third = f.next.clone();
    third.credential_id = serde_json::from_value(serde_json::json!("0b".repeat(32))).unwrap();
    let p = f.root.join("body/foreign.json");
    super::super::super::super::write_json_atomic(&p, &third).unwrap();
    let b = i.joined_body_state.as_mut().unwrap();
    b.credential_path = p.display().to_string();
    b.credential_sha256 = digest(&encoded(&third).unwrap());
    assert!(reconcile(&f.root, &f.record, &i, &third).is_err());
    let path = f.root.join("body").join(FILE);
    let mut value: Renewal = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    value.original_sha256 = "00".repeat(32);
    write_private(&f.root, &value).unwrap();
    assert!(f.commit(0).is_err());
}
#[test]
fn native_part_renewal_lock_refuses_concurrent_transaction() {
    let f = Fixture::new();
    let lock = Lock::acquire(&f.root).unwrap();
    assert!(f.commit(0).is_err());
    drop(lock);
    f.commit(0).unwrap();
}

#[test]
fn native_part_renewal_installation_lock_child() {
    let Some(root) = std::env::var_os("CONDUIT_INSTALLATION_LOCK_TEST") else {
        return;
    };
    let _guard = Lock::acquire(Path::new(&root)).unwrap();
    println!("LOCKED");
    use std::io::Write;
    std::io::stdout().flush().unwrap();
    std::thread::sleep(std::time::Duration::from_secs(30));
}
#[test]
fn native_part_renewal_other_process_blocks_install_join_and_renewal() {
    use std::io::BufRead;
    use std::process::{Command, Stdio};
    let f = Fixture::new();
    let mut child=Command::new(std::env::current_exe().unwrap()).args(["--exact","durable_host::membership::part::renewal::tests::native_part_renewal_installation_lock_child","--nocapture"])
  .env("CONDUIT_INSTALLATION_LOCK_TEST",&f.root).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().unwrap();
    let mut out = std::io::BufReader::new(child.stdout.take().unwrap());
    let mut line = String::new();
    loop {
        line.clear();
        assert_ne!(out.read_line(&mut line).unwrap(), 0);
        if line.contains("LOCKED") {
            break;
        }
    }
    let before = fs::read(f.root.join("installation.json")).unwrap();
    let (manifest, unused_state) = crate::durable_host::tests::fixture();
    let install = crate::durable_host::install_configured_with_todo(
        &manifest,
        &f.root,
        crate::durable_host::selected_speech::Change::Preserve,
        crate::durable_host::selected_model::Change::Preserve,
        crate::durable_host::selected_todo::Change::Preserve,
    )
    .unwrap_err();
    assert!(
        install.contains("installation transaction already active"),
        "{install}"
    );
    let join =
        super::super::super::complete_body_join_document(f.record.receipt.clone(), &f.root, true)
            .unwrap_err();
    assert!(
        join.contains("installation transaction already active"),
        "{join}"
    );
    assert!(f
        .commit(0)
        .unwrap_err()
        .contains("installation transaction already active"));
    assert_eq!(fs::read(f.root.join("installation.json")).unwrap(), before);
    child.kill().unwrap();
    child.wait().unwrap();
    let _ = fs::remove_dir_all(manifest.parent().unwrap().parent().unwrap());
    let _ = fs::remove_dir_all(unused_state);
    f.commit(0).unwrap();
}
