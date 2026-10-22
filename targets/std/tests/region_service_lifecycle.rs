#![cfg(target_os = "linux")]
//! Actual service/planner/ledger/worker modules with a minimal Host shell.
//! This does not exercise the full StdHost adapter or prove product overlap.
extern crate alloc;
extern crate self as conduit_planner;
use conduit_core::*;
#[path = "../../../architecture/planner/src/execution_regions.rs"]
pub mod execution_regions;
#[path = "../../../architecture/composite/tests/support/region_fixture.rs"]
mod fixture;
#[allow(dead_code, unused_imports)]
#[path = "support/region_lifecycle.rs"]
mod kernel_preparation;
use kernel_preparation::service::{InitializedRegionService, RegionAdvance, RegionServiceConfig};
struct StdHost {
    advertisement: HostAdvertisement,
    base_registry: BaseRegistry,
    kernel_resources: kernel_preparation::KernelResourceLedger,
    sequence: u64,
}
struct Issued(ActivePlayIdentity);
impl Issued {
    fn identity(&self) -> &ActivePlayIdentity {
        &self.0
    }
}
impl StdHost {
    // Fixture Host shell delegates identity to the canonical core binder.
    fn issue_kernel_play(&mut self, fragment: &PlanFragment) -> Result<Issued, String> {
        let sequence = self.sequence;
        self.sequence = sequence.checked_add(1).ok_or("sequence overflow")?;
        Ok(Issued(bind_active_play(
            &fragment.plan_id,
            &fragment.host_id,
            &fragment.boot_id,
            sequence,
        )))
    }
}
#[test]
fn public_service_selects_reserves_runs_and_withdraws_without_losing_custody() {
    let mut fragment = fixture::plan().fragments.remove(0);
    fragment.execution_regions.clear();
    let mut advertisement = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: fragment.host_id.clone(),
        boot_id: fragment.boot_id.clone(),
        offer_generation: fragment.offer_generation,
        profile: "fixture".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    for placement in &mut fragment.placements {
        placement.capability_id = format!("{}/offer", placement.placement_id.as_str()).into();
        advertisement.capabilities.push(capability_offer_from_parts! {
            semantic_contract: placement.semantic_contract.clone(), startup_parameters: vec![], shorthand: None,
            capability_id: placement.capability_id.clone(), kind_id: placement.kind_id.clone(), kind_contract_revision: placement.kind_contract_revision.clone(),
            implementation: ImplementationOffer { execution_profile_id: placement.execution_profile_id.clone(), implementation_id: placement.implementation_id.clone(), artifact_id: placement.artifact_id.clone() },
            inputs: placement.inputs.clone(), outputs: placement.outputs.clone(), host_calls: vec![], resource_requirements: vec![], authority_requirements: vec![], limits: placement.limits.clone(),
        });
    }
    let ledger = kernel_preparation::KernelResourceLedger::new(&advertisement).unwrap();
    let mut host = StdHost {
        advertisement,
        kernel_resources: ledger,
        sequence: 0,
        base_registry: BaseRegistry::new(BaseRegistryLimits {
            maximum_bases: 16,
            maximum_capabilities_per_base: 16,
            maximum_resources_per_base: 16,
            maximum_advertised_capabilities: 256,
            maximum_advertised_resources: 256,
        })
        .unwrap(),
    };
    let mut service =
        InitializedRegionService::initialize(&mut host, RegionServiceConfig::new(2)).unwrap();
    let current = service.advertisement().unwrap();
    assert_eq!(current.offer_generation, OfferGeneration(2));
    fragment.offer_generation = current.offer_generation;
    for p in &mut fragment.placements {
        p.offer_generation = current.offer_generation;
    }
    let original = seal_plan(
        PlotIdentity {
            source_document_id: fragment.source_document_id.clone(),
            checked_plot_id: fragment.checked_plot_id.clone(),
            expanded_plot_id: fragment.expanded_plot_id.clone(),
        },
        vec![fragment],
    );
    let groups = original.fragments[0]
        .placements
        .iter()
        .map(|p| vec![p.placement_id.clone()])
        .collect();
    let plan = service.select_plan_regions(original, groups, 4096).unwrap();
    let id = plan.plan_id.clone();
    let mut registry = conduit_composite::KernelOperationRegistry::new();
    registry
        .install(fixture::Factory {
            implementation: plan.fragments[0].placements[0].implementation_id.clone(),
        })
        .unwrap();
    service.prepare(plan, &registry).unwrap();
    assert!(service
        .current_resource_observations()
        .unwrap()
        .iter()
        .all(|o| o.unreserved_units == 0));
    let mut execution = service.into_execution().unwrap();
    assert_eq!(execution.active_play().plan_id, id);
    let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let mut rounds = 0;
    while rounds < 4 {
        match execution.advance().unwrap() {
            RegionAdvance::RoundComplete => rounds += 1,
            RegionAdvance::Pending => std::thread::yield_now(),
            RegionAdvance::Progress => {}
        }
        assert!(std::time::Instant::now() < until);
    }
    let retired = execution.shutdown();
    assert!(retired.host_error.is_none(), "{:?}", retired.host_error);
    assert_eq!(retired.active_play.unwrap().plan_id, id);
    assert_eq!(retired.workers.len(), 2);
    assert_eq!(retired.last_round.unwrap().open_transfers, 0);
    assert!(retired
        .workers
        .iter()
        .all(|w| w.failure.is_none() && w.plan_id.as_ref() == Some(&id)));
    assert!(host.kernel_resources.is_idle());
    assert_eq!(host.advertisement.offer_generation, OfferGeneration(3));
    assert_eq!(host.sequence, 1);
    assert!(host
        .base_registry
        .entries()
        .iter()
        .all(|b| b.lifecycle == BaseLifecycle::Stopped));
}
