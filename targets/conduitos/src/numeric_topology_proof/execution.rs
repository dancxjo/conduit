//! Fresh-Boot generic owner preparation and allocation-sealed scheduler run.
use super::{
    PreparedTopology,
    driver::{Driver, Output, PORTS},
    factories,
    resources::PreparedIngress,
    storage::BorrowedStorage,
};
use alloc::{boxed::Box, collections::BTreeMap, format, rc::Rc, string::String, vec::Vec};
use conduit_core::*;
use conduit_kernel::{
    FixedHostCallBindings, FixedRoutes, FixedSignLog, HostedValueStore, ValueStorage,
    scheduler::{CordSpec, FixedScheduler, NodeSpec},
};
use core::cell::RefCell;
const N: usize = super::MAXIMUM_NODES;
const C: usize = super::MAXIMUM_CORDS;
const EVENTS: usize = 32768;
type Scheduler<'a> = FixedScheduler<
    Driver,
    BorrowedStorage<'a>,
    FixedSignLog<EVENTS>,
    N,
    C,
    PORTS,
    C,
    C,
    C,
    1024,
    1024,
>;
pub struct PreparedExecution<'a> {
    scheduler: Scheduler<'a>,
    expressions: BTreeMap<conduit_kernel::NodeId, crate::expression_host_call::ExpressionHostCall>,
    output: Rc<RefCell<Output>>,
}
impl<'a> PreparedExecution<'a> {
    pub fn prepare(
        topology: &PreparedTopology<'_>,
        ingress: PreparedIngress,
        mut storage: BorrowedStorage<'a>,
    ) -> Result<Self, String> {
        let plan = &topology.plan;
        let fragment = &plan.fragments[0];
        let (lowered, _) = conduit_plan_lowering::lowering::lower_plan_fragment_from_plan(
            plan,
            &fragment.fragment_id,
        )
        .map_err(|e| format!("numeric lowering: {e:?}"))?;
        let active = bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 0);
        let expression_fragment = crate::expression_host_call::PreparedExpressionFragment::prepare(
            fragment, &lowered, &active,
        )
        .map_err(|e| format!("expression fragment: {e:?}"))?;
        let factories = factories::prepare(topology, &ingress)?;
        let mut hosted = HostedValueStore::new(1024, 16384, 16 * 1024 * 1024)
            .map_err(|e| format!("ingress store: {e:?}"))?;
        let mut references = BTreeMap::new();
        let mut live = Vec::new();
        for (kind, bytes) in &ingress.values {
            let reference = hosted
                .store(bytes)
                .map_err(|e| format!("bounded ingress: {e:?}"))?;
            live.push(reference);
            references.insert(kind.clone(), reference);
        }
        let output = Rc::new(RefCell::new(Output::prepare()));
        let mut expressions = BTreeMap::new();
        let mut drivers = Vec::with_capacity(N);
        let mut sinks = 0;
        for (index, gear) in fragment.placements.iter().enumerate() {
            if let Some(factory) = factories
                .iter()
                .find(|f| f.implementation_id() == &gear.implementation_id)
            {
                factory
                    .budget(gear)
                    .map_err(|e| format!("owner budget: {e:?}"))?;
                drivers.push(Driver::Operation(
                    factory
                        .prepare(gear, &mut hosted)
                        .map_err(|e| format!("owner prepare: {e:?}"))?,
                ));
            } else if gear.implementation_id.as_str() == crate::expression_host_call::IMPLEMENTATION
            {
                let owner = expression_fragment
                    .owner(&gear.placement_id)
                    .map_err(|e| format!("expression owner: {e:?}"))?;
                expressions.insert(conduit_kernel::NodeId(index as u16), owner);
                drivers.push(Driver::Operation(Box::new(
                    conduit_kernel::scheduler::HostCallBack::new(
                        gear.host_calls[0].maximum_input_bytes,
                    ),
                )));
            } else if topology
                .recipe
                .reference_fixture_placements
                .iter()
                .any(|p| p.kind_id == gear.kind_id)
            {
                if let Some(value) = references.get(&gear.kind_id) {
                    drivers.push(Driver::Source {
                        value: *value,
                        sent: false,
                        staged: false,
                    });
                } else if gear.inputs.len() == 1 && gear.outputs.is_empty() {
                    sinks += 1;
                    drivers.push(Driver::Sink(output.clone()));
                } else {
                    return Err("unbound exact synthetic boundary".into());
                }
            } else {
                return Err(format!(
                    "unsupported selected numeric owner: {}",
                    gear.implementation_id.as_str()
                ));
            }
        }
        if sinks != 1 {
            return Err("synthetic proof requires exactly one retained output".into());
        }
        storage
            .admit_ingress(&hosted, &live)
            .map_err(|e| format!("static ingress: {e:?}"))?;
        drop(hosted);
        drop(factories);
        drop(ingress);
        let nodes = drivers.len();
        drivers.resize_with(N, || Driver::Inactive);
        let mut routes = FixedRoutes::<C, C>::new(1);
        for route in &lowered.routes {
            routes
                .install(
                    route.source_node,
                    route.source_port,
                    route.range,
                    &route.targets,
                )
                .map_err(|e| format!("route: {e:?}"))?;
        }
        routes.seal().map_err(|e| format!("routes seal: {e:?}"))?;
        let mut bindings = FixedHostCallBindings::<1024>::new(1);
        for binding in &lowered.host_calls {
            bindings
                .install(binding.node, binding.binding)
                .map_err(|e| format!("host binding: {e:?}"))?;
        }
        bindings
            .seal()
            .map_err(|e| format!("host bindings seal: {e:?}"))?;
        let mut specs = lowered.node_specs.clone();
        specs.resize(
            N,
            NodeSpec {
                input_cords: [None; PORTS],
                maximum_step_fuel: 0,
            },
        );
        let mut cords: Vec<_> = lowered.cords.iter().map(|c| c.spec).collect();
        cords.resize(C, CordSpec::inactive());
        let signs = FixedSignLog::<EVENTS>::new(
            (EVENTS * core::mem::size_of::<conduit_kernel::KernelEvent>()) as u32,
        )
        .map_err(|e| format!("sign storage: {e:?}"))?;
        let scheduler = Scheduler::new_with_active_counts_and_host_calls(
            nodes,
            lowered.cords.len(),
            specs.try_into().map_err(|_| "node capacity")?,
            cords.try_into().map_err(|_| "cord capacity")?,
            routes,
            bindings,
            drivers.try_into().map_err(|_| "driver capacity")?,
            storage,
            signs,
        )
        .map_err(|e| format!("scheduler preparation: {e:?}"))?;
        Ok(Self {
            scheduler,
            expressions,
            output,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionRefusal {
    Scheduler,
    Undrained,
    Budget,
    HostOwner,
    HostInput,
    HostInvoke,
    HostOutput,
    HostCommit,
    Output,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionReceipt {
    pub scheduler_steps: usize,
    pub expression_calls: usize,
    pub output_bytes: usize,
    pub output_sha256: [u8; 32],
    pub peak_cells: u16,
    pub peak_payload_bytes: u32,
}
impl PreparedExecution<'_> {
    /// Caller must seal the admitted allocator first; this method never formats
    /// errors, grows collections, discovers resources or reconstructs schemas.
    pub fn run(&mut self) -> Result<ExecutionReceipt, ExecutionRefusal> {
        use conduit_kernel::{
            BoundedValueRef, HostCallDisposition, HostCallOutcome, scheduler::SchedulerStatus,
        };
        use sha2::{Digest, Sha256};
        let mut expression_calls = 0;
        for step in 0..262144 {
            let status = self
                .scheduler
                .step()
                .map_err(|_| ExecutionRefusal::Scheduler)?;
            if let Some(call) = self.scheduler.next_host_request() {
                let owner = self
                    .expressions
                    .get_mut(&call.node)
                    .ok_or(ExecutionRefusal::HostOwner)?;
                let input = self
                    .scheduler
                    .values()
                    .get(call.input.value)
                    .map_err(|_| ExecutionRefusal::HostInput)?;
                let output = owner
                    .invoke(call.node, call.call, call.request, input)
                    .map_err(|_| ExecutionRefusal::HostInvoke)?;
                expression_calls += 1;
                let value = self
                    .scheduler
                    .store_host_value(output)
                    .map_err(|_| ExecutionRefusal::HostOutput)?;
                let output = BoundedValueRef::new(value, output.len() as u32)
                    .map_err(|_| ExecutionRefusal::HostOutput)?;
                self.scheduler
                    .complete_host_call(
                        call.node,
                        call.request,
                        HostCallOutcome {
                            disposition: HostCallDisposition::Completed,
                            output: Some(output),
                            failure: None,
                        },
                    )
                    .map_err(|_| ExecutionRefusal::HostCommit)?;
            }
            if status == SchedulerStatus::Drained && self.scheduler.pending_host_call_count() == 0 {
                let output = self.output.borrow();
                if output.committed != 1 || output.bytes.is_empty() {
                    return Err(ExecutionRefusal::Output);
                }
                let (peak_cells, peak_payload_bytes) = self.scheduler.values().peaks();
                return Ok(ExecutionReceipt {
                    scheduler_steps: step + 1,
                    expression_calls,
                    output_bytes: output.bytes.len(),
                    output_sha256: Sha256::digest(&output.bytes).into(),
                    peak_cells,
                    peak_payload_bytes,
                });
            }
            if matches!(status, SchedulerStatus::Idle | SchedulerStatus::Cancelled)
                && self.scheduler.pending_host_call_count() == 0
            {
                return Err(ExecutionRefusal::Undrained);
            }
        }
        Err(ExecutionRefusal::Budget)
    }
}
