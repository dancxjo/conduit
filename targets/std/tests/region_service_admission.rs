#![cfg(target_os = "linux")]
//! Initialized-offer and accounting fixtures, not installed Play/overlap evidence.
use conduit_core::*;
use conduit_plan_lowering::execution_regions::*;
#[allow(dead_code)]
#[path = "support/region_accounting.rs"]
mod accounting;
#[allow(dead_code)]
#[path = "../../../architecture/composite/tests/support/region_fixture.rs"]
mod fixture;
#[allow(unused_imports)]
#[path = "../src/kernel_preparation/service/offers.rs"]
mod offers;
#[allow(dead_code)]
#[path = "../src/kernel_preparation/worker.rs"]
mod worker;
fn registry() -> BaseRegistry {
    BaseRegistry::new(BaseRegistryLimits {
        maximum_bases: 16,
        maximum_capabilities_per_base: 16,
        maximum_resources_per_base: 16,
        maximum_advertised_capabilities: 256,
        maximum_advertised_resources: 256,
    })
    .unwrap()
}
fn prior() -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: "host".into(),
        boot_id: "boot".into(),
        offer_generation: OfferGeneration(1),
        profile: "reference".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    }
}
#[test]
fn worker_offers_require_real_initialized_one_two_four_capacity_and_exact_generation() {
    for lanes in [1, 2, 4] {
        let config = offers::RegionServiceConfig::new(lanes);
        assert!(offers::initialize(&prior(), &registry(), config, &[]).is_err());
        let workers: Vec<_> = (0..lanes)
            .map(|_| {
                worker::InitializedRegionWorker::spawn(config.stack_bytes, config.transfer_bytes)
                    .unwrap()
            })
            .collect();
        let ready = offers::initialize(&prior(), &registry(), config, &workers).unwrap();
        assert_eq!(ready.advertisement.offer_generation, OfferGeneration(2));
        assert_eq!(ready.shutdown_generation, OfferGeneration(3));
        assert_eq!(ready.provider.resources[0].capacity_units, u32::from(lanes));
        assert_eq!(
            ready.provider.resources[0]
                .compute
                .as_ref()
                .unwrap()
                .service_guarantee,
            ComputeServiceGuarantee::Shared
        );
        assert_eq!(
            ready.provider.resources[1].capacity_units,
            u32::from(lanes) * config.worker_memory_bytes
        );
        assert_eq!(ready.advertisement.bases[0].lifecycle, BaseLifecycle::Ready);
        assert!(ready.control_backing_bytes > 0);
        assert!(ready
            .registry
            .entries()
            .iter()
            .any(|entry| entry.base_id == ready.provider.base_id
                && entry.lifecycle == BaseLifecycle::Ready));
        for worker in workers {
            assert!(worker.shutdown().unwrap().failure.is_none());
        }
    }
}
fn selected_plan(ready: &offers::ReadyOffers) -> (Plan, HostAdvertisement) {
    let mut fragment = fixture::plan().fragments.remove(0);
    let mut host = ready.advertisement.clone();
    fragment.offer_generation = host.offer_generation;
    for placement in &mut fragment.placements {
        placement.offer_generation = host.offer_generation;
        placement.limits.max_active_instances = 2;
        placement.capability_id = format!("{}/offer", placement.placement_id.as_str()).into();
        placement.kind_id = format!("fixture/{}", placement.placement_id.as_str()).into();
        placement.kind_contract_revision = format!("{}@1", placement.kind_id.as_str()).into();
        host.capabilities.push(capability_offer_from_parts! {
            semantic_contract: placement.semantic_contract.clone(), startup_parameters: vec![], shorthand: None,
            capability_id: placement.capability_id.clone(), kind_id: placement.kind_id.clone(), kind_contract_revision: placement.kind_contract_revision.clone(),
            implementation: ImplementationOffer { execution_profile_id: placement.execution_profile_id.clone(), implementation_id: placement.implementation_id.clone(), artifact_id: placement.artifact_id.clone() },
            inputs: placement.inputs.clone(), outputs: placement.outputs.clone(), host_calls: vec![], resource_requirements: vec![], authority_requirements: vec![], limits: placement.limits.clone(),
        });
    }
    for region in &mut fragment.execution_regions {
        region.execution_profile_id = offers::RegionServiceConfig::new(2).execution_profile_id();
        region.lane_base_id = ready.provider.base_id.clone();
        region.lane_resource.pool_id = ready.provider.resources[0].pool_id.clone();
        region
            .lane_resource
            .compute
            .as_mut()
            .unwrap()
            .architecture_base_id = ready.provider.base_id.as_str().into();
        let ExecutionScheduling::PartitionedBoundedStep { storage } = &mut region.scheduling else {
            panic!()
        };
        storage.stack_bytes = 2 * 1024 * 1024;
        storage.binding.pool_id = ready.provider.resources[1].pool_id.clone();
    }
    (
        seal_plan(
            PlotIdentity {
                source_document_id: fragment.source_document_id.clone(),
                checked_plot_id: fragment.checked_plot_id.clone(),
                expanded_plot_id: fragment.expanded_plot_id.clone(),
            },
            vec![fragment],
        ),
        host,
    )
}
#[test]
fn joint_worker_reservation_rolls_back_shortage_and_releases_instances_and_pools() {
    let config = offers::RegionServiceConfig::new(2);
    let workers: Vec<_> = (0..2)
        .map(|_| {
            worker::InitializedRegionWorker::spawn(config.stack_bytes, config.transfer_bytes)
                .unwrap()
        })
        .collect();
    let ready = offers::initialize(&prior(), &registry(), config, &workers).unwrap();
    let (plan, host) = selected_plan(&ready);
    let prepared = PreparedExecutionRegions::prepare(
        &plan,
        &plan.fragments[0].fragment_id,
        ExecutionRegionLimits {
            maximum_regions: 2,
            maximum_lanes: 2,
            maximum_nodes: 2,
            maximum_cords: 1,
        },
    )
    .unwrap();
    let mut ledger = accounting::KernelResourceLedger::new(&host).unwrap();
    let signs = [
        SignId::from("observed/lanes"),
        SignId::from("observed/memory"),
    ];
    let observations = ledger
        .observe_offers(&host, &ready.provider.resources, &signs)
        .unwrap();
    let mut shortage = observations.clone();
    shortage[0].unreserved_units = 1;
    assert!(ledger
        .reserve_worker_resources(&host, &prepared, &shortage)
        .is_err());
    assert!(ledger.is_idle());
    let held = ledger
        .reserve_worker_resources(&host, &prepared, &observations)
        .unwrap();
    assert!(!ledger.is_idle());
    // Even an old positive observation cannot overbook the live ledger.
    assert!(ledger
        .reserve_worker_resources(&host, &prepared, &observations)
        .is_err());
    ledger.release_region_resources(held).unwrap();
    assert!(ledger.is_idle());
    let mut stale = host.clone();
    stale.offer_generation = OfferGeneration(1);
    assert!(ledger
        .observe_offers(&stale, &ready.provider.resources, &signs)
        .is_err());
    for worker in workers {
        worker.shutdown().unwrap();
    }
}
