#![cfg(target_os = "linux")]
//! Real Linux file effects and production kernel commits, with independently
//! explicit CPU placement/authority/offer fixtures. This does not install a
//! native Host, grant Body membership, test power loss, or recover observers in
//! another process. Directory and sealed binding fixtures are reused unchanged.
mod common;
#[path = "../../../../targets/std/resource-snapshot/tests/support/directory.rs"]
mod directory;
#[path = "../../../../mechanisms/io/resource-snapshot/tests/support/fixture.rs"]
mod fixture;
#[path = "file_history/kernel.rs"]
mod kernel;
use common::{basis, chunk, context};
use conduit_environment_history::{HistoryGraph, PreparedCommittedHistoryFace, HISTORY_SUBJECT};
use conduit_environment_runtime::{
    observation::ObservationContract,
    recording::{PreparedRecordingSnapshot, RecordingSnapshotBack},
};
use conduit_face::{Face, FaceBasis, FaceFragment, FacePropertyValue as V};
use conduit_linux_snapshot_store::{FileSnapshotRefusal, PreparedFileSnapshot};
use conduit_resource_snapshot::{PreparedSnapshotRecord, PUBLISH_OPERATION, READ_OPERATION};
use directory::Root;
fn face(fragment: FaceFragment) -> Face {
    Face::new_with_semantics(
        1,
        FaceBasis {
            body_id: None,
            wake_id: None,
            source_document_id: Some("source/linux-file-history-fixture".into()),
            checked_plot_id: Some(basis().checked_plot_id),
            expanded_plot_id: Some("expanded/linux-file-history-fixture".into()),
            plan_id: Some(basis().plan_id),
            active_play_id: Some(basis().active_play_id),
            sign_ids: vec![],
        },
        fragment.subjects,
        fragment.relationships,
        fragment.properties,
        vec![],
        vec![],
        vec![],
    )
    .unwrap()
}
fn property<'a>(f: &'a FaceFragment, name: &str) -> Option<&'a V> {
    f.properties
        .iter()
        .find(|p| p.subject == HISTORY_SUBJECT && p.name == name)
        .map(|p| &p.value)
}
#[test]
fn actual_file_publish_kernel_commit_reopen_and_both_masks_share_exact_recording() {
    let root = Root::prepare();
    let directory = root.directory();
    let contract = ObservationContract::prepare().unwrap();
    let content = chunk(&[(1, 100, true), (2, 200, false), (4, 900, true)]);
    let extent = PreparedRecordingSnapshot::content_bytes(context(), &contract, 3).unwrap();
    assert_eq!(extent, content.len());
    let (write, reference) = fixture::placement("boot/file-publisher", true, extent);
    let mut operation = PreparedFileSnapshot::prepare(&directory, &write, &reference).unwrap();
    let (back, observer) =
        RecordingSnapshotBack::prepare(context(), &contract, 3, &reference).unwrap();
    let observers = [observer.clone()];
    let mut history =
        PreparedCommittedHistoryFace::prepare(context(), &contract, &observers).unwrap();
    let encoded = reference.encode().unwrap();
    kernel::execute(back, &observer, &content, &encoded, &mut history, |bytes| {
        operation.publish(&write.authority[0], bytes)
    })
    .unwrap();
    let fragment = history.contribution(basis()).unwrap();
    assert_eq!(history.recovered().samples, 3);
    assert_eq!(
        property(&fragment, "publication-evidence"),
        Some(&V::Text("kernel-publication-commit".into()))
    );
    assert_eq!(
        property(&fragment, "last-publication-reference"),
        Some(&V::Content(encoded.clone()))
    );
    let semantic = face(fragment);
    let graph = HistoryGraph::prepare(&semantic, HISTORY_SUBJECT).unwrap();
    assert_eq!(graph.samples()[0].temperature_milli_c(), Some(19800));
    assert_eq!(graph.samples()[0].humidity_milli_percent(), Some(59000));
    assert!(graph.samples()[1].failure());
    assert_eq!(graph.samples()[2].gap_before(), Some((3, 3)));
    let html = graph.browser_html();
    let native = graph.native_show().unwrap();
    assert!(html.contains("missing sequences 3 through 3"));
    assert_eq!(native.content_id, semantic.identity.as_str());
    assert_eq!(native.revision, semantic.revision);
    assert!(native.scene.commands().len() <= 48);
    // Same retained resource identity/version, independently prepared authority
    // for a new Boot, and a new descriptor to the real directory.
    let (read, reopened_reference) = fixture::placement("boot/file-reader-new", false, extent);
    assert_eq!(reopened_reference.encode().unwrap(), encoded);
    let reopened_directory = root.directory();
    let mut reopened =
        PreparedFileSnapshot::prepare(&reopened_directory, &read, &reference).unwrap();
    let restored = reopened.read(&read.authority[0]).unwrap();
    assert_eq!(restored, content.as_bytes());
    assert!(observer.committed_content_matches(restored));
    // New view, using the same process-local selected observer: file reopening
    // alone cannot reconstruct a commit observer in a fresh process.
    let mut restored_history =
        PreparedCommittedHistoryFace::prepare(context(), &contract, &observers).unwrap();
    restored_history
        .read_next(std::str::from_utf8(restored).unwrap())
        .unwrap();
    let restored_face = face(restored_history.contribution(basis()).unwrap());
    let restored_graph = HistoryGraph::prepare(&restored_face, HISTORY_SUBJECT).unwrap();
    assert_eq!(restored_graph.samples(), graph.samples());
    assert_eq!(restored_graph.browser_html(), html);
    assert_eq!(restored_graph.native_show().unwrap().scene, native.scene);
    assert_eq!(std::fs::read_dir(&root.path).unwrap().count(), 1);
}
#[test]
fn occupied_generation_fails_original_request_without_replacing_file_or_visible_history() {
    let root = Root::prepare();
    let directory = root.directory();
    let contract = ObservationContract::prepare().unwrap();
    let original = chunk(&[(1, 100, true), (2, 200, true)]);
    let replacement = chunk(&[(1, 101, true), (2, 201, true)]);
    let extent = PreparedRecordingSnapshot::content_bytes(context(), &contract, 2).unwrap();
    let (write, reference) = fixture::placement("boot/file-publisher", true, extent);
    let mut first_operation =
        PreparedFileSnapshot::prepare(&directory, &write, &reference).unwrap();
    let (first_back, first_observer) =
        RecordingSnapshotBack::prepare(context(), &contract, 2, &reference).unwrap();
    let first_observers = [first_observer.clone()];
    let mut first_history =
        PreparedCommittedHistoryFace::prepare(context(), &contract, &first_observers).unwrap();
    let encoded = reference.encode().unwrap();
    kernel::execute(
        first_back,
        &first_observer,
        &original,
        &encoded,
        &mut first_history,
        |bytes| first_operation.publish(&write.authority[0], bytes),
    )
    .unwrap();
    let accepted = first_history.contribution(basis()).unwrap();
    let record = PreparedSnapshotRecord::prepare(&write, &reference).unwrap();
    let path = root
        .path
        .join(record.storage_key().strip_prefix("resource/").unwrap());
    let original_record = std::fs::read(&path).unwrap();
    let mut duplicate_operation =
        PreparedFileSnapshot::prepare(&directory, &write, &reference).unwrap();
    let (duplicate_back, duplicate_observer) =
        RecordingSnapshotBack::prepare(context(), &contract, 2, &reference).unwrap();
    let duplicate_observers = [duplicate_observer.clone()];
    let mut duplicate_history =
        PreparedCommittedHistoryFace::prepare(context(), &contract, &duplicate_observers).unwrap();
    assert!(matches!(
        kernel::execute(
            duplicate_back,
            &duplicate_observer,
            &replacement,
            &encoded,
            &mut duplicate_history,
            |bytes| duplicate_operation.publish(&write.authority[0], bytes)
        ),
        Err(FileSnapshotRefusal::PublishedNameExists)
    ));
    assert_eq!(duplicate_history.recovered().samples, 0);
    assert_eq!(duplicate_observer.committed(), None);
    assert_eq!(first_history.contribution(basis()).unwrap(), accepted);
    assert_eq!(std::fs::read(&path).unwrap(), original_record);
    let (read, _) = fixture::placement("boot/file-reader-new", false, extent);
    let mut reader = PreparedFileSnapshot::prepare(&directory, &read, &reference).unwrap();
    assert_eq!(
        reader.read(&read.authority[0]).unwrap(),
        original.as_bytes()
    );
}
#[test]
fn stale_file_authority_is_denied_without_publication_or_history_commit() {
    let root = Root::prepare();
    let directory = root.directory();
    let contract = ObservationContract::prepare().unwrap();
    let content = chunk(&[(1, 100, true)]);
    let extent = PreparedRecordingSnapshot::content_bytes(context(), &contract, 1).unwrap();
    let (write, reference) = fixture::placement("boot/current", true, extent);
    let mut operation = PreparedFileSnapshot::prepare(&directory, &write, &reference).unwrap();
    let mut stale = write.authority[0].clone();
    stale.boot_id = "boot/stale".into();
    let (back, observer) =
        RecordingSnapshotBack::prepare(context(), &contract, 1, &reference).unwrap();
    let observers = [observer.clone()];
    let mut history =
        PreparedCommittedHistoryFace::prepare(context(), &contract, &observers).unwrap();
    assert!(matches!(
        kernel::execute(
            back,
            &observer,
            &content,
            &reference.encode().unwrap(),
            &mut history,
            |bytes| operation.publish(&stale, bytes)
        ),
        Err(FileSnapshotRefusal::Binding(
            conduit_resource_snapshot::SnapshotRefusal::StaleBoot
        ))
    ));
    assert_eq!(history.recovered().samples, 0);
    assert_eq!(observer.committed(), None);
    assert_eq!(std::fs::read_dir(&root.path).unwrap().count(), 0);
}
