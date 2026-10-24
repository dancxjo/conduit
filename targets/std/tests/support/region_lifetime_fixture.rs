//! Existing finite transfer geometry with observable destruction of actual Backs.
use conduit_composite::{KernelOperationBudget, KernelOperationFactory, KernelOperationRegistry};
use conduit_core::*;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    CanonicalValue, HostedValueStore, PortId,
};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE as PORTS;
use conduit_std_host::StdHost;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
#[allow(dead_code)]
#[path = "../../../../architecture/composite/tests/support/region_fixture.rs"]
mod geometry;
#[derive(Default)]
pub struct Counts {
    pub dropped: AtomicUsize,
    pub consumed: AtomicUsize,
}
struct Back {
    source: bool,
    sent: bool,
    counts: Arc<Counts>,
}
impl Drop for Back {
    fn drop(&mut self) {
        self.counts.dropped.fetch_add(1, Ordering::SeqCst);
    }
}
impl StepBack<PORTS> for Back {
    fn step(&mut self, io: &mut StepIo<PORTS>, input: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.source {
            if self.sent {
                return StepOutcome::Complete;
            }
            io.send_canonical(PortId(0), CanonicalValue::new(&[42]).unwrap())
                .unwrap();
        } else if let Some(bytes) = input.input(PortId(0)) {
            assert_eq!(bytes, &[42]);
            io.consume(PortId(0)).unwrap();
            self.counts.consumed.fetch_add(1, Ordering::SeqCst);
        } else {
            return StepOutcome::Await;
        }
        StepOutcome::Progress
    }
    fn step_committed(&mut self) {
        if self.source {
            self.sent = true;
        }
    }
}
struct Factory {
    implementation: ImplementationId,
    counts: Arc<Counts>,
}
impl KernelOperationFactory for Factory {
    fn implementation_id(&self) -> &ImplementationId {
        &self.implementation
    }
    fn budget(&self, _: &PlannedGear) -> Result<KernelOperationBudget, String> {
        Ok(KernelOperationBudget {
            value_items: 1,
            value_bytes: 1,
            maximum_value_bytes: 1,
            host_requests: 0,
            sign_items: 8,
        })
    }
    fn prepare_worker(
        &self,
        placement: &PlannedGear,
        values: &mut HostedValueStore,
    ) -> Result<conduit_composite::PreparedWorkerBack, String> {
        conduit_composite::PreparedWorkerBack::new(self.prepare(placement, values)?, 0)
    }
    fn prepare(
        &self,
        placement: &PlannedGear,
        _: &mut HostedValueStore,
    ) -> Result<Box<dyn StepBack<PORTS> + Send>, String> {
        Ok(Box::new(Back {
            source: placement.inputs.is_empty(),
            sent: false,
            counts: self.counts.clone(),
        }))
    }
}
pub fn template() -> PlanFragment {
    let mut fragment = geometry::plan().fragments.remove(0);
    fragment.execution_regions.clear();
    for placement in &mut fragment.placements {
        placement.capability_id = format!("{}/offer", placement.placement_id.as_str()).into();
    }
    fragment
}
pub fn host(fragment: &PlanFragment) -> StdHost {
    let mut advertisement = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: fragment.host_id.clone(),
        boot_id: fragment.boot_id.clone(),
        offer_generation: fragment.offer_generation,
        profile: "lifetime-public-worker-fixture".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    for p in &fragment.placements {
        advertisement.capabilities.push(capability_offer_from_parts! {
            semantic_contract: p.semantic_contract.clone(), startup_parameters: vec![], shorthand: None,
            capability_id: p.capability_id.clone(), kind_id: p.kind_id.clone(), kind_contract_revision: p.kind_contract_revision.clone(),
            implementation: ImplementationOffer { execution_profile_id: p.execution_profile_id.clone(), implementation_id: p.implementation_id.clone(), artifact_id: p.artifact_id.clone() },
            inputs: p.inputs.clone(), outputs: p.outputs.clone(), host_calls: vec![], resource_requirements: vec![], authority_requirements: vec![], limits: p.limits.clone(),
        });
    }
    StdHost::from_advertisement(advertisement).unwrap()
}
pub fn plan(fragment: &PlanFragment, advertisement: &HostAdvertisement) -> Plan {
    let mut fragment = fragment.clone();
    fragment.offer_generation = advertisement.offer_generation;
    for placement in &mut fragment.placements {
        placement.offer_generation = advertisement.offer_generation;
    }
    seal_plan(
        PlotIdentity {
            source_document_id: fragment.source_document_id.clone(),
            checked_plot_id: fragment.checked_plot_id.clone(),
            expanded_plot_id: fragment.expanded_plot_id.clone(),
        },
        vec![fragment],
    )
}
pub fn groups(plan: &Plan) -> Vec<Vec<PlacementId>> {
    plan.fragments[0]
        .placements
        .iter()
        .map(|p| vec![p.placement_id.clone()])
        .collect()
}
pub fn registry(fragment: &PlanFragment, counts: &Arc<Counts>) -> KernelOperationRegistry {
    let mut registry = KernelOperationRegistry::new();
    registry
        .install(Factory {
            implementation: fragment.placements[0].implementation_id.clone(),
            counts: counts.clone(),
        })
        .unwrap();
    registry
}
