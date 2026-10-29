#![cfg(unix)]
use conduit_body::{BodyId, MembershipCredential};
use conduit_core::{BootId, HostId, OfferGeneration};
use conduit_environment_host::member_binding::{ExpectedMember, ValidatedMemberBinding};
use conduit_environment_runtime::member_receipts::{admission, current};
use ed25519_dalek::{Signer, SigningKey};
use serde_json::{json, Value};
use std::{
    io::Write,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
    path::PathBuf,
};
struct Fixture {
    dir: PathBuf,
    key: SigningKey,
    body: BodyId,
    host: HostId,
    boot: BootId,
}
impl Fixture {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "conduit-member-binding-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::DirBuilder::new().mode(0o700).create(&dir).unwrap();
        Self {
            dir,
            key: SigningKey::from_bytes(&[42; 32]),
            body: serde_json::from_value(json!("02".repeat(32))).unwrap(),
            host: "04".repeat(32).into(),
            boot: "05".repeat(32).into(),
        }
    }
    fn root(&self) -> [u8; 32] {
        self.key.verifying_key().to_bytes()
    }
    fn credential(&self) -> MembershipCredential {
        serde_json::from_value(json!({"credential_id":"01".repeat(32),"body_id":self.body,"part_id":"03".repeat(32),"host_id":self.host,"boot_id":self.boot,"issued_at_millis":123})).unwrap()
    }
    fn binding(&self, recovery: bool) -> Value {
        let mut admission = admission::JoinedHostReceipt {
            schema: admission::PROFILE.into(),
            credential: self.credential(),
            membership_revision: 7,
            producer_host: "06".repeat(32).into(),
            producer_boot: "07".repeat(32).into(),
            producer_key: self.root(),
            invitation_id: serde_json::from_value(json!("08".repeat(32))).unwrap(),
            offer_generation: OfferGeneration(9),
            signature: vec![],
        };
        admission.signature = self
            .key
            .sign(&admission::transcript(&admission.scope()))
            .to_bytes()
            .to_vec();
        let mut binding = json!({"schema":"conduit.environment/joined-recorder@1","invitation":{"invitation_id":"08".repeat(32),"body_id":self.body,"nonce":([1;32]),"expires_at_millis":1000},"root_key":self.root(),"local_host":self.host,"local_boot":self.boot,"receipt":admission});
        if recovery {
            let request = current::Request {
                schema: current::REQUEST.into(),
                body_id: self.body.clone(),
                host_id: self.host.clone(),
                boot_id: self.boot.clone(),
                nonce: [11; 32],
                signature: vec![12; 64],
            };
            let mut receipt = current::Receipt {
                schema: current::RECEIPT.into(),
                credential: self.credential(),
                membership_revision: 7,
                producer_host: "06".repeat(32).into(),
                producer_boot: "07".repeat(32).into(),
                producer_key: self.root(),
                request_nonce: request.nonce,
                signature: vec![],
            };
            receipt.signature = self.key.sign(&receipt.transcript()).to_bytes().to_vec();
            binding["schema"] = json!("conduit.environment/joined-recorder@2");
            binding["receipt"] = json!(receipt);
            binding["recovery_request"] = json!(request);
        }
        binding
    }
    fn write(&self, value: &Value) {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(self.dir.join("binding.json"))
            .unwrap();
        file.write_all(&serde_json::to_vec(value).unwrap()).unwrap();
    }
    fn valid(&self) -> bool {
        let root = self.root();
        ValidatedMemberBinding::reopen(
            &self.dir.join("binding.json"),
            &ExpectedMember {
                root: &root,
                body: &self.body,
                host: &self.host,
                boot: &self.boot,
            },
        )
        .is_ok()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
#[test]
fn actual_signed_admission_and_current_receipt_preserve_member_scope() {
    let f = Fixture::new();
    for recovery in [false, true] {
        f.write(&f.binding(recovery));
        assert!(f.valid());
    }
}
#[test]
fn independently_selected_pins_cannot_be_chosen_by_signed_file() {
    let f = Fixture::new();
    f.write(&f.binding(false));
    let root = f.root();
    let other = "ff".repeat(32);
    for which in 0..4 {
        let badroot = [3; 32];
        let body: BodyId = serde_json::from_value(json!(other)).unwrap();
        let host: HostId = other.clone().into();
        let boot: BootId = other.clone().into();
        assert!(ValidatedMemberBinding::reopen(
            &f.dir.join("binding.json"),
            &ExpectedMember {
                root: if which == 0 { &badroot } else { &root },
                body: if which == 1 { &body } else { &f.body },
                host: if which == 2 { &host } else { &f.host },
                boot: if which == 3 { &boot } else { &f.boot }
            }
        )
        .is_err());
    }
}
#[test]
fn signature_invitation_schema_and_recovery_scope_refuse() {
    let f = Fixture::new();
    for recovery in [false, true] {
        let value = f.binding(recovery);
        let mut bad = value.clone();
        bad["receipt"]["signature"][0] =
            json!(value["receipt"]["signature"][0].as_u64().unwrap() ^ 1);
        f.write(&bad);
        assert!(!f.valid());
        let mut bad = value.clone();
        bad["invitation"]["nonce"] = json!(vec![0; 32]);
        f.write(&bad);
        assert!(!f.valid());
        let mut bad = value.clone();
        bad["schema"] = json!("foreign");
        f.write(&bad);
        assert!(!f.valid());
        if recovery {
            let mut bad = value.clone();
            bad["recovery_request"]["nonce"] = json!(vec![2; 32]);
            f.write(&bad);
            assert!(!f.valid());
        } else {
            let mut bad = value.clone();
            bad["invitation"]["invitation_id"] = json!("ff".repeat(32));
            f.write(&bad);
            assert!(!f.valid());
            let mut bad = value;
            bad["invitation"]["expires_at_millis"] = json!(100);
            f.write(&bad);
            assert!(!f.valid());
        }
    }
}
#[test]
fn file_capacity_permissions_symlinks_and_malformed_input_refuse() {
    let f = Fixture::new();
    f.write(&f.binding(false));
    let path = f.dir.join("binding.json");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(!f.valid());
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    let root = f.root();
    let expected = ExpectedMember {
        root: &root,
        body: &f.body,
        host: &f.host,
        boot: &f.boot,
    };
    std::os::unix::fs::symlink(&path, f.dir.join("link")).unwrap();
    assert!(ValidatedMemberBinding::reopen(&f.dir.join("link"), &expected).is_err());
    std::fs::set_permissions(&f.dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(!f.valid());
    std::fs::set_permissions(&f.dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::write(&path, vec![b' '; 16385]).unwrap();
    assert!(!f.valid());
    std::fs::write(&path, b"{").unwrap();
    assert!(!f.valid());
}

#[test]
fn pinned_native_codecs_produce_exact_shared_transcripts_and_receipts() {
    // Generated by the actual native receipt modules at fixture.source, using
    // only public synthetic identities and the deterministic fixture key42.
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/native-member-receipts.json")).unwrap();
    let root: [u8; 32] = serde_json::from_value(fixture["root"].clone()).unwrap();
    let admission: admission::JoinedHostReceipt =
        serde_json::from_value(fixture["admission"].clone()).unwrap();
    let current: current::Receipt = serde_json::from_value(fixture["current"].clone()).unwrap();
    let request: current::Request = serde_json::from_value(fixture["request"].clone()).unwrap();
    let hex = |bytes: [u8; 32]| bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
    assert_eq!(
        hex(admission::transcript(&admission.scope())),
        fixture["admission_transcript"]
    );
    assert_eq!(hex(current.transcript()), fixture["current_transcript"]);
    assert_eq!(hex(request.transcript()), fixture["request_transcript"]);
    admission.verify(&root).unwrap();
    current.verify(&root, &request).unwrap();
    assert_eq!(
        serde_json::to_value(admission).unwrap(),
        fixture["admission"]
    );
    assert_eq!(serde_json::to_value(current).unwrap(), fixture["current"]);
}

#[test]
fn root_signed_overlong_or_control_identity_still_refuses() {
    let f = Fixture::new();
    for part in ["x".repeat(129), "bad\npart".into()] {
        let mut value = f.binding(false);
        value["receipt"]["credential"]["part_id"] = json!(part);
        let mut receipt: admission::JoinedHostReceipt =
            serde_json::from_value(value["receipt"].clone()).unwrap();
        receipt.signature = f
            .key
            .sign(&admission::transcript(&receipt.scope()))
            .to_bytes()
            .to_vec();
        value["receipt"] = json!(receipt);
        f.write(&value);
        assert!(!f.valid());
    }
}

#[test]
fn fifo_refuses_without_waiting_for_a_writer() {
    const CHILD: &str = "CONDUIT_MEMBER_FIFO_TEST_CHILD";
    if std::env::var_os(CHILD).is_some() {
        use std::os::unix::ffi::OsStrExt;
        let f = Fixture::new();
        let path =
            std::ffi::CString::new(f.dir.join("binding.json").as_os_str().as_bytes()).unwrap();
        // SAFETY: valid NUL-terminated private test pathname and fixed mode.
        assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
        assert!(!f.valid());
        return;
    }
    // A regression to blocking open must fail in bounded time, not hang CI.
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "fifo_refuses_without_waiting_for_a_writer"])
        .env(CHILD, "1")
        .stdout(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        if std::time::Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("FIFO binding validation blocked waiting for a writer");
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}
