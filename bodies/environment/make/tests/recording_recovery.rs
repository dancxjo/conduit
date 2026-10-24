//! Production Source, retained software and ordinary planner. External store
//! residence/read service, authority and current identities are CPU fixtures.
use conduit_core as c;
use conduit_environment_make::{
    PreparedRecordingRecovery, RecordingPlanningRefusal as E, RecordingRecoverySource,
};
use conduit_environment_runtime::{observation::ObservationContract, recording::*};
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
#[path = "recording_recovery/admission.rs"]
mod admission;
#[path = "recording_recovery/execution.rs"]
mod execution;
#[path = "common/recording_fixture.rs"]
mod recording_fixture;
use recording_fixture::{bases, context, fixture_store, reference};
fn software(
    source: &RecordingRecoverySource,
    host: &mut c::HostAdvertisement,
) -> (
    ReadyRecordingGenerations,
    ReadyRecordingRecovery,
    ReadyRecordingSingleton,
) {
    let mut generations = ReadyRecordingGenerations::prepare_generation(
        source.expanded(),
        host.host_id.clone(),
        host.boot_id.clone(),
        source.reference(),
    )
    .unwrap();
    let mut singleton = ReadyRecordingSingleton::prepare_singleton(
        source.expanded(),
        host.host_id.clone(),
        host.boot_id.clone(),
    )
    .unwrap();
    singleton.append_to_advertisement(host).unwrap();
    let mut recovery = ReadyRecordingRecovery::prepare(
        source.expanded(),
        host.host_id.clone(),
        host.boot_id.clone(),
        context(),
        &ObservationContract::prepare().unwrap(),
        1,
    )
    .unwrap();
    generations.append_to_advertisement(host).unwrap();
    recovery.append_to_advertisement(host).unwrap();
    (generations, recovery, singleton)
}
#[test]
fn checked_generation_read_and_recovery_use_independent_resource_and_authority() {
    let reference = reference();
    let source = RecordingRecoverySource::compile(&reference).unwrap();
    assert_eq!(
        (
            source.expanded().gears.len(),
            source.expanded().connections.len()
        ),
        (4, 3)
    );
    let (mut host, grant) = fixture_store(&reference);
    let (mut generations, mut recovery, mut singleton) = software(&source, &mut host);
    assert!(matches!(
        PreparedRecordingRecovery::prepare(&source, &host, &bases(), &[]),
        Err(E::Planning(
            conduit_planner::PlannerError::AuthorityGrantMissing(_)
        ))
    ));
    let prepared = PreparedRecordingRecovery::prepare(&source, &host, &bases(), &[grant]).unwrap();
    assert!(c::verify_plan(prepared.plan()));
    let fragment = &prepared.plan().fragments[0];
    let read = fragment
        .placements
        .iter()
        .find(|g| g.kind_id.as_str() == conduit_semantic_catalog::SNAPSHOT_READ_KIND)
        .unwrap();
    assert_eq!(
        read.resources[0].content.as_ref().unwrap().contract.version,
        reference.lifetime.version
    );
    assert_eq!(read.authority.len(), 1);
    let mut capacities: Vec<_> = prepared
        .lowered()
        .cords
        .iter()
        .map(|cord| cord.spec.byte_capacity)
        .collect();
    capacities.sort();
    assert_eq!(capacities, [512, 4096, 4096]);
    for gear in &fragment.placements {
        match gear.kind_id.as_str() {
            "environment/recording-generation" => {
                generations
                    .take_selected(fragment, &gear.placement_id)
                    .unwrap();
            }
            "environment/recover-recording" => {
                recovery
                    .take_selected(fragment, &gear.placement_id)
                    .unwrap();
            }
            "flow/singleton" => {
                singleton
                    .take_selected(fragment, &gear.placement_id)
                    .unwrap();
            }
            _ => {}
        }
    }
    assert_eq!(prepared.body_plot_plan().plan, *prepared.plan());
}
#[test]
fn wrong_generation_or_absent_resource_cannot_become_a_recovery_plan() {
    let reference = reference();
    let source = RecordingRecoverySource::compile(&reference).unwrap();
    let (mut host, grant) = fixture_store(&reference);
    let _owners = software(&source, &mut host);
    let mut absent = host.clone();
    absent.resources.clear();
    assert!(PreparedRecordingRecovery::prepare(
        &source,
        &absent,
        &[],
        core::slice::from_ref(&grant)
    )
    .is_err());
    let mut changed = reference.clone();
    changed.lifetime.version = c::ResourceVersionIdentity::from_digest([3; 32]);
    let changed = RecordingRecoverySource::compile(&changed).unwrap();
    // Reinitialize real software for the changed Source; keep the old store's
    // generation so its independently selected content cannot satisfy the read.
    host.capabilities
        .retain(|offer| offer.kind_id.as_str() == conduit_semantic_catalog::SNAPSHOT_READ_KIND);
    let _changed_owners = software(&changed, &mut host);
    assert!(matches!(
        PreparedRecordingRecovery::prepare(&changed, &host, &bases(), &[grant]),
        Err(E::ReferenceBinding)
    ));
}

#[path = "recording_recovery/batch_source.rs"]
mod batch_source;

#[path = "recording_recovery/batch_plan.rs"]
mod batch_plan;

#[path = "recording_recovery/batch_execution.rs"]
mod batch_execution;

#[cfg(feature = "browser-storage-proof")]
#[path = "recording_recovery/batch_browser.rs"]
mod batch_browser;

#[cfg(feature = "browser-storage-proof")]
#[path = "recording_recovery/browser_storage.rs"]
mod browser_storage;

#[path = "recording_recovery/software_preparation.rs"]
mod software_preparation;

#[path = "recording_recovery/body_lifecycle.rs"]
mod body_lifecycle;

#[cfg(target_os = "linux")]
#[path = "recording_recovery/linux_files.rs"]
mod linux_files;

#[cfg(target_os = "linux")]
#[path = "recording_recovery/linux_batch_files.rs"]
mod linux_batch_files;

#[cfg(target_os = "linux")]
#[path = "recording_recovery/linux_files_publication.rs"]
mod linux_files_publication;
#[path = "recording_recovery/linux_files_streaming_publication.rs"]
mod linux_files_streaming_publication;

#[cfg(all(target_os = "linux", feature = "installed-recording-proof"))]
#[path = "recording_recovery/linux_files_checkpoint.rs"]
mod linux_files_checkpoint;
