#[path = "../../../../targets/conduitos/src/pending_host_output.rs"]
mod pending_output;
use super::fixtures::*;
use super::synthetic_resources::synthetic_resources;
use conduit_ai::fixed_numeric_binding::*;
use conduit_composite::KernelOperationFactory;
use conduit_core::*;
use conduit_kernel::{
    scheduler::{FixedScheduler, StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, FixedRoutes, HostedSignLog, HostedValueStore, KernelEvent, PortId as KPort,
    ValueRef, ValueStorage,
};
use std::{
    collections::BTreeMap,
    rc::Rc,
    time::{Duration, Instant},
};
#[path = "runtime/fixed_storage.rs"]
mod fixed_storage;
#[path = "runtime/native_control.rs"]
mod native_control;
#[path = "runtime/native_startup.rs"]
mod native_startup;
#[path = "runtime/native_trace.rs"]
mod native_trace;
#[path = "runtime/native_utterance.rs"]
mod native_utterance;
#[path = "runtime/primary_sink.rs"]
mod primary_sink;
#[path = "runtime/trace_hooks.rs"]
mod trace_hooks;
#[path = "runtime/trace_outputs.rs"]
mod trace_outputs;
pub(super) use native_control::run_native_period_controls;
pub(super) use native_startup::{
    run_native_first_feature, run_native_first_feature16k, run_native_startup_feedback, run_native_warm_startup,
};
pub(super) use native_utterance::{run_native_trained_utterance, run_direct16k_trained_utterance};

#[path = "runtime/model_compute_session.rs"]
mod model_compute_session;

const PORTS: usize = conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
const N: usize = 1024;
const C: usize = 2048;

fn analysis_resources() -> Resources {
    let load = |bytes: &[u8]| {
        bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|value| f32::from_le_bytes(*value))
            .collect()
    };
    [
        (
            "band_weights",
            "NumericF32MatrixRef161x18",
            vec![161, 18],
            load(include_bytes!(
                "../../../../proof/fargan/feature-profile/bands161x18.bin"
            )),
        ),
        (
            "band_bias",
            "NumericF32BiasRef18",
            vec![18],
            load(include_bytes!(
                "../../../../proof/fargan/feature-profile/band_bias18.bin"
            )),
        ),
        (
            "dct_weights",
            "NumericF32MatrixRef18x18",
            vec![18, 18],
            load(include_bytes!(
                "../../../../proof/fargan/feature-profile/dct18x18.bin"
            )),
        ),
    ]
    .into_iter()
    .map(|(name, ty, dims, values)| {
        (
            name.into(),
            Resource::new(
                conduit_ai::fixed_numeric_catalog::fixed_numeric_type(ty).unwrap(),
                &dims,
                values,
            ),
        )
    })
    .collect()
}

trait RuntimeTensor {
    fn value_type(&self) -> &StructuredInfoType;
    fn tensor(&self) -> &conduit_data::TensorValue;
    fn adopt(
        &self,
    ) -> std::sync::Arc<conduit_ai::fixed_tensor_resource::AdmittedFixedTensorResource>;
}

impl RuntimeTensor for Resource {
    fn value_type(&self) -> &StructuredInfoType {
        &self.value_type
    }
    fn tensor(&self) -> &conduit_data::TensorValue {
        &self.tensor
    }
    fn adopt(
        &self,
    ) -> std::sync::Arc<conduit_ai::fixed_tensor_resource::AdmittedFixedTensorResource> {
        Resource::adopt(self)
    }
}
impl RuntimeTensor for super::custody::RetainedTensor {
    fn value_type(&self) -> &StructuredInfoType {
        &self.value_type
    }
    fn tensor(&self) -> &conduit_data::TensorValue {
        self.resource.tensor()
    }
    fn adopt(
        &self,
    ) -> std::sync::Arc<conduit_ai::fixed_tensor_resource::AdmittedFixedTensorResource> {
        self.resource.clone()
    }
}
fn source_cursors(drivers:&[Driver;N])->Vec<usize>{drivers.iter().filter_map(|d|match d{Driver::Source{next,..}=>Some(*next),_=>None}).collect()}
enum Driver {
    Source {
        references: Vec<ValueRef>,
        staged: bool,
        next: usize,
    },
    Operation(Box<dyn StepBack<PORTS>>),
    Sink(primary_sink::PrimarySink),
    Trace(Rc<std::cell::RefCell<trace_hooks::sink::DevelopmentTraceSink>>),
    Inactive,
}
impl StepBack<PORTS> for Driver {
    fn step(&mut self, io: &mut StepIo<PORTS>, bytes: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        match self {
            Self::Source {
                references,
                staged,
                next,
            } => {
                if *next == references.len() {
                    return StepOutcome::Complete;
                }
                if !io.output_ready(KPort(0)) {
                    return StepOutcome::Await;
                }
                io.send(KPort(0), references[*next]).unwrap();
                *staged = true;
                StepOutcome::Progress
            }
            Self::Operation(back) => back.step(io, bytes),
            Self::Trace(back) => back.borrow_mut().step(io, bytes),
            Self::Sink(back) => back.step(io, bytes),
            Self::Inactive => StepOutcome::Complete,
        }
    }
    fn prepared_output(&self, p: KPort) -> Option<&[u8]> {
        match self {
            Self::Operation(back) => back.prepared_output(p),
            _ => None,
        }
    }
    fn step_committed(&mut self) {
        match self {
            Self::Operation(back) => back.step_committed(),
            Self::Trace(back) => {
                <trace_hooks::sink::DevelopmentTraceSink as StepBack<PORTS>>::step_committed(
                    &mut *back.borrow_mut(),
                )
            }
            Self::Sink(back) => back.step_committed(),
            Self::Source { staged, next, .. } if *staged => {
                *next += 1;
                *staged = false;
            }
            _ => {}
        }
    }
    fn cancel(&mut self) {
        match self {
            Self::Operation(back) => back.cancel(),
            Self::Sink(back) => back.cancel(),
            Self::Trace(back) => {
                <trace_hooks::sink::DevelopmentTraceSink as StepBack<PORTS>>::cancel(
                    &mut *back.borrow_mut(),
                )
            }
            _ => {}
        }
    }
}

#[derive(Clone, Copy)]
enum ExecutionMode {
    Normal,
    LifecycleNormal,
    LifecycleWarm,
    LifecycleCancel,
    LifecycleProviderLost,
    LifecyclePressure,
    LifecyclePressureResume,
    StoragePressure,
    CancelFirstExpression,
}
struct EncodedResultAndTiming {
    value: StructuredInfoValue,
    preparation: Duration,
    execution: Duration,
    nodes: usize,
    cords: usize,
}
fn run_epoch_plan(
    plan: Plan,
    context: &super::EpochProfiles,
    resources: &Resources,
    input_values: BTreeMap<String, Vec<u8>>,
    mode: ExecutionMode,
) -> Option<EncodedResultAndTiming> {
    let result = run_epoch_stream_plan(
        plan,
        context,
        resources,
        input_values
            .into_iter()
            .map(|(n, v)| (n, vec![v]))
            .collect(),
        None,
        1,
        mode,
    )?;
    Some(EncodedResultAndTiming {
        value: result.values.into_iter().next().unwrap(),
        preparation: result.preparation,
        execution: result.execution,
        nodes: result.nodes,
        cords: result.cords,
    })
}
struct StreamResultAndTiming {
    values: Vec<StructuredInfoValue>,
    preparation: Duration,
    execution: Duration,
    nodes: usize,
    cords: usize,
    drained: bool,
    scheduler_step_allocations: usize,
    prepared_expression_allocations: usize,
    service_steps: u32,
    completed_host_calls: u64,
    pressure_resumed: bool,
    quantum_boundaries: u32,
    warm_retired: bool,
}
fn run_epoch_stream_plan<R: RuntimeTensor>(
    plan: Plan,
    context: &super::EpochProfiles,
    resources: &BTreeMap<String, R>,
    input_values: BTreeMap<String, Vec<Vec<u8>>>,
    seeded: Option<conduitos::seeded_state::SeededStateOperationFactory>,
    expected: usize,
    mode: ExecutionMode,
) -> Option<StreamResultAndTiming> {
    run_epoch_stream_plan_with_trace(
        plan,
        context,
        resources,
        input_values,
        seeded.as_ref(),
        StreamRun {
            expected,
            mode,
            trace: None,
            service: ServiceBudget::legacy(),
        },
    )
}
#[derive(Clone, Copy)]
struct ServiceBudget {
    quantum: u32,
    quanta: u32,
    observe_boundaries: bool,
}
impl ServiceBudget {
    const fn legacy() -> Self { Self { quantum: 262144, quanta: 1, observe_boundaries: true } }
    fn direct(profile: &super::service_profile::PreparedServiceProfile) -> Self {
        Self { quantum: profile.quantum(), quanta: profile.quanta(), observe_boundaries: true }
    }
}
struct StreamRun<'a> {
    expected: usize,
    mode: ExecutionMode,
    trace: Option<&'a trace_hooks::TraceSinks>,
    service: ServiceBudget,
}
fn run_epoch_stream_plan_with_trace<R: RuntimeTensor>(
    plan: Plan,
    context: &super::EpochProfiles,
    resources: &BTreeMap<String, R>,
    input_values: BTreeMap<String, Vec<Vec<u8>>>,
    seeded: Option<&conduitos::seeded_state::SeededStateOperationFactory>,
    run: StreamRun<'_>,
) -> Option<StreamResultAndTiming> {
    let StreamRun {
        expected,
        mode,
        trace,
        service,
    } = run;
    let run_to_drain = matches!(mode, ExecutionMode::LifecycleNormal | ExecutionMode::LifecycleWarm | ExecutionMode::LifecyclePressureResume) || trace.is_some()
        || seeded.is_some()
        || plan
            .fragments
            .iter()
            .flat_map(|f| &f.placements)
            .any(|gear| {
                matches!(
                    gear.implementation_id.as_str(),
                    conduit_std_host::pure_filter::IMPLEMENTATION
                        | conduit_std_offers::VALUE_REPEAT_IMPLEMENTATION
                        | conduitos::flow_concat_finite::IMPLEMENTATION
                )
            });
    let start = Instant::now();
    assert!(verify_plan(&plan));
    let fragment = &plan.fragments[0];
    let (lowered, _) = conduit_plan_lowering::lowering::lower_plan_fragment_from_plan(
        &plan,
        &fragment.fragment_id,
    )
    .unwrap_or_else(|error| {
        if let conduit_plan_lowering::lowering::LoweringError::ConnectionContractMismatch(id) =
            &error
        {
            for connection in fragment
                .connections
                .iter()
                .filter(|cord| &cord.connection_id == id)
            {
                eprintln!("mismatched cord: {connection:?}");
                for placement in fragment.placements.iter().filter(|placement| {
                    placement.placement_id == connection.source_placement_id
                        || placement.placement_id == connection.sink_placement_id
                }) {
                    eprintln!(
                        "endpoint {:?} {:?} inputs={:?} outputs={:?}",
                        placement.gear_id, placement.kind_id, placement.inputs, placement.outputs
                    );
                }
            }
        }
        panic!("epoch lowering refused: {error:?}")
    });
    assert!(lowered.node_specs.len() <= N && lowered.cords.len() <= C);
    let active = bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    let expression_fragment = fragment.placements.iter().any(|gear| gear.implementation_id.as_str() == conduitos::expression_host_call::IMPLEMENTATION).then(|| conduitos::expression_host_call::PreparedExpressionFragment::prepare(fragment, &lowered, &active).unwrap());
    let slots = match mode {
        ExecutionMode::StoragePressure => {
            resources.len() + input_values.values().map(Vec::len).sum::<usize>() + 2
        }
        _ => 1024,
    };
    let mut store = HostedValueStore::new(
        slots.try_into().unwrap(),
        16384,
        (slots * 16384).try_into().unwrap(),
    )
    .unwrap();
    let mut fixtures: BTreeMap<_, _> = input_values
        .iter()
        .map(|(name, encoded)| {
            (
                name.clone(),
                encoded
                    .iter()
                    .map(|value| store.store(value).unwrap())
                    .collect::<Vec<_>>(),
            )
        })
        .collect();
    for (name, resource) in resources {
        let binding =
            FixedTensorPortBinding::prepare(resource.value_type(), resource.tensor()).unwrap();
        fixtures.insert(name.clone(), vec![store.store(binding.encoded()).unwrap()]);
    }
    let mut adopted = BTreeMap::new();
    for gear in &fragment.placements {
        if let Some(name) = gear.kind_id.as_str().strip_prefix("epoch-proof/") {
            if let Some(resource) = resources.get(name) {
                adopted.insert(gear.placement_id.clone(), resource.adopt());
            }
        }
    }
    let factories =
        conduit_ai::operation_owners::fixed_numeric::FixedNumericOperationFactory::for_plan_capacity64(&plan, &adopted)
            .unwrap();
    let mut factories: Vec<Box<dyn KernelOperationFactory>> = factories
        .into_iter()
        .map(|factory| Box::new(factory) as Box<dyn KernelOperationFactory>)
        .collect();
    use conduit_ai::operation_owners::*;
    let period_profile = std::sync::Arc::new(
        conduit_ai::fixed_numeric_u16_profile::PreparedU16Profile::check_definition(
            "type FarganPeriod = U16 in 32..=255\n",
        )
        .unwrap(),
    );
    factories.push(Box::new(
        fixed_numeric_u16_profile::U16ProfileOperationFactory::for_plan(&plan, &[period_profile])
            .unwrap(),
    ));
    factories.push(Box::new(
        fixed_numeric_embedding_flow::FixedEmbeddingFlowOperationFactory::for_plan(&plan, &adopted)
            .unwrap(),
    ));
    factories.push(Box::new(
        fixed_numeric_flow::FixedAffineFlowOperationFactory::for_plan(&plan, &adopted).unwrap(),
    ));
    factories.push(Box::new(
        fixed_numeric_linear_flow::FixedLinearFlowOperationFactory::for_plan(&plan, &adopted)
            .unwrap(),
    ));
    factories.push(Box::new(
        fixed_numeric_pair_flow::FixedFlowPairOperationFactory::for_plan(&plan).unwrap(),
    ));
    factories.push(Box::new(
        fixed_numeric_float_integer::FloatIntegerOperationFactory::for_plan(&plan).unwrap(),
    ));
    factories.push(Box::new(
        native_profile::NativeProfileOperationFactory::for_plan(&plan, &context.native).unwrap(),
    ));
    factories.push(Box::new(
        nominal_weakening::NominalWeakeningOperationFactory::for_plan(&plan, &context.weakening)
            .unwrap(),
    ));
    factories.push(Box::new(
        fixed_numeric_guard::FixedGuardOperationFactory::for_plan(&plan, context.guards.clone())
            .unwrap(),
    ));
    factories.push(Box::new(conduit_ai::operation_owners::closing_structured_pair::ClosingStructuredPairOperationFactory::for_plan(&plan, context.pairs.clone()).unwrap()));
    let mut repeat_profiles = Vec::new();
    let mut singleton_profiles = Vec::new();
    let mut concat = conduitos::flow_concat_finite::FlowConcatFiniteOperationFactory::default();
    for implementation in [
        conduit_std_offers::VALUE_REPEAT_IMPLEMENTATION,
        conduitos::flow_concat_finite::IMPLEMENTATION,
        conduit_std_offers::FLOW_EXACTLY_ONE_IMPLEMENTATION,
    ] {
        if let Some(gear) = fragment
            .placements
            .iter()
            .find(|gear| gear.implementation_id.as_str() == implementation)
        {
            let input = if implementation == conduit_std_offers::FLOW_EXACTLY_ONE_IMPLEMENTATION {
                "item"
            } else if implementation == conduit_std_offers::VALUE_REPEAT_IMPLEMENTATION {
                "value"
            } else {
                "left"
            };
            let value = &gear
                .semantic_contract
                .value_contracts()
                .iter()
                .find(|entry| entry.location == FrontValueLocation::Input(port_id(input)))
                .unwrap()
                .contract;
            let profile = context
                .native
                .iter()
                .find(|profile| {
                    profile
                        .contract(false)
                        .unwrap()
                        .checked_front()
                        .value_contracts()
                        .iter()
                        .any(|entry| {
                            entry.location == FrontValueLocation::Output(port_id("result"))
                                && &entry.contract == value
                        })
                })
                .expect("terminal fixture operation requires its retained complete native profile");
            if implementation == conduit_std_offers::FLOW_EXACTLY_ONE_IMPLEMENTATION {
                singleton_profiles.push(std::sync::Arc::new(
                    conduit_std_host::flow_exactly_one::PreparedFlowExactlyOne::new(
                        value.clone(),
                        profile.value_type().clone(),
                    )
                    .unwrap(),
                ));
            } else if implementation == conduit_std_offers::VALUE_REPEAT_IMPLEMENTATION {
                let prepare = if gear.kind_id.as_str()
                    == conduit_semantic_catalog::VALUE_REPEAT_CAPACITY2_KIND
                {
                    conduit_std_host::value_repeat::PreparedValueRepeat::capacity2
                } else {
                    conduit_std_host::value_repeat::PreparedValueRepeat::new
                };
                repeat_profiles.push(std::sync::Arc::new(
                    prepare(value.clone(), profile.value_type().clone()).unwrap(),
                ));
            } else {
                concat.install(value, profile.value_type()).unwrap();
            }
        }
    }
    if !singleton_profiles.is_empty() {
        factories.push(Box::new(
            conduit_std_host::flow_exactly_one::FlowExactlyOneOperationFactory::for_plan(
                &plan,
                &singleton_profiles,
            )
            .unwrap(),
        ));
    }
    if !repeat_profiles.is_empty() {
        factories.push(Box::new(
            conduit_std_host::value_repeat::ValueRepeatOperationFactory::for_plan(
                &plan,
                &repeat_profiles,
            )
            .unwrap(),
        ));
    }
    if fragment.placements.iter().any(|gear| {
        gear.implementation_id.as_str() == conduitos::flow_concat_finite::IMPLEMENTATION
    }) {
        concat.validate_plan(&plan).unwrap();
        factories.push(Box::new(concat));
    }
    context.zip.validate_plan(&plan).unwrap();
    let output_pool = primary_sink::OutputPool::prepare(expected);
    let received = output_pool.received.clone();
    let filter_factory = fragment
        .placements
        .iter()
        .any(|gear| {
            gear.implementation_id.as_str() == conduit_std_host::pure_filter::IMPLEMENTATION
        })
        .then(|| {
            conduit_std_host::pure_filter::PureFilterOperationFactory::for_plan(&plan).unwrap()
        });
    let mut filter_owners = BTreeMap::new();
    let mut owners = BTreeMap::new();
    let mut drivers = Vec::new();
    for (index, gear) in fragment.placements.iter().enumerate() {
        if let Some(factory) = factories
            .iter()
            .find(|f| f.implementation_id() == &gear.implementation_id)
            .map(|f| f.as_ref())
            .or_else(|| seeded.filter(|f| f.implementation_id() == &gear.implementation_id).map(|f| f as &dyn KernelOperationFactory))
        {
            factory.budget(gear).unwrap();
            drivers.push(Driver::Operation(
                factory.prepare(gear, &mut store).unwrap(),
            ));
        } else if gear.implementation_id.as_str() == conduit_std_host::pure_filter::IMPLEMENTATION {
            let factory = filter_factory.as_ref().unwrap();
            factory.budget(gear).unwrap();
            filter_owners.insert(
                conduit_kernel::NodeId(index as u16),
                factory.prepare_host(gear).unwrap(),
            );
            drivers.push(Driver::Operation(
                factory.prepare(gear, &mut store).unwrap(),
            ));
        } else if gear.implementation_id.as_str() == conduitos::flow_zip::FRAME16K_IMPLEMENTATION {
            context.zip.budget(gear).unwrap();
            drivers.push(Driver::Operation(
                context.zip.prepare(gear, &mut store).unwrap(),
            ));
        } else if gear.implementation_id.as_str() == conduitos::expression_host_call::IMPLEMENTATION
        {
            let owner = expression_fragment.as_ref().unwrap().owner(&gear.placement_id).unwrap();
            owners.insert(conduit_kernel::NodeId(index as u16), owner);
            drivers.push(Driver::Operation(Box::new(
                conduit_kernel::scheduler::HostCallBack::new(
                    gear.host_calls[0].maximum_input_bytes,
                ),
            )));
        } else {
            let name = gear.kind_id.as_str().strip_prefix("epoch-proof/").unwrap();
            if let Some(reference) = fixtures.get(name) {
                drivers.push(Driver::Source {
                    references: reference.clone(),
                    staged: false,
                    next: 0,
                });
            } else {
                assert_eq!(gear.inputs.len(), 1, "missing fixture for input {name}");
                assert!(gear.outputs.is_empty(), "missing fixture for input {name}");
                if let Some(sink) = trace.and_then(|trace| trace.get(name)) {
                    drivers.push(Driver::Trace(sink));
                } else {
                    drivers.push(Driver::Sink(primary_sink::PrimarySink::prepare(
                        output_pool.clone(),
                    )));
                }
            }
        }
    }
    drop(factories);
    drop(adopted); // drivers retain immutable admitted resource custody.
    let store = fixed_storage::prepare(store, &fixtures, trace.is_some());
    let nodes = drivers.len();
    while drivers.len() < N {
        drivers.push(Driver::Inactive);
    }
    let cords = lowered.cords.len();
    let mut routes = FixedRoutes::<C, C>::new(1);
    for route in &lowered.routes {
        routes
            .install(
                route.source_node,
                route.source_port,
                route.range,
                &route.targets,
            )
            .unwrap();
    }
    routes.seal().unwrap();
    let mut bindings = conduit_kernel::FixedHostCallBindings::<1024>::new(1);
    for b in &lowered.host_calls {
        bindings.install(b.node, b.binding).unwrap();
    }
    bindings.seal().unwrap();
    let mut specs = lowered.node_specs.clone();
    specs.resize(
        N,
        conduit_kernel::scheduler::NodeSpec {
            input_cords: [None; PORTS],
            maximum_step_fuel: 0,
        },
    );
    let mut cord_specs: Vec<_> = lowered.cords.iter().map(|c| c.spec).collect();
    cord_specs.resize(C, conduit_kernel::scheduler::CordSpec::inactive());
    let sign_capacity = 32768u16;
    let mut scheduler=FixedScheduler::<_,_,_,N,C,PORTS,C,C,C,1024,1024>::new_with_active_counts_and_host_calls(nodes,cords,specs.try_into().unwrap(),cord_specs.try_into().unwrap(),routes,bindings,drivers.try_into().unwrap_or_else(|_|panic!("capacity")),store,HostedSignLog::new(sign_capacity,(usize::from(sign_capacity)*core::mem::size_of::<KernelEvent>()) as u32).unwrap()).unwrap();
    let preparation = start.elapsed();
    eprintln!("epoch owners prepared: {nodes} nodes/{cords} cords in {preparation:?}");
    let execute = Instant::now();
    let mut drained = false;
    let mut warm_retired = false;
    let mut stop_evidence = serde_json::Value::Null;
    let mut pressure_resumed = false;
    let mut pending_output = pending_output::PendingHostOutput::<16384>::new();
    let mut scheduler_step_allocations = 0;
    let mut prepared_expression_allocations = 0;
    let mut completed_host_calls = 0u64;
    let mut reported_rows = 0usize;
    let mut service_steps = 0u32;
    let mut stop_reason = "service_quantum_exhausted";
    let mut quantum_boundaries = 0u32;
    let mut prior_quantum_rows = 0usize;
    let mut quantum_receipts = vec![];
    for (quantum, local_step) in (0..service.quanta).flat_map(|q| (0..service.quantum).map(move |step| (q, step))) {
        let service_step = quantum * service.quantum + local_step;
        service_steps = service_step + 1;
        if local_step == 0 && quantum != 0 && service.observe_boundaries {
            let rows = received.borrow().len();
            let receipt = serde_json::json!({"completed_quantum":quantum,"services":service_step,"rows":rows,"completed_host_calls":completed_host_calls,"pending_host_calls":scheduler.pending_host_call_count(),"used_items":scheduler.values().used_items(),"used_bytes":scheduler.values().used_bytes(),"same_live_scheduler":true});
            eprintln!("epoch finite service continuation: {receipt}");
            quantum_receipts.push(receipt);
            quantum_boundaries += 1;
            if rows <= prior_quantum_rows { stop_reason = "quantum_without_output_progress"; break; }
            prior_quantum_rows = rows;
        }
        let (step, allocations) = if trace.is_some() {
            super::allocation_probe::measure(|| scheduler.step())
        } else {
            (scheduler.step(), 0)
        };
        scheduler_step_allocations += allocations;
        let status = match step {
            Ok(status) => status,
            Err(error) => {
                stop_reason = "scheduler_refusal";
                eprintln!("epoch scheduler refused: {error:?}");
                break;
            }
        };
        if matches!(
            status,
            conduit_kernel::scheduler::SchedulerStatus::Idle
                | conduit_kernel::scheduler::SchedulerStatus::Drained
                | conduit_kernel::scheduler::SchedulerStatus::Cancelled
        ) && scheduler.pending_host_call_count() == 0
        {
            stop_reason = "scheduler_settled";
            if matches!(mode, ExecutionMode::LifecycleWarm) && matches!(status, conduit_kernel::scheduler::SchedulerStatus::Idle) && received.borrow().len() == expected {
                let decisions=scheduler.decisions();
                let cursors=source_cursors(scheduler.drivers());
                scheduler.cancel().unwrap();
                assert_eq!(scheduler.decisions(),decisions);
                assert_eq!(source_cursors(scheduler.drivers()),cursors);
                assert_eq!(scheduler.pending_host_call_count(),0);
                assert_eq!(scheduler.values().used_items(),0);
                warm_retired=true;
                stop_reason="warm_source_products_complete_idle_explicit_cancel_retirement";
                stop_evidence=serde_json::json!({"decisions":decisions,"source_cursors":cursors,"complete_outputs":expected,"no_pending_host_work":true,"warm_cleanup_cancelled":true});
            }
            drained = matches!(status, conduit_kernel::scheduler::SchedulerStatus::Drained);
            eprintln!("epoch scheduler settled: {status:?}");
            break;
        }
        if received.borrow().len() != reported_rows || service_step % 16384 == 0 {
            reported_rows = received.borrow().len();
            eprintln!("epoch service step={service_step} rows={reported_rows}/{expected} completed_host_calls={completed_host_calls} pending_host_calls={} elapsed={:?}", scheduler.pending_host_call_count(), execute.elapsed());
            eprintln!("epoch storage used_items={}/{} used_bytes={}/{}", scheduler.values().used_items(), scheduler.values().item_capacity(), scheduler.values().used_bytes(), scheduler.values().byte_capacity());
        }
        if let Some(call) = scheduler.next_host_request() {
            if matches!(mode, ExecutionMode::LifecycleCancel | ExecutionMode::LifecycleProviderLost) {
                let decisions=scheduler.decisions();let cursors=source_cursors(scheduler.drivers());let rows=received.borrow().len();
                if matches!(mode, ExecutionMode::LifecycleProviderLost) {
                    assert!(owners.remove(&call.node).is_some() || filter_owners.remove(&call.node).is_some());
                }
                scheduler.cancel().unwrap();
                assert!(matches!(scheduler.step().unwrap(),conduit_kernel::scheduler::SchedulerStatus::Cancelled));
                assert_eq!(scheduler.decisions(),decisions);assert_eq!(source_cursors(scheduler.drivers()),cursors);assert_eq!(received.borrow().len(),rows);
                assert_eq!(scheduler.pending_host_call_count(),0);assert_eq!(scheduler.values().used_items(),0);
                stop_reason=if matches!(mode, ExecutionMode::LifecycleProviderLost) {"actual_selected_provider_owner_removed_and_scheduler_cancelled"} else {"actual_lifecycle_scheduler_cancelled"};
                stop_evidence=serde_json::json!({"decisions_before_after":decisions,"source_cursors_before_after":cursors,"rows_before_after":rows,"pending_after":0,"storage_items_after":0,"no_further_step_commit":true});break;
            }
            if matches!(mode, ExecutionMode::CancelFirstExpression) {
                if let Some(owner) = owners.get_mut(&call.node) {
                    owner.cancel(call.node, call.call).unwrap();
                } else {
                    assert!(filter_owners.contains_key(&call.node));
                }
                scheduler
                    .complete_host_call(
                        call.node,
                        call.request,
                        conduit_kernel::HostCallOutcome {
                            disposition: conduit_kernel::HostCallDisposition::Cancelled,
                            output: None,
                            failure: None,
                        },
                    )
                    .unwrap();
                // Deliver the ordinary host cancellation to its selected Back.
                for _ in 0..nodes {
                    let _ = scheduler.step();
                }
                break;
            }
            let input = scheduler.values().get(call.input.value).unwrap();
            let filter = filter_owners.get_mut(&call.node);
            let owner = if filter.is_none() {
                owners.get_mut(&call.node)
            } else {
                None
            };
            let invocation = move || match filter {
                Some(filter) => filter.execute(input).unwrap(),
                None => Some(
                    owner
                        .unwrap()
                        .invoke(call.node, call.call, call.request, input)
                        .unwrap(),
                ),
            };
            let (output, allocations) = if trace.is_some() {
                super::allocation_probe::measure(invocation)
            } else {
                (invocation(), 0)
            };
            prepared_expression_allocations += allocations;
            completed_host_calls += 1;
            let value = match output {
                Some(output) => {
                    if matches!(mode,ExecutionMode::LifecyclePressure) {
                        let decisions=scheduler.decisions();let cursors=source_cursors(scheduler.drivers());let rows=received.borrow().len();
                        while scheduler.values().used_items()<scheduler.values().item_capacity() {scheduler.store_host_value(&[]).unwrap();}
                        assert!(scheduler.store_host_value(output).is_err());
                        assert_eq!(scheduler.decisions(),decisions);assert_eq!(source_cursors(scheduler.drivers()),cursors);assert_eq!(received.borrow().len(),rows);
                        assert!(scheduler.pending_host_call_count()>0);
                        stop_reason="actual_output_store_pressure_before_host_completion";
                        stop_evidence=serde_json::json!({"decisions_before_after":decisions,"source_cursors_before_after":cursors,"rows_before_after":rows,"pending_host_calls":scheduler.pending_host_call_count(),"output_bytes":output.len(),"no_host_completion_or_step_commit":true});break;
                    }
                    if matches!(mode, ExecutionMode::LifecyclePressureResume) && !pressure_resumed {
                        pending_output.retain(call.node, call.request, output).unwrap();
                        let decisions = scheduler.decisions();
                        let cursors = source_cursors(scheduler.drivers());
                        let rows = received.borrow().len();
                        let mut own_pressure_leases = Vec::new();
                        while scheduler.values().used_items() < scheduler.values().item_capacity() {
                            own_pressure_leases.push(scheduler.store_host_value(&[]).unwrap());
                        }
                        assert!(pending_output.try_store(|bytes| scheduler.store_host_value(bytes)).is_err());
                        assert_eq!(scheduler.decisions(), decisions);
                        assert_eq!(source_cursors(scheduler.drivers()), cursors);
                        assert_eq!(received.borrow().len(), rows);
                        assert_eq!(pending_output.original().unwrap(), (call.node, call.request, output));
                        for lease in own_pressure_leases { scheduler.discard_host_value(lease).unwrap(); }
                        pending_output.try_store(|bytes| scheduler.store_host_value(bytes)).unwrap();
                        assert!(pending_output.try_complete(|node, request, value, length| scheduler.complete_host_call(node, request,
                            conduit_kernel::HostCallOutcome { disposition: conduit_kernel::HostCallDisposition::Completed,
                                output: Some(BoundedValueRef::new(value, length).unwrap()), failure: None })).unwrap());
                        assert!(pending_output.original().is_none());
                        pressure_resumed = true;
                        stop_evidence = serde_json::json!({"actual_store_capacity_refusal":true,"same_computed_output_retained":true,"no_owner_reinvocation":true,"decisions_at_pressure":decisions,"source_cursors_at_pressure":cursors,"rows_at_pressure":rows,"only_owned_pressure_leases_released":true});
                        continue;
                    }
                    let value = match scheduler.store_host_value(output) {
                        Ok(value) => value,
                        Err(error) => {
                            stop_reason = "host_output_storage_refusal";
                            eprintln!("epoch host output storage refused: {error:?}; service_step={service_step} output_bytes={} used_items={}/{} used_bytes={}/{}", output.len(), scheduler.values().used_items(), scheduler.values().item_capacity(), scheduler.values().used_bytes(), scheduler.values().byte_capacity());
                            break;
                        }
                    };
                    Some(BoundedValueRef::new(value, output.len() as u32).unwrap())
                }
                None => None,
            };
            scheduler
                .complete_host_call(
                    call.node,
                    call.request,
                    conduit_kernel::HostCallOutcome {
                        disposition: conduit_kernel::HostCallDisposition::Completed,
                        output: value,
                        failure: None,
                    },
                )
                .unwrap();
        }
        if received.borrow().len() == expected && !run_to_drain {
            stop_reason = "expected_rows_received";
            break;
        }
    }
    let execution = execute.elapsed();
    let encoded = received.borrow();
    let metrics = serde_json::json!({"nodes":nodes,"cords":cords,"service_steps":service_steps,"quantum_steps":service.quantum,"maximum_quanta":service.quanta,"observed_boundaries":quantum_boundaries,"boundary_receipts":quantum_receipts,"observe_boundaries":service.observe_boundaries,"stop_reason":stop_reason,"rows":encoded.len(),"expected_rows":expected,"completed_host_calls":completed_host_calls,"pending_host_calls":scheduler.pending_host_call_count(),"used_items":scheduler.values().used_items(),"item_capacity":scheduler.values().item_capacity(),"used_bytes":scheduler.values().used_bytes(),"byte_capacity":scheduler.values().byte_capacity(),"preparation_seconds":preparation.as_secs_f64(),"execution_seconds":execution.as_secs_f64(),"drained":drained,"warm_retired":warm_retired,"stop_evidence":stop_evidence,"pressure_resumed":pressure_resumed});
    eprintln!("epoch terminal service metrics: {metrics}");
    if let Ok(directory) = std::env::var("CONDUIT_FARGAN_NATIVE_OUTPUT") {
        std::fs::write(std::path::Path::new(&directory).join(format!("service-{nodes}-{expected}-{}.json", if service.observe_boundaries { "bounded" } else { "uninterrupted" })), serde_json::to_vec_pretty(&metrics).unwrap()).unwrap();
        std::fs::write(std::path::Path::new(&directory).join(format!("service-{nodes}-{expected}-{}-rows.json", if service.observe_boundaries { "bounded" } else { "uninterrupted" })), serde_json::to_vec(&*encoded).unwrap()).unwrap();
    }
    if encoded.len() != expected && matches!(mode, ExecutionMode::Normal) {
        let events: Vec<_> = scheduler.signs().events().collect();
        eprintln!(
            "no epoch output after {:?}; preparation {:?}; {} signs",
            execution,
            preparation,
            events.len()
        );
        for event in events.iter().rev().take(40).rev() {
            eprintln!("epoch sign: {event:?}");
        }
        for (index, gear) in fragment.placements.iter().enumerate() {
            let routed = events
                .iter()
                .filter(|event| {
                    event.node.0 as usize == index
                        && event.kind == conduit_kernel::KernelEventKind::ValueRouted
                })
                .count();
            let consumed = events
                .iter()
                .filter(|event| {
                    event.node.0 as usize == index
                        && event.kind == conduit_kernel::KernelEventKind::ValueConsumed
                })
                .count();
            if routed != 0 || consumed != 0 {
                eprintln!(
                    "epoch node {index} {:?} {:?}: routed={routed} consumed={consumed}",
                    gear.gear_id, gear.kind_id
                );
            }
        }
    }
    if trace.is_some_and(|trace| !trace.finished()) {
        return None;
    }
    if encoded.len() != expected || (run_to_drain && !drained && !warm_retired) {
        return None;
    }
    Some(StreamResultAndTiming {
        values: encoded
            .iter()
            .map(|v| StructuredInfoValue::from_canonical_bytes(v).unwrap())
            .collect(),
        preparation,
        execution,
        nodes,
        cords,
        drained,
        scheduler_step_allocations,
        prepared_expression_allocations,
        service_steps,
        completed_host_calls,
        pressure_resumed,
        quantum_boundaries,
        warm_retired,
    })
}

#[test]
fn ordinary_epoch_scheduler_commits_canonical_pcm16_and_refuses_pressure_or_cancel() {
    std::thread::Builder::new().stack_size(32*1024*1024).spawn(|| {
        let planning=Instant::now();
        let (plan,context)=super::prepared_epoch_plan(true).unwrap();
        let planning=planning.elapsed();
        let resources=synthetic_resources(&plan);
        assert_eq!(resources.len(),26);
        let input=context.native.iter().find(|profile| profile.kind_identity(true)==context.kinds["__EPOCH_INPUT_VALIDATOR__"]).unwrap().value_type();
        let encoded=super::declarations::fixture_value(input).canonical_bytes().unwrap();
        assert_eq!(validate_canonical_structured_value(&encoded).unwrap().record_field("model").unwrap().unwrap().record_field("precision").unwrap().unwrap().variant_tag().unwrap(), "reference_float32");
        let fixtures=BTreeMap::from([("value".into(),encoded)]);
        let result=run_epoch_plan(plan.clone(),&context,&resources,fixtures.clone(),ExecutionMode::Normal).expect("ordinary source-selected epoch must publish PCM16");
        let StructuredInfoValueShape::Record(fields)=result.value.shape() else {panic!("native PCM aggregate")};
        let pcm=fields.iter().find(|field| field.name()=="pcm_i16").unwrap().value();
        let StructuredInfoValueShape::Collection(samples)=pcm.shape() else {panic!("exact PCM vector")};
        assert_eq!(samples.len(),160);
        assert!(samples.iter().any(|sample| {let StructuredInfoValueShape::Leaf(bytes)=sample.shape() else {panic!("I16 sample")}; i16::from_le_bytes(bytes.try_into().unwrap())!=0}));
        assert!(run_epoch_plan(plan.clone(),&context,&resources,fixtures.clone(),ExecutionMode::StoragePressure).is_none());
        assert!(run_epoch_plan(plan,&context,&resources,fixtures,ExecutionMode::CancelFirstExpression).is_none());
        eprintln!("synthetic epoch: planning{planning:?}, owner preparation{:?}, execution{:?},{}nodes/{}cords; no pretrained or streaming feedback claim",result.preparation,result.execution,result.nodes,result.cords);
    }).unwrap().join().unwrap();
}

#[test]
fn ordinary_signal_cycle_reuses_four_subframes_and_drains_final_feedback() {
    std::thread::Builder::new().stack_size(32*1024*1024).spawn(|| {
        let planning=Instant::now();
        let (plan,context,seeded)=super::prepared_signal_cycle_plan();
        let planning=planning.elapsed();
        let resources=synthetic_resources(&plan);assert_eq!(resources.len(),26);
        let source_definition=super::declarations::exact_epoch_declarations()+"\n"+include_str!("../../../speech/fargan_epoch_feedback.conduit");
        let checked=conduit_plot::check_syntax_document(&conduit_plot::parse_syntax_document(&source_definition),&conduit_plot::StartupCatalog::new()).unwrap();
        let ty=|name:&str|&checked.native_types.iter().find(|t|t.name==name).unwrap().value_type;
        let seed=super::epoch_pair_fixture(ty("FarganSignalEpochFeedback"),"",7,7).canonical_bytes().unwrap();
        let events=(7..10).map(|epoch|super::epoch_pair_fixture(ty("FarganSignalConditionEpoch"),"",epoch,epoch).canonical_bytes().unwrap()).collect();
        let inputs=BTreeMap::from([("seed".into(),vec![seed]),("events".into(),events)]);
        let result=run_epoch_stream_plan(plan,&context,&resources,inputs,Some(seeded),3,ExecutionMode::Normal).expect("exact three-frame Source cycle must commit and drain final returned state");
        assert!(result.drained);assert_eq!(result.values.len(),3);
        for (index,result) in result.values.iter().enumerate() {
            let encoded=result.canonical_bytes().unwrap();
            let v=validate_canonical_structured_value(&encoded).unwrap();
            let epoch=v.record_field("epoch").unwrap().unwrap().primitive_bytes("value/u64").unwrap();
            assert_eq!(u64::from_le_bytes(epoch.try_into().unwrap()),7+index as u64);
            let pcm=v.record_field("pcm_i16").unwrap().unwrap();assert_eq!(pcm.collection_length().unwrap(),160);
        }
        eprintln!("synthetic closing signal cycle: planning{planning:?} ownerprep{:?} execute{:?} {} nodes {} cords; three160sample aggregates; no warmup/pretrained/nativeutterance claim",result.preparation,result.execution,result.nodes,result.cords);
    }).unwrap().join().unwrap();
}

#[test]
#[ignore = "private pinned scalar float oracle and model; set CONDUIT_FARGAN_MODEL_FIXTURE"]
fn pinned_source_signal_cycle_retains_model_and_measures_free_running_error() {
    std::thread::Builder::new().stack_size(32*1024*1024).spawn(|| {
        use super::case_state::*;
        use sha2::{Digest,Sha256};
        let root=std::path::PathBuf::from(std::env::var("CONDUIT_FARGAN_MODEL_FIXTURE").unwrap());
        let model=super::custody::RetainedSignalModel::load(&root);
        let oracle=std::fs::read(root.join("subframe-float-oracle.bin")).unwrap();
        assert_eq!(format!("{:x}",Sha256::digest(&oracle)),"bf52f0431af52435e775818d7e7fe65ccf8b6ff7de38e57eab4b0db9935feccf");
        assert_eq!(oracle.len(),96*7180);
        let records:Vec<_>=oracle.as_chunks::<7180>().0.iter().map(|record| {
            let period=u16::try_from(i32::from_le_bytes(record[..4].try_into().unwrap())).unwrap();
            let floats:Vec<_>=record[4..].as_chunks::<4>().0.iter().map(|v|f32::from_le_bytes(*v)).collect();assert!(floats.iter().all(|v|v.is_finite()));(period,floats)
        }).collect();
        let (context,seeded,ids)=super::prepared_signal_cycle_profiles_with_capacity(true);
        for id in ids.values() {assert!(include_str!("../../../speech/fargan_signal_cycle.conduit").contains(id));}
        let template=super::signal_cycle_template();
        let checked_template=conduit_plot::check_syntax_document(&conduit_plot::parse_syntax_document(&template),&context.startup).unwrap();
        assert!(checked_template.plots.iter().any(|p|p.name=="speech/flow-fargan-signal-cycle"));
        let material=model.basis_material(&template,b"pinned scalar oracle development; no native voice or linguistic commitment",&oracle);
        let basis=semantic_digest("speech/fargan-signal-development-basis@1",&material);
        let selected=super::custody::anchor_literal(&model,basis);
        let source=template.replace("selected: FarganModelFrameAnchor\n",&format!("selected: FarganModelFrameAnchor = {selected}\n"));
        let (plan,context)=super::prepare_authored_epoch_entry(context,source,"speech/flow-fargan-signal-cycle",true,seeded.offers().cloned().collect()).unwrap();
        let definition=super::declarations::exact_epoch_declarations()+"\n"+include_str!("../../../speech/fargan_epoch_feedback.conduit");
        let types=conduit_plot::check_syntax_document(&conduit_plot::parse_syntax_document(&definition),&conduit_plot::StartupCatalog::new()).unwrap();
        let ty=|name:&str|&types.native_types.iter().find(|t|t.name==name).unwrap().value_type;
        fn record(ty:&StructuredInfoType,mut field:impl FnMut(&str,&StructuredInfoType)->StructuredInfoValue)->StructuredInfoValue {
            match ty.shape() {StructuredInfoTypeShape::Nominal {representation,..}=>StructuredInfoValue::nominal(ty.clone(),record(representation,field)).unwrap(),StructuredInfoTypeShape::Record {fields,..}=>StructuredInfoValue::record(ty.clone(),fields.iter().map(|f|StructuredFieldValue::new(f.name(),field(f.name(),f.value_type())).unwrap()).collect()).unwrap(),_=>panic!("record")}
        }
        fn u16value(ty:&StructuredInfoType,x:u16)->StructuredInfoValue {match ty.shape(){StructuredInfoTypeShape::Nominal {representation,..}=>StructuredInfoValue::nominal(ty.clone(),u16value(representation,x)).unwrap(),_=>StructuredInfoValue::leaf(ty.clone(),x.to_le_bytes().to_vec()).unwrap()}}
        let prior=State::from_oracle(&records[0].1[80..917]);
        let seed=record(ty("FarganSignalEpochFeedback"),|name,ty|match name {"state"=>StructuredInfoValue::from_canonical_bytes(&prior.encode(ty)).unwrap(),"conditioned_period"=>u16value(ty,records[0].0),"next_epoch"=>StructuredInfoValue::leaf(ty.clone(),0u64.to_le_bytes().to_vec()).unwrap(),_=>panic!("seed field")}).canonical_bytes().unwrap();
        let events:Vec<_>=records.as_chunks::<4>().0.iter().enumerate().map(|(epoch,group)| {
            assert!(group.iter().all(|r|r.0==group[0].0));
            let condition:Vec<_>=group.iter().flat_map(|r|r.1[..80].iter().copied()).collect();
            let next=records.get((epoch+1)*4).map_or(group[0].0,|r|r.0);
            record(ty("FarganSignalConditionEpoch"),|name,ty|match name {"condition"=>vector(ty,&condition),"next_period"=>u16value(ty,next),"epoch"=>StructuredInfoValue::leaf(ty.clone(),(epoch as u64).to_le_bytes().to_vec()).unwrap(),_=>panic!("event field")}).canonical_bytes().unwrap()
        }).collect();
        let result=run_epoch_stream_plan(plan,&context,&model.resources,BTreeMap::from([("seed".into(),vec![seed]),("events".into(),events)]),Some(seeded),24,ExecutionMode::Normal).expect("pinned signal cycle must commit24 frames and final returned state");
        assert!(result.drained);
        let mut maximum_pcm=0i32;let mut pcm_squared=0f64;let mut maximum_state=0f32;
        for (epoch,value) in result.values.iter().enumerate() {
            let actual=field(value,"pcm_i16");let StructuredInfoValueShape::Collection(actual)=actual.shape() else {panic!("pcm")};
            let expected:Vec<_>=records[epoch*4..epoch*4+4].iter().flat_map(|r|r.1[917..957].iter()).map(|sample|(sample.clamp(-1.,32767./32768.)*32768.).round() as i16).collect();
            for (sample,expected) in actual.iter().zip(expected) {let StructuredInfoValueShape::Leaf(raw)=sample.shape() else {panic!("I16")};let error=(i32::from(i16::from_le_bytes(raw.try_into().unwrap()))-i32::from(expected)).abs();maximum_pcm=maximum_pcm.max(error);pcm_squared+=f64::from(error).powi(2);}
            let state=State::from_result(value).flattened();let expected=&records[epoch*4+3].1[957..];
            for (a,b) in state.iter().zip(expected) {maximum_state=maximum_state.max((a-b).abs());}
        }
        eprintln!("pinned free-running signal24epochs: maxPCM16error={maximum_pcm}, RMSPCM16error={}, maxstateabs={maximum_state}, prep{:?}, execute{:?}; C-conditioning inputs and C-warmed seed; no Source startup/native utterance/bit parity claim",(pcm_squared/(24.*160.)).sqrt(),result.preparation,result.execution);
        // Empirical regression bounds for this pinned24epoch trace only.
        // Same float storage does not imply C activation/rounding parity.
        assert!(maximum_state.is_finite() && maximum_state <= 0.006);
        assert!(maximum_pcm <= 1);
        assert!((pcm_squared/(24.*160.)).sqrt() <= 0.15);
        assert_eq!(result.values.len(),24);
    }).unwrap().join().unwrap();
}

#[test]
#[ignore = "private shared native feature receipt and pinned model; set CONDUIT_FARGAN_MODEL_FIXTURE and CONDUIT_FARGAN_FEATURE_RECEIPT"]
fn retained_native_feature_executes_authored_conditioning_with_shared_tensor_custody() {
    run_retained_native_conditioning(false);
}

#[test]
#[ignore = "private native feature, model and five-update C oracle"]
fn retained_native_feature_warms_five_authored_conditioning_calls() {
    run_retained_native_conditioning(true);
}

fn run_retained_native_conditioning(warm: bool) {
    std::thread::Builder::new().stack_size(32*1024*1024).spawn(move || {
        use super::case_state::vector;
        let root=std::path::PathBuf::from(std::env::var("CONDUIT_FARGAN_MODEL_FIXTURE").unwrap());
        let model=super::custody::RetainedSignalModel::load(&root);
        let (layout,resources)=model.conditioning_resources();
        let interface=if warm {None} else {Some(super::interface::ConditioningInterface::prepare(model.conditioning_descriptor(),layout.clone()).unwrap())};
        let evidence=std::fs::read(std::env::var("CONDUIT_FARGAN_FEATURE_RECEIPT").unwrap()).unwrap();
        use sha2::{Digest,Sha256};
        let original_feature_digest="b8a72c0c8eedb09fbd56255180e37f1b96c65ff9b3b438672599ff5c9a2650d8";
        let expected_feature_digest=std::env::var("CONDUIT_FARGAN_FEATURE_RECEIPT_SHA256").unwrap_or_else(|_|original_feature_digest.into());
        assert_eq!(format!("{:x}",Sha256::digest(&evidence)),expected_feature_digest);
        if warm {assert_eq!(expected_feature_digest,original_feature_digest,"the pinned warm oracle belongs to the original feature receipt");}
        let receipt:serde_json::Value=serde_json::from_slice(&evidence).unwrap();
        assert_eq!(receipt["joint_committed_session"],false);
        let features:Vec<f32>=receipt["feature20"].as_array().unwrap().iter().map(|v|v.as_f64().unwrap() as f32).collect();
        assert_eq!(features.len(),20);
        let period=u16::try_from(receipt["feature_period_samples_16k"].as_u64().unwrap()).unwrap();
        assert!((32..=255).contains(&period));
        let context=super::prepared_epoch_profiles_with_capacity(true);
        let source=if warm {String::from(include_str!("../../../speech/fargan_conditioning.conduit"))+"\n"+include_str!("../../../speech/fargan_warm_conditioning.conduit")} else {interface.as_ref().unwrap().source.clone()};
        let entry=if warm {"speech/fargan-warm-first-conditioning"} else {"speech/flow-fargan-conditioning-core"};
        let history_name=if warm {"initial_history"} else {"history"};
        let (plan,context)=super::prepare_authored_epoch_entry(context,source.clone(),entry,true,vec![]).unwrap();
        let binding=|name:&str|match name {"features"=>conduit_ai::fixed_numeric_catalog::fixed_numeric_type("NumericF32Vector20").unwrap(),"history"=>conduit_ai::fixed_numeric_catalog::fixed_numeric_type("NumericHistory2x64").unwrap(),"period"=>conduit_ai::fixed_numeric_u16_profile::PreparedU16Profile::check_definition("type FarganPeriod = U16 in 32..=255\n").unwrap().value_type().clone(),_=>panic!("input")};
        fn scalar(ty:&StructuredInfoType,x:u16)->StructuredInfoValue {match ty.shape(){StructuredInfoTypeShape::Nominal {representation,..}=>StructuredInfoValue::nominal(ty.clone(),scalar(representation,x)).unwrap(),_=>StructuredInfoValue::leaf(ty.clone(),x.to_le_bytes().to_vec()).unwrap()}}
        let inputs=BTreeMap::from([("features".into(),vec![vector(&binding("features"),&features).canonical_bytes().unwrap()]),("period".into(),vec![scalar(&binding("period"),period).canonical_bytes().unwrap()]),(history_name.into(),vec![vector(&binding("history"),&[0.;128]).canonical_bytes().unwrap()])]);
        let result=run_epoch_stream_plan(plan,&context,&resources,inputs,None,2,ExecutionMode::Normal).expect("actual native feature must execute Source conditioning");
        let outputs:Vec<_>=result.values.iter().map(super::case_state::floats).collect();
        let mut widths:Vec<_>=outputs.iter().map(Vec::len).collect();widths.sort();assert_eq!(widths,[128,320]);
        assert!(outputs.iter().flatten().all(|v|v.is_finite()));
        if warm {
            let oracle=std::fs::read(root.join("native-first-five-oracle.bin")).unwrap();
            assert_eq!(oracle.len(),5*1800);
            assert_eq!(format!("{:x}",Sha256::digest(&oracle)),"8c151416cc237bbb9f911cc76065e42be0e781248440b569b2550203e95c84c2");
            let last=&oracle[4*1800..];
            assert_eq!(i32::from_le_bytes(last[1796..1800].try_into().unwrap()),i32::from(period));
            let expected:Vec<_>=last[..1792].as_chunks::<4>().0.iter().map(|v|f32::from_le_bytes(*v)).collect();
            let condition=outputs.iter().find(|v|v.len()==320).unwrap();let history=outputs.iter().find(|v|v.len()==128).unwrap();
            let max_condition=condition.iter().zip(&expected[..320]).map(|(a,b)|(a-b).abs()).fold(0f32,f32::max);
            let max_history=history.iter().zip(&expected[320..]).map(|(a,b)|(a-b).abs()).fold(0f32,f32::max);
            eprintln!("Source first-feature five-update warm conditioning vs pinned fullfloat C: maxcondition={max_condition}, maxhistory={max_history}; no signal-continuation/fullstartup claim");
            assert!(max_condition.is_finite() && max_condition <= 0.0003);
            assert!(max_history.is_finite() && max_history <= 0.0001);
        }
        if let Ok(path)=std::env::var("CONDUIT_FARGAN_CONDITIONING_EVIDENCE") {
            let output=serde_json::json!({"feature_execution_receipt":receipt,"conditioning_source":source,"conditioning_source_sha256":format!("{:x}",Sha256::digest(source.as_bytes())),"conditioning_layout":serde_json::from_slice::<serde_json::Value>(&layout).unwrap(),"model_raw_blob_sha256":model.raw_blob_sha256,"signal_only_model_artifact_identity":model.model.artifact().content_identity(),"signal_only_model_descriptor_identity":model.model.descriptor_identity(),"condition320":outputs.iter().find(|v|v.len()==320).unwrap(),"next_history128":outputs.iter().find(|v|v.len()==128).unwrap(),"initial_history128":vec![0f32;128],"graph_nodes":result.nodes,"graph_cords":result.cords,"owner_preparation_ns":result.preparation.as_nanos().to_string(),"execution_ns":result.execution.as_nanos().to_string(),"joint_committed_session":false,"conditioner_model_signature_admitted":interface.is_some(),"source_native_carrier_correspondence_checked":interface.is_some(),"model_compute_lifecycle_admitted":false,"conditioner_interface":interface.as_ref().map(|i|serde_json::json!({"model_resource_reference_canonical":i.model.artifact().content.encode().unwrap(),"model_artifact_descriptor":format!("{:?}",i.model.artifact()),"logical_model_signature_canonical":conduit_plot::rust_binding::NativeRustBinding::encode(i.model.signature().clone()).unwrap(),"descriptor_identity":i.model.descriptor_identity(),"source_front":format!("{:?}",i.front),"native_ports_canonical":i.native_ports.iter().map(|(name,ty)|(name.clone(),ty.canonical_bytes().unwrap())).collect::<BTreeMap<_,_>>(),"period_definition":"type FarganPeriod = U16 in 32..=255\n","reviewed_layout":serde_json::from_slice::<serde_json::Value>(&i.layout).unwrap()})),"source_warm_initialization":false,"source_repeat_first_five_conditioning":warm,"neural_waveform":false});
            std::fs::write(path,serde_json::to_vec_pretty(&output).unwrap()).unwrap();
        }
        eprintln!("native feature20→authored conditioning320/history128: {}nodes/{}cords, prep{:?}, execution{:?}, retained7slices/{}layoutbytes; explicit zero-history development seed; no joint committed session/startup/native waveform claim",result.nodes,result.cords,result.preparation,result.execution,layout.len());
    }).unwrap().join().unwrap();
}

#[test]
#[ignore = "private native feature, model and scalar warm-start oracle"]
fn source_warm_startup_executes_five_condition_updates_and_four_zero_continuations() {
    std::thread::Builder::new().stack_size(32*1024*1024).spawn(|| {
        use super::case_state::{vector,State};
        use sha2::{Digest,Sha256};
        let root=std::path::PathBuf::from(std::env::var("CONDUIT_FARGAN_MODEL_FIXTURE").unwrap());
        let mut model=super::custody::RetainedSignalModel::load(&root);
        let (_,conditioning)=model.conditioning_resources();
        for (name,value) in conditioning {assert!(model.resources.insert(format!("conditioning_{name}"),value).is_none());}
        assert_eq!(model.resources.len(),33);
        let evidence=std::fs::read(std::env::var("CONDUIT_FARGAN_FEATURE_RECEIPT").unwrap()).unwrap();
        assert_eq!(format!("{:x}",Sha256::digest(&evidence)),"b8a72c0c8eedb09fbd56255180e37f1b96c65ff9b3b438672599ff5c9a2650d8");
        let receipt:serde_json::Value=serde_json::from_slice(&evidence).unwrap();
        let features:Vec<f32>=receipt["feature20"].as_array().unwrap().iter().map(|v|v.as_f64().unwrap() as f32).collect();
        let period=u16::try_from(receipt["feature_period_samples_16k"].as_u64().unwrap()).unwrap();
        let source=[include_str!("../../../speech/fargan_conditioning.conduit"),include_str!("../../../speech/fargan_signal.conduit"),include_str!("../../../speech/fargan_pitch_history.conduit"),include_str!("../../../speech/fargan_subframe.conduit"),include_str!("../../../speech/fargan_warm_conditioning.conduit"),include_str!("../../../speech/fargan_zero_continuation.conduit"),include_str!("../../../speech/fargan_warm_startup.conduit")].join("\n");
        let planning=Instant::now();
        let (plan,context)=super::prepare_authored_epoch_entry(super::prepared_epoch_profiles_with_capacity(true),source,"speech/fargan-warm-startup",true,vec![]).unwrap();
        let planning=planning.elapsed();
        let period_type=conduit_ai::fixed_numeric_u16_profile::PreparedU16Profile::check_definition("type FarganPeriod = U16 in 32..=255\n").unwrap().value_type().clone();
        fn scalar(ty:&StructuredInfoType,x:u16)->StructuredInfoValue {match ty.shape(){StructuredInfoTypeShape::Nominal {representation,..}=>StructuredInfoValue::nominal(ty.clone(),scalar(representation,x)).unwrap(),_=>StructuredInfoValue::leaf(ty.clone(),x.to_le_bytes().to_vec()).unwrap()}}
        let inputs=BTreeMap::from([("first_features".into(),vec![vector(&conduit_ai::fixed_numeric_catalog::fixed_numeric_type("NumericF32Vector20").unwrap(),&features).canonical_bytes().unwrap()]),("first_period".into(),vec![scalar(&period_type,period).canonical_bytes().unwrap()])]);
        let result=run_epoch_stream_plan(plan,&context,&model.resources,inputs,None,3,ExecutionMode::Normal).expect("Source warm-start must commit state/history/period");
        let state=result.values.iter().find(|v|matches!(v.shape(),StructuredInfoValueShape::Record(_))).unwrap();
        let actual=State::from_state(state).flattened();
        let oracle=std::fs::read(root.join("native-first-warm-subframe.bin")).unwrap();
        assert_eq!(format!("{:x}",Sha256::digest(&oracle)),"a81f44cfe73d1a2f790199b229ae01dd55defa7f5b99ed390bf8c271812ae202");
        let expected:Vec<_>=oracle[324..324+837*4].as_chunks::<4>().0.iter().map(|v|f32::from_le_bytes(*v)).collect();
        let maximum=actual.iter().zip(&expected).map(|(a,b)|(a-b).abs()).fold(0f32,f32::max);
        assert!(actual.iter().all(|v|v.is_finite()) && maximum.is_finite());
        assert!(maximum <= 0.001, "pinned fullfloat warm-state tolerance exceeded: {maximum}");
        assert!(actual[580..].iter().all(|v|*v==0.));
        eprintln!("Source warm startup fiveconditioning/fourcontinuation: {}nodes/{}cords, planning{planning:?}, prep{:?}, execution{:?}, maxstateabs={maximum}; actual retained feature point; no compound signature/jointcommit/native waveform claim",result.nodes,result.cords,result.preparation,result.execution);
    }).unwrap().join().unwrap();
}

#[test]
#[ignore = "private pinned model fixture; two complete trained-resource Source feedback epochs"]
fn retained_model_executes_compound_conditioning_signal_cycles_with_final_pcm_ack() {
    std::thread::Builder::new().stack_size(32*1024*1024).spawn(|| {
        let root=std::path::PathBuf::from(std::env::var("CONDUIT_FARGAN_MODEL_FIXTURE").unwrap());
        let mut model=super::custody::RetainedSignalModel::load(&root);
        let (_,conditioning)=model.conditioning_resources();
        for (name,resource) in conditioning {assert!(model.resources.insert(format!("conditioning_{name}"),resource).is_none());}
        assert_eq!(model.resources.len(),33);
        let (context,seeded,_)=super::prepared_signal_cycle_profiles_with_capacity(true);
        let (context,seeded,_)=super::conditioning_cycle::prepare_with(context,seeded);
        let source=super::conditioning_cycle::compound_source();
        let basis=model.basis_material(&source,b"synthetic bounded zero-history seed; not linguistic admission",b"two synthetic feature events");
        use sha2::{Digest,Sha256};
        let selected=super::custody::anchor_literal(&model,Sha256::digest(&basis).into());
        let source=source.replace("selected: FarganModelFrameAnchor\n",&format!("selected: FarganModelFrameAnchor = {selected}\n"));
        let planning=Instant::now();
        let (plan,context)=super::prepare_authored_epoch_entry(context,source,"speech/flow-fargan-compound-cycle",true,seeded.offers().cloned().collect()).unwrap();
        let planning=planning.elapsed();
        let definition=super::declarations::exact_epoch_declarations()+"\n"+include_str!("../../../speech/fargan_epoch_feedback.conduit")+"\n"+include_str!("../../../speech/fargan_conditioning_epoch_contracts.conduit");
        let checked=conduit_plot::check_syntax_document(&conduit_plot::parse_syntax_document(&definition),&conduit_plot::StartupCatalog::new()).unwrap();
        let ty=|name:&str|&checked.native_types.iter().find(|t|t.name==name).unwrap().value_type;
        let inputs=BTreeMap::from([
            ("signal_seed".into(),vec![super::epoch_pair_fixture(ty("FarganSignalEpochFeedback"),"",7,7).canonical_bytes().unwrap()]),
            ("conditioning_seed".into(),vec![super::epoch_pair_fixture(ty("FarganConditioningEpochFeedback"),"",7,7).canonical_bytes().unwrap()]),
            ("events".into(),[7,8].iter().map(|epoch|super::epoch_pair_fixture(ty("FarganFeatureConditionEpoch"),"",*epoch,*epoch).canonical_bytes().unwrap()).collect()),
        ]);
        let result=run_epoch_stream_plan(plan,&context,&model.resources,inputs,Some(seeded),2,ExecutionMode::Normal).expect("both Source feedback cells must receive exact final PCM epoch acknowledgments and drain");
        assert!(result.drained);assert_eq!(result.values.len(),2);
        for (epoch,value) in [7u64,8].iter().zip(&result.values) {
            let raw=super::case_state::field(value,"epoch");
            let StructuredInfoValueShape::Leaf(raw)=raw.shape() else {panic!("epoch")};
            assert_eq!(u64::from_le_bytes(raw.try_into().unwrap()),*epoch);
            let StructuredInfoValueShape::Collection(pcm)=super::case_state::field(value,"pcm_i16").shape() else {panic!("PCM")};
            assert_eq!(pcm.len(),160);
            assert!(super::case_state::State::from_result(value).flattened().iter().all(|v|v.is_finite()));
        }
        eprintln!("trained-resource Source compound two epochs: {}nodes/{}cords, planning{planning:?}, prep{:?}, execute{:?}; shared33tensor custody, zero/synthetic seed/features; no compound signature/native linguistic/session/waveform quality admission",result.nodes,result.cords,result.preparation,result.execution);
    }).unwrap().join().unwrap();
}

#[test]
fn closing_feature_graph_reuses_public_analysis_resources_for_three_frames() {
    std::thread::Builder::new().stack_size(32 * 1024 * 1024).spawn(|| {
        use super::case_state::vector;
        let source = super::feature_cycle::feature_source(true);
        let context = super::prepared_epoch_profiles_with_capacity(true);
        let planning = Instant::now();
        let (plan, context) = super::prepare_authored_epoch_entry(context, source,
            "speech/flow-fargan-feature-frame", true, vec![]).unwrap();
        let planning = planning.elapsed();
        let load = |bytes: &[u8]| bytes.as_chunks::<4>().0.iter().map(|v| f32::from_le_bytes(*v)).collect();
        let resources: Resources = [
            ("band_weights", "NumericF32MatrixRef161x18", vec![161,18], load(include_bytes!("../../../../proof/fargan/feature-profile/bands161x18.bin"))),
            ("band_bias", "NumericF32BiasRef18", vec![18], load(include_bytes!("../../../../proof/fargan/feature-profile/band_bias18.bin"))),
            ("dct_weights", "NumericF32MatrixRef18x18", vec![18,18], load(include_bytes!("../../../../proof/fargan/feature-profile/dct18x18.bin"))),
        ].into_iter().map(|(name, ty, dimensions, values)| (name.into(), Resource::new(
            conduit_ai::fixed_numeric_catalog::fixed_numeric_type(ty).unwrap(), &dimensions, values))).collect();
        let waveform: Vec<f32> = (0..640).map(|n| 0.02 * (std::f32::consts::TAU * n as f32 / 80.).sin()).collect();
        let waveform = vector(&conduit_ai::fixed_numeric_catalog::fixed_numeric_type("NumericF32Vector640").unwrap(), &waveform).canonical_bytes().unwrap();
        let period = conduit_ai::fixed_numeric_u16_profile::PreparedU16Profile::check_definition("type FarganPeriod = U16 in 32..=255\n").unwrap();
        let StructuredInfoTypeShape::Nominal { representation, .. } = period.value_type().shape() else { panic!("exact period profile"); };
        let period = StructuredInfoValue::nominal(period.value_type().clone(), StructuredInfoValue::leaf(representation.clone(), 80u16.to_le_bytes().to_vec()).unwrap()).unwrap().canonical_bytes().unwrap();
        let inputs = BTreeMap::from([("waveform".into(), vec![waveform;3]), ("period".into(), vec![period;3])]);
        let result = run_epoch_stream_plan(plan.clone(), &context, &resources, inputs.clone(), None, 3, ExecutionMode::Normal).expect("explicit closing feature owners must publish every frame");
        assert!(run_epoch_stream_plan(plan.clone(), &context, &resources, inputs.clone(), None, 3, ExecutionMode::StoragePressure).is_none());
        assert!(run_epoch_stream_plan(plan, &context, &resources, inputs, None, 3, ExecutionMode::CancelFirstExpression).is_none());
        assert_eq!(result.values.len(),3);
        for value in &result.values[1..] { assert_eq!(value.canonical_bytes().unwrap(), result.values[0].canonical_bytes().unwrap()); }
        eprintln!("three-frame Source feature stream: {}nodes/{}cords, planning{planning:?}, preparation{:?}, execution{:?}; no causal feedback/native utterance waveform claim", result.nodes,result.cords,result.preparation,result.execution);
    }).unwrap().join().unwrap();
}

#[test]
fn native_feature_analysis_stream_commits_exact_provisional_memories_for_two_pcm_epochs() {
    std::thread::Builder::new().stack_size(32*1024*1024).spawn(|| {
        let (context,_,ids)=super::feature_cycle::prepare_feedback();
        let input_type=context.native.iter().find(|p|p.kind_identity(true)==ids["__FEATURE_INPUT_NATIVE__"]).unwrap().value_type().clone();
        let source=super::feature_cycle::analysis_entry_source(&ids);
        let (plan,context)=super::prepare_authored_epoch_entry(context,source,"speech/flow-fargan-feature-native-analysis",true,vec![]).unwrap();
        let load=|bytes:&[u8]|bytes.as_chunks::<4>().0.iter().map(|v|f32::from_le_bytes(*v)).collect();
        let resources:Resources=[("band_weights","NumericF32MatrixRef161x18",vec![161,18],load(include_bytes!("../../../../proof/fargan/feature-profile/bands161x18.bin"))),("band_bias","NumericF32BiasRef18",vec![18],load(include_bytes!("../../../../proof/fargan/feature-profile/band_bias18.bin"))),("dct_weights","NumericF32MatrixRef18x18",vec![18,18],load(include_bytes!("../../../../proof/fargan/feature-profile/dct18x18.bin")))].into_iter().map(|(name,ty,dims,values)|(name.into(),Resource::new(conduit_ai::fixed_numeric_catalog::fixed_numeric_type(ty).unwrap(),&dims,values))).collect();
        fn field<'a>(value:&'a StructuredInfoValue,name:&str)->&'a StructuredInfoValue {let StructuredInfoValueShape::Record(fields)=value.shape() else {panic!("record")};fields.iter().find(|f|f.name()==name).unwrap().value()}
        fn constant_i16(ty:&StructuredInfoType,x:i16)->StructuredInfoValue {match ty.shape(){StructuredInfoTypeShape::Nominal{representation,..}=>StructuredInfoValue::nominal(ty.clone(),constant_i16(representation,x)).unwrap(),StructuredInfoTypeShape::Collection{element,length}=>StructuredInfoValue::collection(ty.clone(),(0..length).map(|_|constant_i16(element,x)).collect()).unwrap(),_=>StructuredInfoValue::leaf(ty.clone(),x.to_le_bytes().to_vec()).unwrap()}}
        let fixture=super::declarations::fixture_value(&input_type);
        let StructuredInfoValueShape::Record(fields)=fixture.shape() else {panic!("input")};
        let zero=StructuredInfoValue::record(input_type.clone(),fields.iter().map(|f|StructuredFieldValue::new(f.name(),if f.name()=="epoch" {StructuredInfoValue::leaf(f.value().value_type().clone(),0u64.to_le_bytes().to_vec()).unwrap()} else {f.value().clone()}).unwrap()).collect()).unwrap();
        let StructuredInfoValueShape::Record(fields)=zero.shape() else {panic!("input")};
        let pulse=StructuredInfoValue::record(input_type.clone(),fields.iter().map(|f|StructuredFieldValue::new(f.name(),match f.name(){"samples"=>constant_i16(f.value().value_type(),1000),"epoch"=>StructuredInfoValue::leaf(f.value().value_type().clone(),1u64.to_le_bytes().to_vec()).unwrap(),_=>f.value().clone()}).unwrap()).collect()).unwrap();
        let inputs=BTreeMap::from([("value".into(),vec![zero.canonical_bytes().unwrap(),pulse.canonical_bytes().unwrap()])]);
        let result=run_epoch_stream_plan(plan,&context,&resources,inputs,None,2,ExecutionMode::Normal).expect("native proposal must carry all provisional memories");
        assert_eq!(result.values.len(),2);
        let leaf=|value:&StructuredInfoValue|{let StructuredInfoValueShape::Leaf(bytes)=value.shape() else {panic!("F32")};f32::from_le_bytes(bytes.try_into().unwrap())};
        assert_eq!(leaf(field(&result.values[0],"previous_raw")),0.);
        assert_eq!(leaf(field(&result.values[0],"previous_normalized")),0.);
        assert_eq!(leaf(field(&result.values[1],"previous_raw")),1000.);
        assert_eq!(leaf(field(&result.values[1],"previous_normalized")),1000./32768.);
        let StructuredInfoValueShape::Collection(history)=field(&result.values[1],"history").shape() else {panic!("history640")};
        assert_eq!(history.len(),640);
        assert!(history[..480].iter().all(|v|leaf(v)==0.));
        assert!((leaf(&history[480])-500./32768.).abs()<1e-8);
        assert!((leaf(&history[481])-(1000.-0.85*500.)/32768.).abs()<1e-8);
        assert!(history[482..].iter().all(|v|(leaf(v)-(1000.-0.85*1000.)/32768.).abs()<1e-8));
        eprintln!("Source native PCM→feature/provisional642: {}nodes/{}cords, prep{:?}/exec{:?}; no feedback ACK or trained waveform claim",result.nodes,result.cords,result.preparation,result.execution);
    }).unwrap().join().unwrap();
}

#[test]
fn utterance_tail_runs_two_source_epochs_in_order_and_drains() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| {
            let (context, _, ids) = super::feature_cycle::prepare_feedback();
            let (context, source, raw, offers) = super::feature_cycle::prepare_tail(context, &ids);
            let input = super::declarations::fixture_value(&raw)
                .canonical_bytes()
                .unwrap();
            let (plan, context) = super::prepare_authored_epoch_entry(
                context,
                source,
                "speech/fargan-utterance-tail",
                true,
                offers,
            )
            .unwrap();
            let inputs = BTreeMap::from([("value".into(), vec![input])]);
            let result = run_epoch_stream_plan(
                plan.clone(),
                &context,
                &Resources::new(),
                inputs.clone(),
                None,
                2,
                ExecutionMode::Normal,
            )
            .expect("two Source-admitted tails must drain");
            assert!(result.drained);
            assert_eq!(result.values.len(), 2);
            assert!(run_epoch_stream_plan(
                plan.clone(),
                &context,
                &Resources::new(),
                inputs.clone(),
                None,
                2,
                ExecutionMode::StoragePressure,
            )
            .is_none());
            assert!(run_epoch_stream_plan(
                plan,
                &context,
                &Resources::new(),
                inputs,
                None,
                2,
                ExecutionMode::CancelFirstExpression
            )
            .is_none());
            for (value, epoch) in result.values.iter().zip([63u64, 64]) {
                let StructuredInfoValueShape::Record(fields) = value.shape() else {
                    panic!("tail event")
                };
                let StructuredInfoValueShape::Leaf(bytes) = fields
                    .iter()
                    .find(|field| field.name() == "epoch")
                    .unwrap()
                    .value()
                    .shape()
                else {
                    panic!("epoch")
                };
                assert_eq!(bytes, &epoch.to_le_bytes());
            }
            eprintln!(
                "ordinary Source tail: {}nodes/{}cords, preparation{:?}, execution{:?}",
                result.nodes, result.cords, result.preparation, result.execution
            );
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn last_live_feature_closure_releases_exactly_two_ordered_continuations() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| {
            let (context, _, ids) = super::feature_cycle::prepare_feedback();
            let event = context
                .native
                .iter()
                .find(|profile| {
                    profile.kind_identity(true) == ids["__FEATURE_MODEL_EVENT_NATIVE__"]
                })
                .unwrap()
                .value_type()
                .clone();
            let fixture = super::declarations::fixture_value(&event);
            let StructuredInfoValueShape::Record(fields) = fixture.shape() else {
                panic!("event")
            };
            let inputs = [60u64, 61, 62]
                .into_iter()
                .map(|epoch| {
                    StructuredInfoValue::record(
                        event.clone(),
                        fields
                            .iter()
                            .map(|field| {
                                StructuredFieldValue::new(
                                    field.name(),
                                    if field.name() == "epoch" {
                                        StructuredInfoValue::leaf(
                                            field.value().value_type().clone(),
                                            epoch.to_le_bytes().to_vec(),
                                        )
                                        .unwrap()
                                    } else {
                                        field.value().clone()
                                    },
                                )
                                .unwrap()
                            })
                            .collect(),
                    )
                    .unwrap()
                    .canonical_bytes()
                    .unwrap()
                })
                .collect();
            let (context, source, _, offers) = super::feature_cycle::prepare_tail(context, &ids);
            let (plan, context) = super::prepare_authored_epoch_entry(
                context,
                source,
                "speech/flow-fargan-feature-utterance",
                true,
                offers,
            )
            .unwrap();
            let result = run_epoch_stream_plan(
                plan,
                &context,
                &Resources::new(),
                BTreeMap::from([("value".into(), inputs)]),
                None,
                5,
                ExecutionMode::Normal,
            )
            .expect("live closure and exact singleton must release two tails");
            assert!(result.drained);
            let epochs: Vec<_> = result
                .values
                .iter()
                .map(|value| {
                    let StructuredInfoValueShape::Record(fields) = value.shape() else {
                        panic!("event")
                    };
                    let StructuredInfoValueShape::Leaf(bytes) = fields
                        .iter()
                        .find(|field| field.name() == "epoch")
                        .unwrap()
                        .value()
                        .shape()
                    else {
                        panic!("epoch")
                    };
                    u64::from_le_bytes(bytes.try_into().unwrap())
                })
                .collect();
            assert_eq!(epochs, [60, 61, 62, 63, 64]);
            eprintln!(
                "ordinary live/EOF Source closure: {}nodes/{}cords, preparation{:?}, execution{:?}",
                result.nodes, result.cords, result.preparation, result.execution
            );
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn native_feature_causal_cell_drains_only_matching_final_pcm_acknowledgments() {
    std::thread::Builder::new().stack_size(32*1024*1024).spawn(|| {
        let (context,seeded,ids)=super::feature_cycle::prepare_feedback();
        let native=|key:&str|context.native.iter().find(|p|p.kind_identity(true)==ids[key]).unwrap().value_type().clone();
        let seed_type=native("__FEATURE_STATE_NATIVE__");
        let event_type=native("__FEATURE_EVENT_NATIVE__");
        let pcm_type=native("__FEATURE_PCM_NATIVE__");
        let receipt=format!("[{}]",vec!["1";32].join(","));
        let selected=format!("{{artifact_identity:{receipt},model_descriptor_identity:{receipt},session_basis_identity:{receipt},precision:reference_float32(\"\")}}");
        let source=super::feature_cycle::cycle_source(&ids).replace("selected: FarganModelFrameAnchor\n",&format!("selected: FarganModelFrameAnchor = {selected}\n"));
        let (plan,context)=super::prepare_authored_epoch_entry(context,source,"speech/flow-fargan-feature-cycle",true,seeded.offers().cloned().collect()).unwrap();
        let load=|bytes:&[u8]|bytes.as_chunks::<4>().0.iter().map(|v|f32::from_le_bytes(*v)).collect();
        let resources:Resources=[("band_weights","NumericF32MatrixRef161x18",vec![161,18],load(include_bytes!("../../../../proof/fargan/feature-profile/bands161x18.bin"))),("band_bias","NumericF32BiasRef18",vec![18],load(include_bytes!("../../../../proof/fargan/feature-profile/band_bias18.bin"))),("dct_weights","NumericF32MatrixRef18x18",vec![18,18],load(include_bytes!("../../../../proof/fargan/feature-profile/dct18x18.bin")))].into_iter().map(|(name,ty,dims,values)|(name.into(),Resource::new(conduit_ai::fixed_numeric_catalog::fixed_numeric_type(ty).unwrap(),&dims,values))).collect();
        let inputs=BTreeMap::from([
            ("seed".into(),vec![super::epoch_pair_fixture(&seed_type,"",0,0).canonical_bytes().unwrap()]),
            ("events".into(),[0,1].into_iter().map(|epoch|super::epoch_pair_fixture(&event_type,"",epoch,epoch).canonical_bytes().unwrap()).collect()),
            ("accepted".into(),[0,1].into_iter().map(|epoch|super::epoch_pair_fixture(&pcm_type,"",epoch,epoch).canonical_bytes().unwrap()).collect()),
        ]);
        let (_, missing_seeded, _)=super::feature_cycle::prepare_feedback();
        let result=run_epoch_stream_plan(plan.clone(),&context,&resources,inputs.clone(),Some(seeded),2,ExecutionMode::Normal).expect("Source feature feedback must receive each matching final PCM receipt and drain");
        assert!(result.drained);assert_eq!(result.values.len(),2);
        for (epoch,value) in [0u64,1].iter().zip(&result.values) {let raw=super::case_state::field(value,"epoch");let StructuredInfoValueShape::Leaf(bytes)=raw.shape() else {panic!("epoch")};assert_eq!(u64::from_le_bytes(bytes.try_into().unwrap()),*epoch);}
        let mut missing=inputs;missing.insert("accepted".into(),vec![]);
        assert!(run_epoch_stream_plan(plan,&context,&resources,missing,Some(missing_seeded),2,ExecutionMode::Normal).is_none(),"closed accepted input must never waive outstanding final PCM ACK debt");
        eprintln!("Source causal642 feedback: {}nodes/{}cords, prep{:?}/exec{:?}; synthetic admitted PCM acknowledgments, no trained synthesis claim",result.nodes,result.cords,result.preparation,result.execution);
    }).unwrap().join().unwrap();
}

#[test]
fn generic_source_when_filter_keeps_original_frames_and_drains_false_items() {
    std::thread::Builder::new().stack_size(32*1024*1024).spawn(|| {
        let source="type ProbePayload = collection F32 = 3\ntype ProbeFrame = {\n epoch: U64\n payload: ProbePayload\n}\nplot generic-filter (\n >> value: ProbeFrame...|\n result: ProbeFrame...| >>\n) {\n value >> when(.epoch < 2) >> result\n}\n".to_string();
        let (plan, context)=super::prepare_authored_epoch_entry(super::prepared_epoch_profiles_with_capacity(true),source,"generic-filter",true,vec![]).unwrap();
        let ty=plan.fragments[0].placements.iter().find(|gear|gear.implementation_id.as_str()==conduit_std_host::pure_filter::IMPLEMENTATION).unwrap();
        let factory=conduit_std_host::pure_filter::PureFilterOperationFactory::for_plan(&plan).unwrap();
        let mut drift=ty.clone();drift.host_calls[0].maximum_input_bytes-=1;
        assert!(factory.budget(&drift).is_err());
        let program=conduit_plot::PortableExpressionProgram::from_canonical_hex(match &ty.configuration[0].value {ConfigurationValue::Text(v)=>v,_=>panic!("program")}).unwrap();
        let inputs:Vec<_>=[0,1,2].iter().map(|epoch|super::epoch_pair_fixture(&program.input_type,"",*epoch,*epoch).canonical_bytes().unwrap()).collect();
        let mut host = factory.prepare_host(ty).unwrap();
        let (result, allocations) = super::allocation_probe::measure(|| host.execute(&inputs[0]));
        assert_eq!(result.unwrap(), Some(inputs[0].as_slice()));
        assert_eq!(allocations, 0, "prepared true filter must not allocate");
        let (result, allocations) = super::allocation_probe::measure(|| host.execute(&inputs[2]));
        assert_eq!(result.unwrap(), None);
        assert_eq!(allocations, 0, "prepared false filter must not allocate");
        assert!(host.execute(&inputs[0][..inputs[0].len()-1]).is_err());
        let foreign = super::case_state::vector(&conduit_ai::fixed_numeric_catalog::fixed_numeric_type("NumericF32Vector3").unwrap(), &[0.;3]).canonical_bytes().unwrap();
        assert!(host.execute(&foreign).is_err());
        assert_eq!(host.execute(&inputs[1]).unwrap(), Some(inputs[1].as_slice()));
        let mut unsealed=plan.clone();unsealed.fragments[0].placements[0].artifact_id="foreign".into();
        assert!(conduit_std_host::pure_filter::PureFilterOperationFactory::for_plan(&unsealed).is_err());
        let result=run_epoch_stream_plan(plan.clone(),&context,&BTreeMap::<String,Resource>::new(),BTreeMap::from([("value".into(),inputs.clone())]),None,2,ExecutionMode::Normal).unwrap();
        assert!(result.drained);assert_eq!(result.values.len(),2);
        for (actual,expected) in result.values.iter().zip(&inputs) {assert_eq!(actual.canonical_bytes().unwrap(),*expected);}
        assert!(run_epoch_stream_plan(plan,&context,&BTreeMap::<String,Resource>::new(),BTreeMap::from([("value".into(),inputs)]),None,2,ExecutionMode::CancelFirstExpression).is_none());
    }).unwrap().join().unwrap();
}

#[test]
fn reused_filter_back_waits_under_output_pressure_and_refuses_cancel_publication() {
    use conduit_kernel::{FailureCode, HostCallDisposition, HostCallOutcome, RequestId};
    let input = ValueRef {
        slot: 0,
        generation: 1,
        byte_len: 8,
    };
    let output = ValueRef {
        slot: 1,
        generation: 1,
        byte_len: 8,
    };
    for disposition in [
        HostCallDisposition::Completed,
        HostCallDisposition::Cancelled,
    ] {
        let mut back = conduit_semantic_catalog::StructuredSelectorBack::new(8);
        let mut request = StepIo::test_frame([Some(input)], [false], [Some(8)], None, 32);
        let bytes = 0u64.to_le_bytes();
        let view = StepInputBytes::test_frame([Some(bytes.as_slice())], None);
        let (outcome, allocations) =
            super::allocation_probe::measure(|| back.step(&mut request, &view));
        assert_eq!(outcome, StepOutcome::Progress);
        assert_eq!(allocations, 0, "filter request Step must not allocate");
        assert!(request.test_consumed(KPort(0)));
        let completion = Some((
            RequestId(0),
            HostCallOutcome {
                disposition,
                output: (disposition == HostCallDisposition::Completed)
                    .then_some(BoundedValueRef::new(output, 8).unwrap()),
                failure: None,
            },
        ));
        let mut blocked = StepIo::test_frame([None], [true], [None], completion, 32);
        let no_input = StepInputBytes::test_frame([None], None);
        if disposition == HostCallDisposition::Completed {
            assert_eq!(back.step(&mut blocked, &no_input), StepOutcome::Await);
            assert!(!blocked.test_host_completion_consumed());
            let mut ready = StepIo::test_frame([None], [true], [Some(8)], completion, 32);
            assert_eq!(back.step(&mut ready, &no_input), StepOutcome::Progress);
            assert!(ready.test_host_completion_consumed());
            let mut close = StepIo::test_frame([None], [true], [Some(8)], None, 32);
            assert_eq!(back.step(&mut close, &no_input), StepOutcome::Complete);
        } else {
            let StepOutcome::Fail(failure) = back.step(&mut blocked, &no_input) else {
                panic!("cancel must refuse publication")
            };
            assert_eq!(failure.code, FailureCode::Cancelled);
            assert!(!blocked.test_host_completion_consumed());
        }
    }
}
