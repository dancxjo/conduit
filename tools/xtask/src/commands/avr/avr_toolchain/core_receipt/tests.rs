use super::*;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "conduit-avr-core-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        for (vendor, version) in [
            ("arduino", ARDUINO_AVR_VERSION),
            ("SparkFun", SPARKFUN_AVR_VERSION),
        ] {
            let path = root
                .join(PACKAGES)
                .join(vendor)
                .join("hardware/avr")
                .join(version);
            fs::create_dir_all(&path).unwrap();
            // SparkFun's platform.txt version is not the package release version.
            fs::write(path.join("platform.txt"), "name=fixture\nversion=1.6.19\n").unwrap();
        }
        let gcc = avr_gcc_bin(&root);
        fs::create_dir_all(&gcc).unwrap();
        fs::write(
            gcc.join("avr-gcc"),
            b"fixture compiler bytes, never executed",
        )
        .unwrap();
        Self(root)
    }
    fn record(&self) {
        fs::write(
            self.0.join(RECEIPT),
            serde_json::to_vec(&snapshot(&self.0).unwrap()).unwrap(),
        )
        .unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
#[test]
fn content_receipt_is_stable_and_unreceipted_install_requires_provisioning() {
    let f = Fixture::new();
    assert!(!verify(&f.0).unwrap());
    let before = snapshot(&f.0).unwrap();
    f.record();
    assert_eq!(before, snapshot(&f.0).unwrap());
    assert!(before.files.keys().any(|p| p.ends_with("avr-gcc")));
}
#[test]
fn altered_or_extra_cached_content_is_refused_before_any_compiler_execution() {
    let f = Fixture::new();
    f.record();
    fs::write(avr_gcc_bin(&f.0).join("avr-gcc"), b"changed compiler").unwrap();
    assert!(verify(&f.0)
        .unwrap_err()
        .to_string()
        .contains("content changed"));
    f.record();
    fs::write(avr_gcc_bin(&f.0).join("extra-tool"), b"extra").unwrap();
    assert!(verify(&f.0)
        .unwrap_err()
        .to_string()
        .contains("content changed"));
}
#[test]
fn installation_versions_are_package_versions_and_wrong_or_duplicate_entries_refuse() {
    let valid = serde_json::json!({"platforms": [
        {"id":"arduino:avr","installed_version":ARDUINO_AVR_VERSION},
        {"id":"SparkFun:avr","installed_version":SPARKFUN_AVR_VERSION}
    ]});
    verify_versions(&valid).unwrap();
    let mut wrong = valid.clone();
    wrong["platforms"][1]["installed_version"] = "1.6.19".into();
    assert!(verify_versions(&wrong).is_err());
    let mut duplicate = valid.clone();
    duplicate["platforms"]
        .as_array_mut()
        .unwrap()
        .push(valid["platforms"][0].clone());
    assert!(verify_versions(&duplicate).is_err());
    assert!(verify_versions(&serde_json::json!({"platforms":[]})).is_err());
}
#[test]
fn forged_receipt_version_or_missing_core_refuses() {
    let f = Fixture::new();
    let mut receipt = snapshot(&f.0).unwrap();
    receipt.arduino = "other".into();
    fs::write(f.0.join(RECEIPT), serde_json::to_vec(&receipt).unwrap()).unwrap();
    assert!(verify(&f.0).is_err());
    fs::remove_dir_all(f.0.join(PACKAGES).join("SparkFun")).unwrap();
    assert!(verify(&f.0).is_err());
}
#[cfg(unix)]
#[test]
fn escaping_symlink_refuses_and_internal_file_link_is_recorded() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    let compiler = avr_gcc_bin(&f.0).join("avr-gcc");
    let alias = avr_gcc_bin(&f.0).join("cc");
    symlink("avr-gcc", &alias).unwrap();
    assert!(snapshot(&f.0)
        .unwrap()
        .files
        .values()
        .any(|entry| entry.symlink.as_deref() == Some("avr-gcc")));
    fs::remove_file(&alias).unwrap();
    symlink(&f.0, &alias).unwrap();
    assert!(snapshot(&f.0).is_err());
    assert!(compiler.is_file());
}
