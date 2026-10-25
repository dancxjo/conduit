//! Fixture ISO byte-I/O proof; no actual kernel build, boot or deployment claim.
use super::*;
use crate::{check_body_description, DeploymentScope};
use conduit_body::{
    PreparedBirthHostRole, PREPARED_BIRTH_MEDIA_BYTES, PREPARED_BIRTH_MEDIA_SCHEMA,
};
use conduit_host_make::{
    build_default_host_image, parse_host_configuration_conduit, BuildInputs, SporeOutputKind,
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom},
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
#[path = "../../tests/common/deployment.rs"]
mod fixture;
const SLOT: usize = 65530; // Deliberately crosses the streaming scan boundary.
const LEGACY: &[u8] = b"CONDUIT_SPORE_MEDIA@1\0";

fn prepared() -> (PreparedDeployment, HostImage) {
    let catalog = conduit_workspace_make::catalog();
    let packages = conduit_workspace_make::package_set();
    let mut draft = fixture::encounter("Willow");
    let config = parse_host_configuration_conduit(include_str!(
        "../../../../targets/conduitos/profiles/conduitos-x86_64-pc.host.conduit"
    ))
    .unwrap();
    let mut description = draft.construction().description().clone();
    for host in &mut description.hosts {
        host.configuration = "fixture/iso.host.conduit".into();
        host.spore.output = SporeOutputKind::DiskImage;
    }
    let checked = check_body_description(
        description,
        &BTreeMap::from([("fixture/iso.host.conduit".into(), config)]),
        &catalog,
        &packages,
    )
    .unwrap();
    draft
        .replace_construction(draft.revision(), checked, "first")
        .unwrap();
    draft
        .select_hosts(draft.revision(), "first", DeploymentScope::AllHosts)
        .unwrap();
    let review = draft.review(draft.revision(), &packages).unwrap();
    let deployment = draft
        .prepare(review, 1, "fixture/one-birth".into(), &catalog, &packages)
        .unwrap();
    let (image, _) = build_default_host_image(
        deployment.construction().hosts()[0]
            .configuration
            .profile()
            .clone(),
        &catalog,
        &packages,
        &BuildInputs {
            source_identity: "git:fixture-source".into(),
            toolchain_available: true,
        },
    )
    .unwrap();
    (deployment, image)
}
fn iso() -> Vec<u8> {
    let mut bytes = vec![0x5a; 160 * 1024];
    bytes[16 * 2048..16 * 2048 + 7].copy_from_slice(&[1, b'C', b'D', b'0', b'0', b'1', 1]);
    blank_slot(&mut bytes, SLOT);
    // A parser string is not a pristine provision slot.
    bytes[1024..1024 + LEGACY.len()].copy_from_slice(LEGACY);
    bytes
}
fn blank_slot(bytes: &mut [u8], offset: usize) {
    let slot = &mut bytes[offset..offset + PREPARED_BIRTH_MEDIA_BYTES];
    slot.fill(0xff);
    slot[..LEGACY.len()].copy_from_slice(LEGACY);
    slot[24..32].fill(0);
}
fn manifest(image: &HostImage, bytes: &[u8]) -> Value {
    let digest = format!("{:x}", Sha256::digest(bytes));
    json!({"schema":"conduit.host/target-build-manifest@3", "artifact_role":"product-host",
        "prepared_birth_media_schema":PREPARED_BIRTH_MEDIA_SCHEMA,
        "profile_id":image.manifest.profile_id, "build_id":image.manifest.build_id,
        "source_identity":image.manifest.source_identity, "toolchain_identity":image.manifest.toolchain_identity,
        "target":image.manifest.target, "resolved_description_binding":image.manifest.image_id,
        "image_id":format!("image:sha256:{digest}"),
        "image":{"role":"final-bootable-image","file":"fixture.iso","sha256":digest},
        "resolved_build":image.manifest})
}
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Files {
    root: PathBuf,
    input: File,
    output: File,
}
impl Files {
    fn new(bytes: &[u8]) -> Self {
        let root = std::env::temp_dir().join(format!(
            "conduit-birth-artifact-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("base.iso"), bytes).unwrap();
        let input = File::open(root.join("base.iso")).unwrap();
        let output = OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .open(root.join("bound.iso"))
            .unwrap();
        Self {
            root,
            input,
            output,
        }
    }
    fn bind(
        &mut self,
        image: &HostImage,
        media: &PreparedBirthMedia,
        manifest: &Value,
    ) -> Result<PreparedBirthArtifactReceipt, PreparedBirthArtifactRefusal> {
        bind_prepared_birth_iso(
            &mut self.input,
            &mut self.output,
            image,
            media,
            &serde_json::to_vec(manifest).unwrap(),
            1024 * 1024,
            &mut || Ok(()),
        )
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn selected_hosts_keep_one_birth_and_require_exact_profile_and_payload() {
    let (prepared, image) = prepared();
    let first = prepared.birth_media_for_image("first", &image).unwrap();
    let later = prepared.birth_media_for_image("second", &image).unwrap();
    assert_eq!(first.evidence(), later.evidence());
    assert_eq!(first.role(), PreparedBirthHostRole::FirstHost);
    assert_eq!(later.role(), PreparedBirthHostRole::LaterHost);
    assert!(prepared.birth_media_for_image("unknown", &image).is_err());
    let mut wrong = image.clone();
    wrong.manifest.source_configuration_id = Some("configuration/forged".into());
    assert!(prepared.birth_media_for_image("first", &wrong).is_err());
    let mut wrong = image.clone();
    wrong.payload.target = "target/forged".into();
    assert!(prepared.birth_media_for_image("first", &wrong).is_err());
    let mut wrong = image;
    wrong.manifest.build_id = "build/forged".into();
    assert!(prepared.birth_media_for_image("first", &wrong).is_err());
}

#[test]
fn actual_file_copy_patches_one_slot_and_reads_back_exact_birth_and_final_digest() {
    let (prepared, image) = prepared();
    let media = prepared.birth_media_for_image("first", &image).unwrap();
    let original = iso();
    let mut files = Files::new(&original);
    let receipt = files
        .bind(&image, &media, &manifest(&image, &original))
        .unwrap();
    assert_eq!(receipt.frame_offset, SLOT as u64);
    assert_eq!(receipt.artifact_bytes, original.len() as u64);
    assert!(!receipt.boot_observed && !receipt.join_observed);
    let bound = fs::read(files.root.join("bound.iso")).unwrap();
    assert_eq!(fs::read(files.root.join("base.iso")).unwrap(), original);
    assert_eq!(&bound[..SLOT], &original[..SLOT]);
    assert_eq!(
        &bound[SLOT + PREPARED_BIRTH_MEDIA_BYTES..],
        &original[SLOT + PREPARED_BIRTH_MEDIA_BYTES..]
    );
    assert_eq!(
        PreparedBirthMedia::decode(&bound[SLOT..SLOT + PREPARED_BIRTH_MEDIA_BYTES]).unwrap(),
        media
    );
    assert_eq!(
        receipt.artifact_sha256,
        format!("sha256:{:x}", Sha256::digest(&bound))
    );
    assert_eq!(
        receipt.frame_sha256,
        format!(
            "sha256:{:x}",
            Sha256::digest(&bound[SLOT..SLOT + PREPARED_BIRTH_MEDIA_BYTES])
        )
    );
    assert_eq!(
        files.bind(&image, &media, &manifest(&image, &original)),
        Err(PreparedBirthArtifactRefusal::Io)
    );
}

#[test]
fn missing_consumer_and_false_target_metadata_refuse_without_writing_output() {
    let (prepared, image) = prepared();
    let media = prepared.birth_media_for_image("first", &image).unwrap();
    let bytes = iso();
    for field in [
        "schema",
        "artifact_role",
        "profile_id",
        "build_id",
        "source_identity",
        "target",
        "toolchain_identity",
        "resolved_description_binding",
        "image_id",
        "prepared_birth_media_schema",
    ] {
        let mut metadata = manifest(&image, &bytes);
        metadata[field] = Value::String("forged".into());
        let mut files = Files::new(&bytes);
        assert!(files.bind(&image, &media, &metadata).is_err());
        assert_eq!(files.output.metadata().unwrap().len(), 0);
    }
    let mut metadata = manifest(&image, &bytes);
    metadata
        .as_object_mut()
        .unwrap()
        .remove("prepared_birth_media_schema");
    assert_eq!(
        Files::new(&bytes).bind(&image, &media, &metadata),
        Err(PreparedBirthArtifactRefusal::UnsupportedConsumer)
    );
}

#[test]
fn corrupt_occupied_ambiguous_missing_and_non_iso_inputs_are_refused() {
    let (prepared, image) = prepared();
    let media = prepared.birth_media_for_image("first", &image).unwrap();
    let original = iso();
    let metadata = manifest(&image, &original);
    let mut corrupt = original.clone();
    corrupt[100] ^= 1;
    assert_eq!(
        Files::new(&corrupt).bind(&image, &media, &metadata),
        Err(PreparedBirthArtifactRefusal::InvalidManifest)
    );
    for (bytes, expected) in {
        let mut occupied = original.clone();
        occupied[SLOT + 40] = 1;
        let mut ambiguous = original.clone();
        blank_slot(&mut ambiguous, 112 * 1024);
        let mut missing = original.clone();
        missing[SLOT..SLOT + LEGACY.len()].fill(0);
        let mut invalid_iso = original.clone();
        invalid_iso[32769] = 0;
        [
            (occupied, PreparedBirthArtifactRefusal::SlotOccupied),
            (ambiguous, PreparedBirthArtifactRefusal::SlotAmbiguous),
            (missing, PreparedBirthArtifactRefusal::SlotMissing),
            (invalid_iso, PreparedBirthArtifactRefusal::InvalidIso),
        ]
    } {
        assert_eq!(
            Files::new(&bytes).bind(&image, &media, &manifest(&image, &bytes)),
            Err(expected)
        );
    }
    let mut occupied = original.clone();
    media
        .encode_into(&mut occupied[SLOT..SLOT + PREPARED_BIRTH_MEDIA_BYTES])
        .unwrap();
    assert_eq!(
        Files::new(&occupied).bind(&image, &media, &manifest(&image, &occupied)),
        Err(PreparedBirthArtifactRefusal::SlotOccupied)
    );
}

#[test]
fn cancellation_and_candidate_work_bounds_preserve_caller_owned_partial_output() {
    let (prepared, image) = prepared();
    let media = prepared.birth_media_for_image("first", &image).unwrap();
    let bytes = iso();
    let mut files = Files::new(&bytes);
    let mut checks = 0;
    let result = bind_prepared_birth_iso(
        &mut files.input,
        &mut files.output,
        &image,
        &media,
        &serde_json::to_vec(&manifest(&image, &bytes)).unwrap(),
        1024 * 1024,
        &mut || {
            checks += 1;
            if checks == 3 {
                Err(PreparedBirthArtifactRefusal::Cancelled)
            } else {
                Ok(())
            }
        },
    );
    assert_eq!(result, Err(PreparedBirthArtifactRefusal::Cancelled));
    assert!(files.output.metadata().unwrap().len() > 0);
    assert!(files.root.join("bound.iso").is_file());
    assert_eq!(fs::read(files.root.join("base.iso")).unwrap(), bytes);
    let mut crowded = iso();
    for i in 0..65 {
        crowded[2048 + i * 32..2048 + i * 32 + LEGACY.len()].copy_from_slice(LEGACY);
    }
    assert_eq!(
        Files::new(&crowded).bind(&image, &media, &manifest(&image, &crowded)),
        Err(PreparedBirthArtifactRefusal::BoundExhausted)
    );
    files.output.seek(SeekFrom::Start(0)).unwrap();
    let mut observed = Vec::new();
    files.output.read_to_end(&mut observed).unwrap();
    assert_eq!(observed, bytes[..observed.len()]);
}
