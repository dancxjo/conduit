use super::*;
use crate::test_support::{allocation, common, single_child};
use crate::{KernelOperationBudget, KernelOperationFactory, ReusableKernelOperation};
use conduit_core::{ImplementationId, PlannedGear};
use conduit_kernel::scheduler::{
    prepared_session::ReusableStepBack, StepBack, StepInputBytes, StepIo, StepOutcome,
};
use conduit_kernel::{HostedValueStore, PortId, ValueStorage};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE as PORTS;
use core::sync::atomic::{AtomicUsize, Ordering};

struct Echo {
    done: bool,
    staged: bool,
}
impl StepBack<PORTS> for Echo {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        self.staged = false;
        if self.done {
            return StepOutcome::Complete;
        }
        let Some(value) = io.input(PortId(0)) else {
            return StepOutcome::Await;
        };
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        io.send(PortId(0), value).unwrap();
        io.consume(PortId(0)).unwrap();
        self.staged = true;
        StepOutcome::Progress
    }
    fn step_committed(&mut self) {
        if self.staged {
            self.done = true;
            self.staged = false;
        }
    }
}
impl ReusableStepBack<PORTS> for Echo {
    type Next = ();
    fn preflight_next(&self, _: &(), _: &dyn ValueStorage) -> bool {
        self.done && !self.staged
    }
    fn begin_next(&mut self, _: ()) {
        self.done = false;
    }
}
struct Factory {
    id: ImplementationId,
    one_shot: Arc<AtomicUsize>,
    reusable: Arc<AtomicUsize>,
    allow: bool,
}
impl KernelOperationFactory for Factory {
    fn implementation_id(&self) -> &ImplementationId {
        &self.id
    }
    fn budget(&self, _: &PlannedGear) -> Result<KernelOperationBudget, String> {
        Ok(KernelOperationBudget {
            value_items: 0,
            value_bytes: 0,
            maximum_value_bytes: 1,
            host_requests: 0,
            sign_items: 8,
        })
    }
    fn prepare(
        &self,
        _: &PlannedGear,
        _: &mut HostedValueStore,
    ) -> Result<Box<dyn StepBack<PORTS> + Send>, String> {
        self.one_shot.fetch_add(1, Ordering::Relaxed);
        Ok(Box::new(Echo {
            done: false,
            staged: false,
        }))
    }
    fn prepare_reusable_value(
        &self,
        _: &PlannedGear,
        _: &mut HostedValueStore,
    ) -> Result<ReusableKernelOperation, String> {
        self.reusable.fetch_add(1, Ordering::Relaxed);
        if !self.allow {
            return Err("fixture declines reuse".into());
        }
        Ok(Box::new(Echo {
            done: false,
            staged: false,
        }))
    }
}
fn geometry(
    allow: bool,
) -> (
    KernelCompositeDefinition,
    KernelOperationRegistry,
    Arc<AtomicUsize>,
    Arc<AtomicUsize>,
) {
    let definition = single_child(common::fragment(), 1);
    let one = Arc::new(AtomicUsize::new(0));
    let reused = Arc::new(AtomicUsize::new(0));
    let mut registry = KernelOperationRegistry::new();
    registry
        .install(Factory {
            id: definition.internal_plan.fragments[0].placements[0]
                .implementation_id
                .clone(),
            one_shot: one.clone(),
            reusable: reused.clone(),
            allow,
        })
        .unwrap();
    (definition, registry, one, reused)
}
fn invoke(host: &mut KernelCompositeHost, sequence: u64, byte: u8) {
    let input = host.definition().boundary.input_fronts[0]
        .external_port
        .clone();
    let output = host.definition().boundary.output_fronts[0]
        .external_port
        .clone();
    host.start().unwrap();
    host.admit_input(
        &input.port_id,
        sequence,
        &ValuePayload {
            value_kind: input.value_kind,
            encoded: vec![byte],
        },
    )
    .unwrap();
    host.close_input(&input.port_id).unwrap();
    host.step().unwrap();
    let (actual_sequence, value) = host.output(&output.port_id).unwrap().unwrap();
    assert_eq!(actual_sequence, sequence);
    assert_eq!(value.encoded, [byte]);
    host.complete_output(&output.port_id, sequence).unwrap();
    for _ in 0..8 {
        if host.step().unwrap() == KernelCompositeStatus::Complete {
            return;
        }
    }
    panic!("not complete")
}
#[test]
fn one_composite_child_reuses_bindings_and_moves_complete_evidence_without_allocation() {
    let (definition, registry, one, reused) = geometry(true);
    let mut host =
        KernelCompositeHost::prepare_reusable_value(definition.clone(), &registry).unwrap();
    let parent = ActivePlayId::from("actual-parent");
    host.bind_parent_play(&parent, "scan", 0).unwrap();
    let binding = host.prepare_value_invocation(&parent, "scan", 1).unwrap();
    let signs = host.empty_value_evidence().unwrap();
    let other = KernelCompositeHost::prepare_reusable_value(definition, &registry).unwrap();
    let foreign = other.prepare_value_invocation(&parent, "scan", 1).unwrap();
    let foreign_signs = other.empty_value_evidence().unwrap();
    invoke(&mut host, 0, 7);
    let before = host.signs();
    let old_play = host.active_plays().clone();
    assert!(matches!(
        host.rearm_value(foreign, foreign_signs),
        Err(KernelCompositeError::InvalidLifecycle)
    ));
    assert_eq!(host.signs(), before);
    let mut evidence = None;
    let count = allocation::allocations_during(|| {
        evidence = Some(host.rearm_value(binding, signs).unwrap());
    });
    assert_eq!(count, 0);
    let evidence = evidence.unwrap();
    assert_eq!(evidence.parent_play_binding(), (&parent, 0));
    assert_eq!(evidence.active_plays(), &old_play);
    assert_eq!(
        evidence.signs().events().collect::<Vec<_>>(),
        before.values().next().unwrap().clone()
    );
    assert_ne!(host.active_plays(), &old_play);
    invoke(&mut host, 1, 9);
    assert_eq!(one.load(Ordering::Relaxed), 0);
    assert_eq!(
        reused.load(Ordering::Relaxed),
        2,
        "two owners, never a warm reprepare"
    );
}
#[test]
fn an_unavailable_reusable_factory_never_falls_back_to_one_shot_preparation() {
    let (definition, registry, one, reused) = geometry(false);
    assert!(KernelCompositeHost::prepare_reusable_value(definition, &registry).is_err());
    assert_eq!(reused.load(Ordering::Relaxed), 1);
    assert_eq!(one.load(Ordering::Relaxed), 0);
}
