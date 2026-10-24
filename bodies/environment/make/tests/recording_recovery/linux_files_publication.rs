//! Actual checked Source/lowering/kernel publication; no injected completions.
//! Host/Body admission, resource placement and producer context remain fixtures.
use super::linux_files::directory;
use super::*;
use conduit_environment_linux::{FilePublicationRefusal, PreparedFileRecordingPublication};
use conduit_kernel::scheduler::{RemoteIngressOutcome, SchedulerStatus};
#[path = "publication/plan.rs"]
pub(super) mod plan;
#[derive(Clone, Copy, Debug)]
enum Mode {
    Published,
    Occupied,
    CancelAfterFile,
}
#[test]
fn linux_files_publication_uses_original_requests_and_committed_observers() {
    for mode in [Mode::Published, Mode::Occupied, Mode::CancelAfterFile] {
        execute(mode);
    }
}
fn execute(mode: Mode) {
    let contract = ObservationContract::prepare().unwrap();
    let mut selected = reference();
    selected.extent.bytes =
        PreparedRecordingSnapshot::content_bytes(context(), &contract, 2).unwrap() as u64;
    let encoded = selected.encode().unwrap();
    let mut generation = Some(
        PreparedRecordingBatch::prepare(
            context(),
            &contract,
            2,
            core::slice::from_ref(&selected),
            1,
            4096,
        )
        .unwrap()
        .into_generations()
        .pop()
        .unwrap(),
    );
    let mut quantities =
        conduit_environment_runtime::observation::PreparedObservationQuantityForms::prepare(
            &contract,
        )
        .unwrap();
    let observation = quantities.unpack(&[198, 0, 78, 2]).unwrap();
    let encoder = &mut generation.as_mut().unwrap().encoder;
    encoder
        .append(
            1,
            60_000,
            RecordingDisposition::Available,
            Some(observation),
        )
        .unwrap();
    encoder
        .append(2, 120_000, RecordingDisposition::Checksum, None)
        .unwrap();
    let content = encoder.seal().unwrap().as_bytes().to_vec();
    let (plan, lowered) = plan::plan(&selected);
    let fragment = &plan.fragments[0];
    let active = c::bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 1);
    let foreign = c::bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 2);
    let root = directory::Root::prepare();
    let directory = root.directory();
    if matches!(mode, Mode::Occupied) {
        conduit_linux_snapshot_store::PreparedFileSnapshot::prepare(
            &directory,
            &fragment.placements[0],
            &selected,
        )
        .unwrap()
        .publish(&fragment.placements[0].authority[0], &content)
        .unwrap();
    }
    let mut owner = PreparedFileRecordingPublication::prepare(
        &directory,
        fragment,
        &lowered,
        &active,
        &mut generation,
    )
    .unwrap();
    assert!(generation.is_none());
    assert!(matches!(
        owner.admit(&foreign),
        Err(FilePublicationRefusal::Scope)
    ));
    assert!(matches!(
        owner.step(&foreign),
        Err(FilePublicationRefusal::Scope)
    ));
    assert!(matches!(
        owner.service_next(&foreign),
        Err(FilePublicationRefusal::Scope)
    ));
    assert!(matches!(
        owner.take_reference(&foreign),
        Err(FilePublicationRefusal::Scope)
    ));
    let mut requests = 0;
    let mut delivered = 0;
    let mut terminal = false;
    let mut refused = false;
    let (_, memory) = allocation_probe::observe(|| {
        assert!(matches!(
            owner.admit(&active).unwrap(),
            RemoteIngressOutcome::Accepted { sequence: 0 }
        ));
        for _ in 0..40 {
            if let Some(completion) = owner.service_next(&active).unwrap() {
                requests += 1;
                assert_eq!(
                    owner.observer().committed(),
                    None,
                    "file effect is not a Back commit"
                );
                if completion.refusal.is_some() {
                    refused = true;
                }
                if matches!(mode, Mode::CancelAfterFile) {
                    assert_eq!(
                        completion.disposition,
                        conduit_kernel::HostCallDisposition::Completed
                    );
                    owner.cancel(&active).unwrap();
                    terminal = true;
                    break;
                }
            }
            if let Some(bytes) = owner.take_reference(&active).unwrap() {
                assert_eq!(bytes, encoded);
                delivered += 1;
            }
            match owner.step(&active) {
                Ok(SchedulerStatus::Drained) => {
                    terminal = true;
                    break;
                }
                Err(FilePublicationRefusal::Kernel(_)) => {
                    terminal = true;
                    assert!(matches!(mode, Mode::Occupied));
                    break;
                }
                Ok(_) => {}
                Err(error) => panic!("publication refusal: {error:?}"),
            }
        }
    });
    assert!(terminal, "publication mode {mode:?} did not terminate");
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    assert_eq!(requests, 1);
    let catalog = PreparedRecordingCatalog::prepare_committed(
        core::slice::from_ref(&selected),
        core::slice::from_ref(owner.observer()),
        2,
        8192,
    );
    if matches!(mode, Mode::Published) {
        assert_eq!(delivered, 1);
        assert!(!refused);
        assert_eq!(
            owner.observer().committed(),
            Some(RecordedRange {
                first_sequence: 1,
                last_sequence: 2,
                samples: 2
            })
        );
        let catalog = catalog.unwrap();
        assert!(!catalog.content().is_empty());
        let mut catalog_reference = selected.clone();
        catalog_reference.identity = c::ResourceSemanticIdentity::from_digest([42; 32]);
        catalog_reference.extent.bytes = catalog.content().len() as u64;
        let checkpoint = conduit_environment_make::PreparedRecordingCatalogCheckpoint::prepare_after_publications(
            catalog_reference, core::slice::from_ref(&selected),
            core::slice::from_ref(owner.observer()), 2, 8192,
        ).unwrap();
        assert_eq!(checkpoint.content(), catalog.content());
        owner.cancel(&active).unwrap();
        assert!(owner.observer().committed().is_some());
    } else {
        assert_eq!(delivered, 0);
        assert_eq!(owner.observer().committed(), None);
        assert_eq!(
            catalog.err(),
            Some(RecordingCatalogRefusal::UncommittedGeneration(0))
        );
        assert_eq!(refused, matches!(mode, Mode::Occupied));
    }
    // Reopen independently with a separate current read grant. Cancellation
    // after the effect cannot erase the file, and an occupied name is unchanged.
    let reopened = root.directory();
    let (mut read, _) = super::linux_files::publisher_fixture::placement(
        "fixture/recorder/read-boot",
        false,
        content.len(),
    );
    read.host_id = fragment.host_id.clone();
    read.authority[0].host_id = read.host_id.clone();
    let resource = read.resources[0].content.as_mut().unwrap();
    resource.owner_host = read.host_id.clone();
    resource.contract.identity = selected.identity;
    resource.contract.version = selected.lifetime.version;
    let mut reader =
        conduit_linux_snapshot_store::PreparedFileSnapshot::prepare(&reopened, &read, &selected)
            .unwrap();
    assert_eq!(reader.read(&read.authority[0]).unwrap(), content);
}

#[test]
fn linux_files_publication_preflight_retains_an_incomplete_generation_without_effects() {
    let contract = ObservationContract::prepare().unwrap();
    let mut selected = reference();
    selected.extent.bytes =
        PreparedRecordingSnapshot::content_bytes(context(), &contract, 2).unwrap() as u64;
    let mut generation = Some(
        PreparedRecordingBatch::prepare(
            context(),
            &contract,
            2,
            core::slice::from_ref(&selected),
            1,
            4096,
        )
        .unwrap()
        .into_generations()
        .pop()
        .unwrap(),
    );
    let (plan, lowered) = plan::plan(&selected);
    let fragment = &plan.fragments[0];
    let active = c::bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 1);
    let root = directory::Root::prepare();
    let directory = root.directory();
    let result = PreparedFileRecordingPublication::prepare(
        &directory,
        fragment,
        &lowered,
        &active,
        &mut generation,
    );
    assert!(matches!(
        result,
        Err(FilePublicationRefusal::Content(
            RecordingSnapshotRefusal::Incomplete
        ))
    ));
    assert!(generation.is_some());
    let (foreign, foreign_lowered) = plan::foreign_plan(&selected);
    let fragment = &foreign.fragments[0];
    let foreign_active =
        c::bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 1);
    assert!(matches!(
        PreparedFileRecordingPublication::prepare(
            &directory,
            fragment,
            &foreign_lowered,
            &foreign_active,
            &mut generation
        ),
        Err(FilePublicationRefusal::Selection)
    ));
    assert!(generation.is_some());
    assert_eq!(std::fs::read_dir(&root.path).unwrap().count(), 0);
}
