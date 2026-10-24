use super::*;
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "conduit-riscv64-{}-{}",
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
        member: name.into(),
        bytes: bytes.len() as u64,
        sha256: format!("{:x}", Sha256::digest(bytes)),
    }
}
fn pin() -> Manifest {
    Manifest {
        identity: "fixture-only".into(),
        archive: Package {
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
fn verified_offline_cache_is_idempotent_and_concurrent() {
    let f = Fixture::new();
    install(&cache(&f.0));
    std::thread::scope(|scope| {
        for _ in 0..4 {
            let root = &f.0;
            scope.spawn(move || {
                prepare_with(root, &pin(), |_, _| {
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
        assert!(prepare_with(&f.0, &pin(), |_, _| panic!("bad cache must refuse")).is_err());
        assert_eq!(fs::read(root.join("CODE")).unwrap(), bytes);
    }
    fs::remove_file(root.join("CODE")).unwrap();
    assert!(pair(&root, &pin()).is_err());
}
#[test]
fn downloaded_wrong_hash_or_length_never_installs_and_cleans_stage() {
    let f = Fixture::new();
    for bytes in [b"".as_slice(), b"unverified".as_slice()] {
        assert!(prepare_with(&f.0, &pin(), |_, path| {
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
fn actual_tar_members_are_verified_before_publish() {
    let f = Fixture::new();
    let source = f.0.join("source");
    install(&source);
    let archive = f.0.join("fixture.tar.xz");
    assert!(Command::new("tar")
        .args(["--create", "--xz", "--file"])
        .arg(&archive)
        .arg("--directory")
        .arg(&source)
        .args(["CODE", "VARS"])
        .status()
        .unwrap()
        .success());
    let bytes = fs::read(&archive).unwrap();
    let mut reviewed = pin();
    reviewed.archive.bytes = bytes.len() as u64;
    reviewed.archive.sha256 = format!("{:x}", Sha256::digest(&bytes));
    let mut wrong = pin();
    wrong.archive.bytes = reviewed.archive.bytes;
    wrong.archive.sha256 = reviewed.archive.sha256.clone();
    wrong.files[1].sha256 = "incorrect".into();
    assert!(prepare_with(&f.0, &wrong, |_, out| {
        fs::copy(&archive, out)?;
        Ok(())
    })
    .is_err());
    assert!(!cache(&f.0).exists());
    prepare_with(&f.0, &reviewed, |_, out| {
        fs::copy(&archive, out)?;
        Ok(())
    })
    .unwrap();
    pair(&cache(&f.0), &reviewed).unwrap();
    assert!(cache(&f.0).join("copyright").is_file());
    assert!(cache(&f.0).join("publisher-notice.md").is_file());
    assert!(fs::metadata(cache(&f.0).join("VARS"))
        .unwrap()
        .permissions()
        .readonly());
}
