//! Explicit CPU producer, storage offer and authority fixtures; no live Host.
use conduit_core as c;
use conduit_environment_runtime::recording::RecordingContext;
use std::sync::LazyLock;
pub(crate) fn context() -> RecordingContext<'static> {
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
pub(crate) fn bases() -> [c::BaseImplementationId; 1] {
    ["conduit.base/local@1".into()]
}
pub(crate) fn reference() -> c::BoundedResourceRef {
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
pub(crate) fn fixture_store(
    reference: &c::BoundedResourceRef,
) -> (c::HostAdvertisement, c::AuthorityGrant) {
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
// Current Host/resource/authority facts are explicit CPU fixtures.

use conduit_environment_make::RecordingPublicationSource;
use std::collections::BTreeMap;
pub(crate) fn plan(
    reference: &c::BoundedResourceRef,
) -> (
    c::Plan,
    conduit_plan_lowering::lowering::LoweredPlanFragment,
) {
    planned(reference, false)
}
pub(crate) fn foreign_plan(
    reference: &c::BoundedResourceRef,
) -> (
    c::Plan,
    conduit_plan_lowering::lowering::LoweredPlanFragment,
) {
    planned(reference, true)
}
fn planned(
    reference: &c::BoundedResourceRef,
    foreign: bool,
) -> (
    c::Plan,
    conduit_plan_lowering::lowering::LoweredPlanFragment,
) {
    let source = RecordingPublicationSource::compile(reference).unwrap();
    let (mut host, mut grant) = fixture_store(reference);
    let kind = conduit_semantic_catalog::resource_snapshot_semantic_contract(true);
    let content = host.resources[0].content.as_mut().unwrap();
    content.contract.access = c::ResourceAccessMode::WriteCandidatePublish;
    content.contract.publication_slots = 1;
    let base_id = content.base_id.clone();
    let mut offer =
        conduit_environment_linux::recording_publication_offer(&host.resources[0]).unwrap();
    if foreign {
        offer.implementation.execution_profile_id = "fixture/foreign-publisher@1".into();
        offer.implementation.implementation_id = "fixture/foreign-publisher@1".into();
        offer.implementation.artifact_id = "fixture/foreign-publisher@1".into();
    }
    grant.capability_id = offer.capability_id.clone();
    grant.subject_kind = kind.kind_id;
    grant.host_call_contract_id = conduit_linux_snapshot_store::PUBLISH_OPERATION.into();
    host.bases.push(c::BaseProviderAdvertisement {
        base_id,
        provider_instance_id: "fixture/file-provider".into(),
        provider_generation: 1,
        implementation_id: "conduit.base/local@1".into(),
        mechanism_family: "fixture/files".into(),
        enforcement_class: c::BaseEnforcementClass::Cooperative,
        lifecycle: c::BaseLifecycle::Ready,
        capability_ids: vec![offer.capability_id.clone()],
        resource_pool_ids: vec![host.resources[0].pool_id.clone()],
    });
    host.capabilities = vec![offer];
    let hosts = [host];
    let choices =
        conduit_planner::default_expanded_placements(&source.plot.expanded, &hosts).unwrap();
    let mut boundaries = BTreeMap::new();
    for (direction, bindings, byte_capacity) in [
        (c::PortDirection::Input, &source.plot.input_bindings, 4096),
        (c::PortDirection::Output, &source.plot.output_bindings, 512),
    ] {
        for binding in bindings {
            boundaries.insert(
                conduit_planner::ForeBoundaryKey {
                    direction,
                    front_port_id: binding.front_port_id.clone(),
                    track: binding.track,
                },
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity,
                },
            );
        }
    }
    let plan = conduit_planner::plan_expanded_authoring_with_options(
        &source.plot,
        &hosts,
        &choices,
        &bases(),
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 4096,
            authority_grants: &[grant],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap();
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(&plan.fragments[0]).unwrap();
    (plan, lowered)
}
