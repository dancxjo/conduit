#![cfg(target_os = "linux")]
//! Actual application catalogue Sources through installed JSON/State execution.
//! Offers, grants and Body admission are CPU fixtures; file/process/kernel
//! behavior is real. No prior chunk publication, live producer or ACK claim.
use conduit_core as c;
#[path = "environment_catalogue_checkpoint/allocation.rs"]
mod allocation;
#[path = "../../../bodies/environment/make/tests/catalogue_checkpoint/catalogue.rs"]
mod catalogue;
#[path = "environment_catalogue_checkpoint/execution.rs"]
mod execution;
#[path = "environment_catalogue_checkpoint/host.rs"]
mod host;
#[path = "environment_catalogue_checkpoint/readback.rs"]
mod readback;
#[path = "../../../bodies/environment/make/tests/catalogue_checkpoint/source.rs"]
mod source;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};
const BODY: &str = "body/catalogue-runtime-fixture";
const CHILD_TEST: &str = "actual_catalogue_seed_recovers_exactly_in_fresh_process_and_boot";
fn chunk_reference(seed: u8) -> c::BoundedResourceRef {
    c::BoundedResourceRef {
        identity: c::ResourceSemanticIdentity::from_digest([seed; 32]),
        content_profile: "text/json-utf8@1".into(),
        access_class: "resource/snapshot@1".into(),
        extent: c::ResourceExtent {
            bytes: 1024,
            items: Some(1),
        },
        lifetime: c::ResourceLifetime {
            version: c::ResourceVersionIdentity::from_digest([seed; 32]),
            expires_at: None,
        },
    }
}
fn catalogue_bytes() -> Vec<u8> {
    let refs = (1..=16).map(chunk_reference).collect::<Vec<_>>();
    let catalogue = catalogue::PreparedRecordingCatalog::prepare(&refs, 17, 32768).unwrap();
    let mut reader = catalogue::PreparedRecordingCatalogReader::default();
    reader.read(catalogue.content()).unwrap();
    assert!(!reader.is_empty());
    assert_eq!(reader.len(), 16);
    for (index, reference) in refs.iter().enumerate() {
        assert_eq!(
            reader.encoded_reference(index),
            Some(reference.encode().unwrap().as_slice())
        );
    }
    assert_eq!(catalogue.required_records(), 17);
    assert_eq!(
        catalogue.required_content_bytes(),
        16 * 1024 + catalogue.content().len() as u64
    );
    conduit_web::validate_json_text(catalogue.content().as_bytes()).unwrap();
    catalogue.content().as_bytes().to_vec()
}
fn catalogue_reference() -> c::BoundedResourceRef {
    let mut reference = chunk_reference(99);
    reference.extent.bytes = catalogue_bytes().len() as u64;
    reference
}
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "conduit-catalogue-runtime-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn stored_files(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn collect(root: &Path, path: &Path, values: &mut BTreeMap<PathBuf, Vec<u8>>) {
        if !path.exists() {
            return;
        }
        for entry in fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                collect(root, &path, values);
            } else {
                values.insert(
                    path.strip_prefix(root).unwrap().to_path_buf(),
                    fs::read(path).unwrap(),
                );
            }
        }
    }
    let mut values = BTreeMap::new();
    collect(root, root, &mut values);
    values
}
#[test]
fn actual_catalogue_seed_recovers_exactly_in_fresh_process_and_boot() {
    const MODE: &str = "CONDUIT_CATALOGUE_RUNTIME_MODE";
    const ROOT: &str = "CONDUIT_CATALOGUE_RUNTIME_ROOT";
    if let Ok(mode) = std::env::var(MODE) {
        let root = PathBuf::from(std::env::var_os(ROOT).unwrap());
        let write = mode == "publish";
        let (values, fragment) = execution::execute(
            &root,
            &format!("boot/{mode}"),
            BODY,
            write.then(catalogue_bytes),
            false,
        )
        .unwrap();
        assert_eq!(
            execution::reference_bytes(&values).unwrap(),
            catalogue_reference().encode().unwrap()
        );
        assert_eq!(fragment.states.len(), 1);
        if !write {
            readback::verify(&root, &execution::reference_bytes(&values).unwrap());
        }
        if write {
            assert_eq!(values.len(), 2);
        } else {
            assert_eq!(values.len(), 1);
        }
        return;
    }
    let root = Directory::new();
    fs::create_dir(root.0.join("snapshots")).unwrap();
    let (_, write, _) = host::prepare(&root.0.join("snapshots"), "boot/plan-write", true);
    let (_, read, _) = host::prepare(&root.0.join("snapshots"), "boot/plan-read", false);
    assert_eq!(
        write.states[0].maximum_value_bytes,
        source::MAXIMUM_SEED_BYTES
    );
    assert_eq!(
        read.states[0].maximum_value_bytes,
        source::MAXIMUM_SEED_BYTES
    );
    assert_eq!(write.states[0].lifetime, c::StateLifetime::Body);
    assert_eq!(write.states[0].state_id, read.states[0].state_id);
    assert_eq!(write.states[0].value_kind, read.states[0].value_kind);
    assert_eq!(write.states[0].initial_value, read.states[0].initial_value);
    let expected_type = source::read_only().unwrap().value_type;
    let initial = c::StructuredInfoValue::from_canonical_bytes(
        write.states[0].initial_value.as_ref().unwrap(),
    )
    .unwrap();
    assert_eq!(initial.value_type(), &expected_type);
    assert!(matches!(
        initial.shape(),
        c::StructuredInfoValueShape::Variant { tag: "none", .. }
    ));
    for mode in ["publish", "recover"] {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", CHILD_TEST, "--nocapture"])
            .env(MODE, mode)
            .env(ROOT, &root.0)
            .status()
            .unwrap();
        assert!(status.success(), "fresh {mode} process failed");
    }
    let (foreign, _) = execution::execute(
        &root.0,
        "boot/foreign-body",
        "body/another-catalogue-fixture",
        None,
        false,
    )
    .unwrap();
    assert_eq!(execution::reference_bytes(&foreign), None);
    let (same, _) = execution::execute(&root.0, "boot/same-body-again", BODY, None, false).unwrap();
    assert_eq!(
        execution::reference_bytes(&same).unwrap(),
        catalogue_reference().encode().unwrap()
    );
}
#[test]
fn invalid_json_occupied_generation_and_precancellation_preserve_committed_catalogue_seed() {
    let root = Directory::new();
    let json = catalogue_bytes();
    execution::execute(&root.0, "boot/initial", BODY, Some(json.clone()), false).unwrap();
    let before = stored_files(&root.0);
    let mut malformed = json.clone();
    malformed[0] = b'!';
    assert!(
        execution::execute(&root.0, "boot/invalid-json", BODY, Some(malformed), false).is_err()
    );
    assert_eq!(stored_files(&root.0), before);
    // A valid catalogue with different references cannot overwrite this selected
    // immutable generation, even with equal exact encoded extent.
    let refs = (2..=17).map(chunk_reference).collect::<Vec<_>>();
    let replacement = catalogue::PreparedRecordingCatalog::prepare(&refs, 17, 32768).unwrap();
    let mut validator = catalogue::PreparedRecordingCatalogReader::default();
    validator.read(replacement.content()).unwrap();
    assert_eq!(replacement.content().len(), json.len());
    assert!(execution::execute(
        &root.0,
        "boot/occupied",
        BODY,
        Some(replacement.content().as_bytes().to_vec()),
        false
    )
    .is_err());
    let after_occupied = stored_files(&root.0);
    // The immutable publication protocol retains its staged candidate after a
    // failed non-replacing link. Root reconciles it; it is not a committed record.
    for (path, bytes) in &before {
        assert!(
            after_occupied
                .get(path)
                .is_some_and(|actual| actual == bytes),
            "occupied publication changed committed file {path:?}"
        );
    }
    let published = before
        .keys()
        .find(|path| path.starts_with("snapshots"))
        .unwrap();
    let candidate = Path::new("snapshots").join(format!(
        ".pending-{}",
        published.file_name().unwrap().to_str().unwrap()
    ));
    assert_eq!(after_occupied.len(), before.len() + 1);
    assert!(!before.contains_key(&candidate));
    assert!(after_occupied
        .get(&candidate)
        .unwrap()
        .ends_with(replacement.content().as_bytes()));
    let (cancelled, _) =
        execution::execute(&root.0, "boot/cancelled", BODY, Some(json), true).unwrap();
    assert!(cancelled.is_empty());
    assert!(
        stored_files(&root.0) == after_occupied,
        "precancellation changed file custody"
    );
    let (values, _) =
        execution::execute(&root.0, "boot/after-refusals", BODY, None, false).unwrap();
    assert_eq!(
        execution::reference_bytes(&values).unwrap(),
        catalogue_reference().encode().unwrap()
    );
    readback::verify(&root.0, &execution::reference_bytes(&values).unwrap());
    assert!(
        stored_files(&root.0) == after_occupied,
        "recovery changed file custody"
    );
}
