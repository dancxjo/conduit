//! Native sample files -> ordinary catalog publication/Body State -> fresh Boot.
//! Body admission, producer authentication and storage grants remain CPU fixtures.
use super::*;
use conduit_environment_make::PreparedRecordingCatalogCheckpoint;
use conduit_std_host as host;
#[path = "../support/catalog_host.rs"]
mod catalog_host;
#[path = "../recording_recovery/checkpoint_plan.rs"]
mod checkpoint_plan;

pub(super) fn check(
    directory: &conduit_linux_snapshot_store::SnapshotDirectory,
    references: &[c::BoundedResourceRef],
    observers: &[RecordingCommitObserver],
) {
    let catalog =
        PreparedRecordingCatalog::prepare_committed(references, observers, 3, 16384).unwrap();
    let mut selected = fixture::reference();
    selected.identity = c::ResourceSemanticIdentity::from_digest([99; 32]);
    selected.lifetime.version = c::ResourceVersionIdentity::from_digest([201; 32]);
    selected.extent.bytes = catalog.content().len() as u64;
    let checkpoint = PreparedRecordingCatalogCheckpoint::prepare_after_publications(
        selected.clone(),
        references,
        observers,
        3,
        16384,
    )
    .unwrap();
    assert_eq!(checkpoint.content(), catalog.content());
    let state = tempfile::tempdir().unwrap();
    let body: conduit_body::BodyId = serde_json::from_str("\"fixture/native-recording\"").unwrap();
    let (mut writer, plan, expected) = checkpoint_plan::planned_checkpoint(
        checkpoint.source(),
        checkpoint.reference(),
        "fixture/native-catalog-write-boot",
    );
    let storage_host = plan.fragments[0].host_id.clone();
    let control = host::RunControl::default();
    let mut current = catalog_host::WriteCurrent {
        expected,
        control: control.clone(),
    };
    let report = writer
        .run_body_durable_snapshot_plot_to(
            &body,
            state.path(),
            directory,
            plan.fragments[0].clone(),
            host::ExternalForeRun {
                inputs: &[host::ExternalForeInput {
                    front_port_id: "catalog".into(),
                    track: c::ConnectionTrack::Payload,
                    bytes: checkpoint.content().as_bytes().to_vec(),
                }],
                output: &mut current,
                sequential: false,
                stream: None,
            },
            &mut std::io::sink(),
            &mut catalog_host::NoWait,
            &control,
        )
        .unwrap();
    assert_eq!(report.control_receipts.len(), 1);
    drop(writer);
    let seed = std::fs::read_dir(state.path())
        .unwrap()
        .map(|entry| entry.unwrap().path().join("current"))
        .find(|path| path.is_file())
        .unwrap();
    let before = std::fs::read(&seed).unwrap();
    let (mut reader, read_plan, _) =
        catalog_host::planned(false, "fixture/native-catalog-restart-boot");
    let recovered = conduit_environment_linux::recover_durable_catalog_seed(
        &mut reader,
        &body,
        state.path(),
        read_plan.fragments[0].clone(),
        &mut catalog_host::NoWait,
    )
    .unwrap()
    .unwrap();
    assert_eq!(recovered, selected);
    assert_eq!(std::fs::read(&seed).unwrap(), before);
    let (mut read, _) = file_fixture::placement(
        "fixture/native-catalog-read-boot",
        false,
        selected.extent.bytes as usize,
    );
    read.host_id = storage_host;
    read.authority[0].host_id = read.host_id.clone();
    let resource = read.resources[0].content.as_mut().unwrap();
    resource.owner_host = read.host_id.clone();
    resource.contract.identity = selected.identity;
    resource.contract.version = selected.lifetime.version;
    let mut reader = conduit_environment_linux::PreparedFileCatalogRecovery::prepare(
        directory, &read, &recovered,
    )
    .unwrap();
    let decoded = reader.read(&read.authority[0]).unwrap();
    assert_eq!(decoded.len(), references.len());
    for (index, reference) in references.iter().enumerate() {
        assert_eq!(
            c::BoundedResourceRef::decode(decoded.encoded_reference(index).unwrap()).unwrap(),
            *reference
        );
    }
    eprintln!("native_catalog_checkpoint_cpu chunk_generations=2 catalog_published=true body_seed_committed=true fresh_boot_discovery=true admission_fixture=true");
}
