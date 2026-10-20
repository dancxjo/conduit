use super::*;
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "conduit-ia32-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn artifact(name: &str, bytes: &[u8]) -> Artifact {
    Artifact {
        name: name.into(),
        bytes: bytes.len() as u64,
        sha256: format!("{:x}", Sha256::digest(bytes)),
    }
}
fn pin() -> Manifest {
    Manifest {
        identity: "fixture-only".into(),
        package: Package {
            url: "https://invalid.test/fixture.deb".into(),
            bytes: 0,
            sha256: String::new(),
        },
        files: vec![artifact("CODE", b"code"), artifact("VARS", b"vars")],
    }
}
fn install(root: &Path) {
    fs::create_dir_all(root).unwrap();
    fs::write(root.join("CODE"), b"code").unwrap();
    fs::write(root.join("VARS"), b"vars").unwrap();
}

#[test]
fn installed_matching_pair_needs_no_package_or_download() {
    let f = Fixture::new();
    let installed = f.0.join("installed");
    install(&installed);
    prepare_with(&f.0, &installed, &pin(), |_, _| {
        panic!("installed pair must avoid download")
    })
    .unwrap();
    assert!(!cache(&f.0).exists());
}
#[test]
fn verified_offline_cache_is_idempotent_and_concurrent() {
    let f = Fixture::new();
    install(&cache(&f.0));
    std::thread::scope(|scope| {
        for _ in 0..4 {
            let root = &f.0;
            scope.spawn(move || {
                prepare_with(root, &root.join("missing"), &pin(), |_, _| {
                    panic!("offline cache must avoid download")
                })
                .unwrap()
            });
        }
    });
    pair(&cache(&f.0), &pin()).unwrap();
}
#[test]
fn missing_corrupt_truncated_and_mixed_cache_refuse_without_replacing() {
    let f = Fixture::new();
    let root = cache(&f.0);
    install(&root);
    for bytes in [b"bad!".as_slice(), b"cod".as_slice()] {
        fs::write(root.join("CODE"), bytes).unwrap();
        assert!(
            prepare_with(&f.0, &f.0.join("missing"), &pin(), |_, _| panic!(
                "bad cache must refuse"
            ))
            .is_err()
        );
        assert_eq!(fs::read(root.join("CODE")).unwrap(), bytes);
    }
    fs::remove_file(root.join("CODE")).unwrap();
    assert!(pair(&root, &pin()).is_err());
    assert!(resolve(&f.0, &f.0.join("missing")).is_err());
}
#[test]
fn downloaded_wrong_hash_or_length_never_installs_and_cleans_stage() {
    let f = Fixture::new();
    for bytes in [b"".as_slice(), b"unverified".as_slice()] {
        assert!(prepare_with(&f.0, &f.0.join("missing"), &pin(), |_, path| {
            fs::write(path, bytes)?;
            Ok(())
        })
        .is_err());
        assert!(!cache(&f.0).exists());
        assert_eq!(
            fs::read_dir(f.0.join("target/conduitos/toolchain"))
                .unwrap()
                .count(),
            1,
            "only persistent lock remains"
        );
    }
}
#[test]
fn readonly_template_becomes_writable_private_vars_without_template_mutation() {
    let f = Fixture::new();
    let template = f.0.join("template");
    let output = f.0.join("run-vars");
    fs::write(&template, b"vars").unwrap();
    let mut permissions = fs::metadata(&template).unwrap().permissions();
    permissions.set_readonly(true);
    fs::set_permissions(&template, permissions).unwrap();
    copy_vars(&template, &output).unwrap();
    fs::write(output, b"changed").unwrap();
    assert_eq!(fs::read(template).unwrap(), b"vars");
}

#[cfg(target_os = "linux")]
fn package(f: &Fixture, include_vars: bool) -> (Manifest, PathBuf) {
    let source = f.0.join("package");
    fs::create_dir_all(source.join("DEBIAN")).unwrap();
    fs::write(source.join("DEBIAN/control"), "Package: conduit-firmware-test\nVersion: 1\nArchitecture: all\nMaintainer: Test <test@example.invalid>\nDescription: fixture\n").unwrap();
    install(&source.join("usr/share/OVMF"));
    if !include_vars {
        fs::remove_file(source.join("usr/share/OVMF/VARS")).unwrap();
    }
    fs::create_dir_all(source.join("usr/share/doc/ovmf-ia32")).unwrap();
    fs::write(source.join("usr/share/doc/ovmf-ia32/copyright"), "fixture").unwrap();
    let output = f.0.join("fixture.deb");
    assert!(Command::new("dpkg-deb")
        .args(["--build", "--root-owner-group"])
        .arg(source)
        .arg(&output)
        .status()
        .unwrap()
        .success());
    let data = fs::read(&output).unwrap();
    let mut pin = pin();
    pin.package.bytes = data.len() as u64;
    pin.package.sha256 = format!("{:x}", Sha256::digest(data));
    (pin, output)
}
#[cfg(target_os = "linux")]
#[test]
fn package_preparation_is_atomic_and_concurrent_download_occurs_once() {
    let f = Fixture::new();
    let (pin, package) = package(&f, true);
    let fetches = AtomicU64::new(0);
    std::thread::scope(|scope| {
        for _ in 0..4 {
            let (root, pin, package, fetches) = (&f.0, &pin, &package, &fetches);
            scope.spawn(move || {
                prepare_with(
                    root,
                    &root.join("no-installed-package"),
                    pin,
                    |_, output| {
                        fetches.fetch_add(1, Ordering::Relaxed);
                        fs::copy(package, output)?;
                        Ok(())
                    },
                )
                .unwrap();
            });
        }
    });
    assert_eq!(fetches.load(Ordering::Relaxed), 1);
    let (code, vars) = pair(&cache(&f.0), &pin).unwrap();
    assert!(fs::metadata(code).unwrap().permissions().readonly());
    assert!(fs::metadata(vars).unwrap().permissions().readonly());
    assert!(cache(&f.0).join("copyright").is_file());
}
#[cfg(target_os = "linux")]
#[test]
fn authenticated_package_with_missing_firmware_never_publishes_partial_pair() {
    let f = Fixture::new();
    let (pin, package) = package(&f, false);
    assert!(prepare_with(&f.0, &f.0.join("missing"), &pin, |_, output| {
        fs::copy(package, output)?;
        Ok(())
    })
    .is_err());
    assert!(!cache(&f.0).exists());
}
