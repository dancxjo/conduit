//! Product selected-host CPU proof, not CLI, Play, durable ACK or live sensor proof.
use super::*;
use conduit_core::{BoundedResourceRef, OfferGeneration, ResourceRetention};
use conduit_dht11_observation::{
    recording::{PreparedRecordingChunk, RecordingContext, RecordingDisposition},
    ObservationContract, PreparedObservationQuantityForms,
};
use conduit_host_make::{build_default_host_image, BuildInputs, HostImage};
use conduit_linux_snapshot_store::PreparedFileSnapshot;
use conduit_std_host::StdHostConfig;
use conduit_std_offers::*;
use std::{collections::BTreeMap, fs, path::Path};

fn config() -> StdHostConfig {
    StdHostConfig {
        host_id: "host/product-environment-history".into(),
        boot_id: "boot/product-current".into(),
        offer_generation: OfferGeneration(1),
    }
}
fn original_json() -> String {
    let contract = ObservationContract::prepare().unwrap();
    let (host, boot, plan, play) = (
        "host/reported".into(),
        "boot/reported".into(),
        "plan/reported".into(),
        "play/reported".into(),
    );
    let mut chunk = PreparedRecordingChunk::prepare(
        RecordingContext {
            host: &host,
            boot: &boot,
            plan: &plan,
            play: &play,
        },
        &contract,
    )
    .unwrap();
    let mut forms = PreparedObservationQuantityForms::prepare(&contract).unwrap();
    let observation = forms.unpack(&[198, 0, 78, 2]).unwrap();
    chunk
        .append(1, 7, RecordingDisposition::Available, Some(observation))
        .unwrap();
    chunk
        .append(4, 101, RecordingDisposition::Checksum, None)
        .unwrap();
    chunk.seal().unwrap().into()
}
fn records(path: &Path) -> BTreeMap<std::ffi::OsString, Vec<u8>> {
    fs::read_dir(path)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            (entry.file_name(), fs::read(entry.path()).unwrap())
        })
        .collect()
}
fn equipment(path: &Path, reference: &BoundedResourceRef) -> (CheckedHostConfiguration, HostImage) {
    let directory = serde_json::to_string(path.to_str().unwrap()).unwrap();
    let reference = reference
        .encode()
        .unwrap()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let maximum = conduit_semantic_catalog::reported_generation_maximum_bytes().unwrap();
    let face_memory = conduit_semantic_catalog::reported_generation_face_workspace_bytes().unwrap();
    let memory = 64 * 1024 * 1024;
    let source = format!(
        r#"host product-environment-history-test {{
 schema = 1
 target = {{architecture: "x86_64", machine: "computer", os: "linux"}}
 base = {{kind: "timer/monotonic", implementation: "hosted/monotonic-clock@1"}}
 base = {{kind: "{ENVIRONMENT_RECORDING_HISTORY_BASE}", implementation: "{ENVIRONMENT_RECORDING_HISTORY_IMPLEMENTATION}", initialization: {{source_pool: "history/source", memory_pool: "history/source-memory", capture_access: "operator-admitted", directory: {directory}, reference: "{reference}", producer_host: "host/reported", producer_boot: "boot/reported", producer_plan: "plan/reported", producer_play: "play/reported", temperature_source: "source/t", temperature_channel: "channel/t", temperature_name: "Temperature", humidity_source: "source/h", humidity_channel: "channel/h", humidity_name: "Relative humidity"}}}}
 base = {{kind: "{REPORTED_GENERATION_FACE_BASE}", implementation: "{REPORTED_GENERATION_FACE_DRIVER}", initialization: {{face_pool: "history/face", memory_pool: "history/face-memory"}}}}
 capability = {{kind: "time/every", pinned_implementation: "{TIME_EVERY_IMPLEMENTATION}"}}
 capability = {{kind: "data/environment-recording-history", pinned_implementation: "{ENVIRONMENT_RECORDING_HISTORY_IMPLEMENTATION}"}}
 capability = {{kind: "face/reported-generation-series", pinned_implementation: "{REPORTED_GENERATION_FACE_IMPLEMENTATION}"}}
 need = {{id: "timer/main", class: "conduit.resource/timer-slot@1", slots: 8, bytes: 512}}
 need = {{id: "history/source", class: "{ENVIRONMENT_RECORDING_HISTORY_RESOURCE}", slots: 1, bytes: {maximum}}}
 need = {{id: "history/source-memory", class: "conduit.resource/runtime-memory@1", slots: {memory}, bytes: {memory}}}
 need = {{id: "history/face", class: "conduit.resource/face-slot@1", slots: 1, bytes: 1}}
 need = {{id: "history/face-memory", class: "conduit.resource/runtime-memory@1", slots: {face_memory}, bytes: {face_memory}}}
 limits = {{static_memory_bytes: 16777216, heap_arena_bytes: 268435456, queue_items: 4096, buffered_bytes: 16777216, active_instances: 512, operation_slots: 256, timer_slots: 128, line_sessions: 64, evidence_items: 4096}}
}}"#
    );
    let catalog = conduit_workspace_make::catalog();
    let packages = conduit_workspace_make::package_set();
    let checked = check_host_configuration(
        parse_host_configuration_conduit(&source).unwrap(),
        &catalog,
        &packages,
    )
    .unwrap();
    let (image, _) = build_default_host_image(
        checked.profile().clone(),
        &catalog,
        &packages,
        &BuildInputs {
            source_identity: "fixture/product-environment-history".into(),
            toolchain_available: true,
        },
    )
    .unwrap();
    (checked, image)
}
fn selection(checked: &CheckedHostConfiguration, image: &HostImage) -> Selection {
    Selection {
        source: canonical_host_configuration_conduit(checked.configuration()).unwrap(),
        configuration_id: checked.configuration_id().into(),
        host_image: Some(image.clone()),
    }
}
fn initialize(selected: &Selection) -> Result<StdHost, String> {
    selected.initialize_image(
        config(),
        #[cfg(all(target_os = "linux", feature = "native-display"))]
        None,
    )
}

#[test]
fn ordinary_selection_initializes_readonly_boot_original_json_with_canonical_image() {
    let root = Directory::new();
    let directory = root.open();
    let json = original_json();
    let write = WriteFacts::new(json.len());
    PreparedFileSnapshot::prepare_selected(
        &directory,
        write.selected(),
        &write.reference,
        &write.original,
        &write.local,
    )
    .unwrap()
    .publish(&write.authority, json.as_bytes())
    .unwrap();
    let before = records(&root.0);
    let read = super::snapshot_read::ReadBinding::authorize(
        &config(),
        &directory,
        write.reference.clone(),
    )
    .unwrap();
    assert_eq!(
        read.capture()
            .selected
            .binding
            .content
            .as_ref()
            .unwrap()
            .contract
            .retention,
        ResourceRetention::Boot
    );
    let (checked, image) = equipment(&root.0, &write.reference);
    let selected = selection(&checked, &image);
    selected.checked().unwrap();
    let host = initialize(&selected).unwrap();
    assert!(
        host.native_source_worker_inspection()
            .unwrap()
            .idle_backing_is_reserved
    );
    assert_eq!(records(&root.0), before);
    let mut missing_image = selected.clone();
    missing_image.host_image = None;
    assert!(missing_image
        .checked()
        .unwrap_err()
        .contains("canonical --image"));
    let mut forged_image = selected.clone();
    forged_image.host_image.as_mut().unwrap().manifest.image_id = "image/forged".into();
    assert!(forged_image.checked().is_err());
    let mut changed_source = selected;
    changed_source.source = changed_source
        .source
        .replace("boot/reported", "boot/foreign");
    assert!(changed_source.checked().is_err());
    assert_eq!(records(&root.0), before);
}

#[test]
fn ordinary_selection_missing_record_and_exact_reference_mismatch_refuse_without_ready_host() {
    let root = Directory::new();
    let directory = root.open();
    let json = original_json();
    let write = WriteFacts::new(json.len());
    let (checked, image) = equipment(&root.0, &write.reference);
    assert!(initialize(&selection(&checked, &image)).is_err());
    assert!(records(&root.0).is_empty());
    PreparedFileSnapshot::prepare_selected(
        &directory,
        write.selected(),
        &write.reference,
        &write.original,
        &write.local,
    )
    .unwrap()
    .publish(&write.authority, json.as_bytes())
    .unwrap();
    let before = records(&root.0);
    // Same generation identity/version, different exact extent. A new canonical
    // IMAGE cannot turn the already published original into this other reference.
    let aliases = Directory::new();
    let linked = aliases.0.join("selected-directory");
    std::os::unix::fs::symlink(&root.0, &linked).unwrap();
    let (linked_checked, linked_image) = equipment(&linked, &write.reference);
    assert!(initialize(&selection(&linked_checked, &linked_image)).is_err());
    assert_eq!(records(&root.0), before);
    let mut reference = write.reference.clone();
    reference.extent.bytes -= 1;
    let (checked, image) = equipment(&root.0, &reference);
    assert!(initialize(&selection(&checked, &image)).is_err());
    assert_eq!(records(&root.0), before);
}

// Minimal original-record custody for this product test, independent of producer
// metadata and selected read grants. No generated capture is retained in Git.
struct Directory(std::path::PathBuf);
impl Directory {
    fn new() -> Self {
        use std::os::unix::fs::DirBuilderExt;
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "conduit-product-recording-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        Self(path)
    }
    fn open(&self) -> conduit_linux_snapshot_store::SnapshotDirectory {
        conduit_linux_snapshot_store::SnapshotDirectory::open_selected(&self.0).unwrap()
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
struct WriteFacts {
    host: conduit_core::HostId,
    boot: conduit_core::BootId,
    kind: conduit_core::KindId,
    capability: conduit_core::CapabilityId,
    binding: conduit_core::ResourceBinding,
    call: conduit_core::HostCallRequirement,
    authority: conduit_core::AuthorityBinding,
    reference: BoundedResourceRef,
    original: conduit_core::AuthorityGrant,
    local: conduit_core::ResourceReferenceBinding,
}
impl WriteFacts {
    fn new(bytes: usize) -> Self {
        Self::for_host(bytes, config())
    }
    fn for_host(bytes: usize, config: StdHostConfig) -> Self {
        use conduit_core::*;
        let host = config.host_id;
        let boot = config.boot_id;
        let kind = kind_id("resource/snapshot-publish");
        let capability: CapabilityId = "conduit.host/resource-snapshot-publish@1".into();
        let original = AuthorityGrant {
            grant_id: "fixture/original-record-write".into(),
            contract_id: "authority/resource-snapshot@1".into(),
            host_call_contract_id: "conduit.host/resource-snapshot-publish@1".into(),
            subject_kind: kind.clone(),
            host_id: host.clone(),
            boot_id: boot.clone(),
            capability_id: capability.clone(),
        };
        let authority = AuthorityBinding {
            grant_id: original.grant_id.clone(),
            contract_id: original.contract_id.clone(),
            host_call_contract_id: original.host_call_contract_id.clone(),
            subject_kind: kind.clone(),
            host_id: host.clone(),
            boot_id: boot.clone(),
            capability_id: capability.clone(),
        };
        let reference = BoundedResourceRef {
            identity: ResourceSemanticIdentity::from_digest([1; 32]),
            content_profile: "text/json-utf8@1".into(),
            access_class: "resource/snapshot@1".into(),
            extent: ResourceExtent {
                bytes: bytes as u64,
                items: Some(1),
            },
            lifetime: ResourceLifetime {
                version: ResourceVersionIdentity::from_digest([2; 32]),
                expires_at: None,
            },
        };
        let local = ResourceReferenceBinding {
            identity: reference.identity,
            version: reference.lifetime.version,
            content_profile: reference.content_profile.clone(),
            access_class: reference.access_class.clone(),
            handle: "fixture/original-record".into(),
            authority_contract: original.contract_id.clone(),
            authority_grant: original.grant_id.clone(),
            maximum_bytes: 4096,
            maximum_items: Some(1),
            availability: ResourceReferenceAvailability::Available,
        };
        let binding = ResourceBinding {
            pool_id: "fixture/original-record".into(),
            class_id: reference.access_class.clone(),
            units: 1,
            compute: None,
            protected: None,
            content: Some(ResourceContentOffer {
                contract: ResourceContentRequirement {
                    identity: reference.identity,
                    version: reference.lifetime.version,
                    content_profile: reference.content_profile.clone(),
                    maximum_bytes: 4096,
                    maximum_items: 1,
                    retention: ResourceRetention::ExternalDurable,
                    sharing: ResourceSharing::SingleWriterPublished,
                    access: ResourceAccessMode::WriteCandidatePublish,
                    generation_slots: 1,
                    reader_leases: 1,
                    publication_slots: 1,
                    sensitive: true,
                },
                owner_host: host.clone(),
                owner_boot: boot.clone(),
                base_id: "fixture/original-record".into(),
                residence_profile: kind_id("linux/snapshot@1"),
            }),
        };
        let call = HostCallRequirement {
            contract_id: original.host_call_contract_id.clone(),
            target_kind: Some(kind.clone()),
            maximum_in_flight: 1,
            maximum_input_bytes: 4096,
            maximum_output_bytes: 512,
        };
        Self {
            host,
            boot,
            kind,
            capability,
            binding,
            call,
            authority,
            reference,
            original,
            local,
        }
    }
    fn selected(&self) -> conduit_linux_snapshot_store::SnapshotRecordSelection<'_> {
        conduit_linux_snapshot_store::SnapshotRecordSelection {
            host: &self.host,
            boot: &self.boot,
            kind: &self.kind,
            capability: &self.capability,
            binding: &self.binding,
            host_call: &self.call,
            authority: &self.authority,
        }
    }
}

/// Actual product selected-IMAGE preparation for ordinary Owner CPU proof.
/// The configured original files disappear before the caller starts a Play.
pub(crate) fn host_fixture(current: StdHostConfig) -> StdHost {
    host_fixture_json(current, original_json())
}

/// Full original generation; no generic Data16 conversion or tail truncation.
pub(crate) fn host_fixture_64(current: StdHostConfig) -> StdHost {
    let contract = ObservationContract::prepare().unwrap();
    let (host, boot, plan, play) = (
        "host/reported".into(),
        "boot/reported".into(),
        "plan/reported".into(),
        "play/reported".into(),
    );
    let mut chunk = PreparedRecordingChunk::prepare(
        RecordingContext {
            host: &host,
            boot: &boot,
            plan: &plan,
            play: &play,
        },
        &contract,
    )
    .unwrap();
    let mut forms = PreparedObservationQuantityForms::prepare(&contract).unwrap();
    let observation = forms.unpack(&[198, 0, 78, 2]).unwrap();
    for index in 0..64_u64 {
        let status = if index % 2 == 0 {
            RecordingDisposition::Available
        } else {
            RecordingDisposition::Checksum
        };
        chunk
            .append(
                2 * index + 1,
                index * index * 7 + 11,
                status,
                (status == RecordingDisposition::Available).then_some(observation),
            )
            .unwrap();
    }
    host_fixture_json(current, chunk.seal().unwrap().into())
}

fn host_fixture_json(current: StdHostConfig, json: String) -> StdHost {
    let root = Directory::new();
    let directory = root.open();
    let mut original = current.clone();
    original.boot_id = "boot/original-record-write".into();
    let write = WriteFacts::for_host(json.len(), original);
    PreparedFileSnapshot::prepare_selected(
        &directory,
        write.selected(),
        &write.reference,
        &write.original,
        &write.local,
    )
    .unwrap()
    .publish(&write.authority, json.as_bytes())
    .unwrap();
    let (checked, image) = equipment(&root.0, &write.reference);
    let selected = selection(&checked, &image);
    let host = selected
        .initialize_image(
            current.clone(),
            #[cfg(all(target_os = "linux", feature = "native-display"))]
            None,
        )
        .unwrap();
    assert_eq!(host.advertisement().host_id, current.host_id);
    assert_eq!(host.advertisement().boot_id, current.boot_id);
    drop(directory);
    drop(root);
    host
}
