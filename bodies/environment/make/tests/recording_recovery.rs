//! Production Source, retained software and ordinary planner. External store
//! residence/read service, authority and current identities are CPU fixtures.
use conduit_core as c;
use conduit_environment_make::{
    PreparedRecordingRecovery, RecordingPlanningRefusal as E, RecordingRecoverySource,
};
use conduit_environment_runtime::{observation::ObservationContract, recording::*};
use std::sync::LazyLock;
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
#[path = "recording_recovery/admission.rs"]
mod admission;
#[path = "recording_recovery/execution.rs"]
mod execution;
fn context() -> RecordingContext<'static> {
    static HOST: LazyLock<c::HostId> = LazyLock::new(|| "fixture/producer".into());
    static BOOT: LazyLock<c::BootId> = LazyLock::new(|| "fixture/producer/old-boot".into());
    static PLAN: LazyLock<c::PlanId> = LazyLock::new(|| "fixture/producer/plan".into());
    static PLAY: LazyLock<c::ActivePlayId> = LazyLock::new(|| "fixture/producer/play".into());
    RecordingContext {
        host: &HOST,
        boot: &BOOT,
        plan: &PLAN,
        play: &PLAY,
    }
}
fn bases() -> [c::BaseImplementationId; 1] {
    ["conduit.base/local@1".into()]
}
fn reference() -> c::BoundedResourceRef {
    c::BoundedResourceRef {
        identity: c::ResourceSemanticIdentity::from_digest([1; 32]),
        content_profile: "text/json-utf8@1".into(),
        access_class: "resource/snapshot@1".into(),
        extent: c::ResourceExtent {
            bytes: 1024,
            items: Some(1),
        },
        lifetime: c::ResourceLifetime {
            version: c::ResourceVersionIdentity::from_digest([2; 32]),
            expires_at: None,
        },
    }
}
fn fixture_store(reference: &c::BoundedResourceRef) -> (c::HostAdvertisement, c::AuthorityGrant) {
    let host: c::HostId = "fixture/recorder".into();
    let boot: c::BootId = "fixture/recorder/new-boot".into();
    let kind = conduit_semantic_catalog::resource_snapshot_semantic_contract(false);
    let contract = c::ResourceContentRequirement {
        identity: reference.identity,
        version: reference.lifetime.version,
        content_profile: reference.content_profile.clone(),
        maximum_bytes: 4096,
        maximum_items: 1,
        retention: c::ResourceRetention::ExternalDurable,
        sharing: c::ResourceSharing::SingleWriterPublished,
        access: c::ResourceAccessMode::ReadPublished,
        generation_slots: 1,
        reader_leases: 1,
        publication_slots: 0,
        sensitive: false,
    };
    let mut offer = c::BackOfferBuilder::new(
        kind.clone(),
        c::Back {
            capability_id: "fixture/snapshot/read".into(),
            execution_profile_id: "fixture/snapshot/read@1".into(),
            implementation_id: "fixture/snapshot/read@1".into(),
            artifact_id: "fixture/snapshot/read@1".into(),
            host_calls: vec![c::HostCallRequirement {
                contract_id: "conduit.host/resource-snapshot-read@1".into(),
                target_kind: Some(kind.kind_id.clone()),
                maximum_in_flight: 1,
                maximum_input_bytes: 512,
                maximum_output_bytes: 4096,
            }],
            resource_requirements: vec![c::ResourceRequirement {
                class_id: reference.access_class.clone(),
                units: 1,
                compute: None,
                protected_role: None,
                content: Some(contract.clone()),
            }],
            authority_requirements: vec![c::AuthorityRequirement {
                contract_id: "authority/resource-snapshot@1".into(),
                host_call_contract_id: "conduit.host/resource-snapshot-read@1".into(),
                subject_kind: kind.kind_id.clone(),
            }],
        },
    )
    .build();
    offer.limits.max_queue_bytes = 4096;
    let grant = c::AuthorityGrant {
        grant_id: "fixture/snapshot/grant".into(),
        contract_id: "authority/resource-snapshot@1".into(),
        host_call_contract_id: "conduit.host/resource-snapshot-read@1".into(),
        subject_kind: kind.kind_id,
        host_id: host.clone(),
        boot_id: boot.clone(),
        capability_id: offer.capability_id.clone(),
    };
    (
        c::HostAdvertisement {
            protocol_version: c::PROTOCOL_VERSION,
            host_id: host.clone(),
            boot_id: boot.clone(),
            offer_generation: c::OfferGeneration(1),
            profile: "fixture/recorder".into(),
            bases: vec![],
            resources: vec![c::ResourceOffer {
                pool_id: "fixture/snapshot/pool".into(),
                class_id: reference.access_class.clone(),
                capacity_units: 1,
                compute: None,
                content: Some(c::ResourceContentOffer {
                    contract,
                    owner_host: host,
                    owner_boot: boot,
                    base_id: "fixture/snapshot/base".into(),
                    residence_profile: "fixture/snapshot@1".into(),
                }),
            }],
            capabilities: vec![offer],
            planner_capabilities: vec![],
        },
        grant,
    )
}
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
