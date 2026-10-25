//! Actual-file readback tests with a synthetic receipt, not a Linux build proof.
use super::*;
use conduit_host_make::{
    build_default_host_image, check_host_configuration, parse_host_configuration_conduit,
    BuildInputs, RealizedBuilderArtifact,
};
use sha2::{Digest, Sha256};
use std::{fs, io::Write, sync::atomic::AtomicU64};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    image: HostImage,
    receipt: BuilderReceipt,
    manifest: Vec<u8>,
    file: File,
    path: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let configuration = parse_host_configuration_conduit(include_str!(
            "../../../../../targets/conduitos/profiles/conduitos-native.host.conduit"
        ))
        .unwrap();
        let checked = check_host_configuration(
            configuration,
            &conduit_workspace_make::catalog(),
            &conduit_workspace_make::package_set(),
        )
        .unwrap();
        let (image, _) = build_default_host_image(
            checked.profile().clone(),
            &conduit_workspace_make::catalog(),
            &conduit_workspace_make::package_set(),
            &BuildInputs {
                source_identity: "git:fixture-source".into(),
                toolchain_available: true,
            },
        )
        .unwrap();
        let path = std::env::temp_dir().join(format!(
            "conduit-creche-readback-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        let bytes = vec![0x5a; 150_000];
        file.write_all(&bytes).unwrap();
        let m = &image.manifest;
        let receipt = BuilderReceipt {
            schema: BUILDER_RECEIPT_SCHEMA.into(),
            request_id: "request/exact".into(),
            builder_implementation_id: BUILDER_BASE_IMPLEMENTATION.into(),
            builder_host_id: "host/exact".into(),
            builder_boot_id: "boot/exact".into(),
            source_identity: m.source_identity.clone(),
            source_revision: "fixture-revision".into(),
            source_content_sha256: "sha256:fixture".into(),
            profile_id: m.profile_id.clone(),
            build_id: m.build_id.clone(),
            image_id: m.image_id.clone(),
            toolchain_identity: m.toolchain_identity.clone(),
            make_package_id: m.make_package_id.clone(),
            builder_adapter: m.builder_adapter.clone(),
            target: m.target.clone(),
            artifact: RealizedBuilderArtifact {
                path: path.clone(),
                bytes: bytes.len() as u64,
                sha256: format!("sha256:{:x}", Sha256::digest(&bytes)),
                evidence_sha256: "sha256:fixture".into(),
                stdout_bytes: 0,
                stderr_bytes: 0,
            },
            carrier_realized: false,
            boot_observed: false,
            destination_host_observed: false,
        };
        let manifest = serde_json::to_vec(&serde_json::json!({ "resolved_build": m })).unwrap();
        Self {
            image,
            receipt,
            manifest,
            file,
            path,
        }
    }
    fn verify(&mut self, cancelled: &AtomicBool) -> Result<(), CrecheProductionRefusal> {
        let m = &self.image.manifest;
        verify_readback(
            &ReadbackExpectation {
                request_id: "request/exact",
                host_id: "host/exact",
                boot_id: "boot/exact",
                source_identity: "git:fixture-source",
                toolchain_identity: &m.toolchain_identity,
                package_id: &m.make_package_id,
                adapter: &m.builder_adapter,
                maximum_bytes: 200_000,
            },
            &self.receipt,
            &self.image,
            &self.manifest,
            &mut self.file,
            cancelled,
        )
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}
#[test]
fn exact_builder_receipt_matches_reopened_bytes_and_resolved_build() {
    let mut fixture = Fixture::new();
    fixture.verify(&AtomicBool::new(false)).unwrap();
    assert_eq!(fixture.file.stream_position().unwrap(), 0);
}
#[test]
fn source_build_and_operation_receipt_mismatches_are_refused() {
    for field in [
        "source",
        "build",
        "image",
        "profile",
        "toolchain",
        "package",
        "adapter",
        "target",
        "request",
        "host",
        "boot",
    ] {
        let mut fixture = Fixture::new();
        let value = match field {
            "source" => &mut fixture.receipt.source_identity,
            "build" => &mut fixture.receipt.build_id,
            "image" => &mut fixture.receipt.image_id,
            "profile" => &mut fixture.receipt.profile_id,
            "toolchain" => &mut fixture.receipt.toolchain_identity,
            "package" => &mut fixture.receipt.make_package_id,
            "adapter" => &mut fixture.receipt.builder_adapter,
            "target" => &mut fixture.receipt.target,
            "request" => &mut fixture.receipt.request_id,
            "host" => &mut fixture.receipt.builder_host_id,
            "boot" => &mut fixture.receipt.builder_boot_id,
            _ => unreachable!(),
        };
        *value = "different".into();
        assert!(fixture.verify(&AtomicBool::new(false)).is_err(), "{field}");
    }
}
#[test]
fn actual_file_mutation_resolved_manifest_swap_and_cancellation_are_refused() {
    let mut fixture = Fixture::new();
    fixture.file.seek(SeekFrom::Start(70_000)).unwrap();
    fixture.file.write_all(b"changed").unwrap();
    assert!(fixture.verify(&AtomicBool::new(false)).is_err());
    let mut fixture = Fixture::new();
    fixture.file.set_len(1).unwrap();
    assert!(fixture.verify(&AtomicBool::new(false)).is_err());
    let mut fixture = Fixture::new();
    fixture.manifest = b"{\"resolved_build\":null}".to_vec();
    assert!(fixture.verify(&AtomicBool::new(false)).is_err());
    let mut fixture = Fixture::new();
    assert!(matches!(
        fixture.verify(&AtomicBool::new(true)),
        Err(CrecheProductionRefusal::Cancelled)
    ));
}
