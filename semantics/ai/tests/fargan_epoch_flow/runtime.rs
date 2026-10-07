use super::fixtures::*;
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
const PORTS: usize = conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
const N: usize = 1024;
const C: usize = 2048;
enum Driver {
    Source {
        reference: ValueRef,
        staged: bool,
        sent: bool,
    },
    Operation(Box<dyn StepBack<PORTS>>),
    Sink {
        received: Rc<std::cell::RefCell<Option<Vec<u8>>>>,
        staged: Option<Vec<u8>>,
    },
    Inactive,
}
impl StepBack<PORTS> for Driver {
    fn step(&mut self, io: &mut StepIo<PORTS>, bytes: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        match self {
            Self::Source {
                reference,
                staged,
                sent,
            } => {
                if *sent {
                    return StepOutcome::Complete;
                }
                if !io.output_ready(KPort(0)) {
                    return StepOutcome::Await;
                }
                io.send(KPort(0), *reference).unwrap();
                *staged = true;
                StepOutcome::Progress
            }
            Self::Operation(back) => back.step(io, bytes),
            Self::Sink { staged, .. } => {
                let Some(input) = bytes.input(KPort(0)) else {
                    return StepOutcome::Await;
                };
                *staged = Some(input.to_vec());
                io.consume(KPort(0)).unwrap();
                StepOutcome::Complete
            }
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
            Self::Sink {
                received, staged, ..
            } => {
                *received.borrow_mut() = staged.take();
            }
            Self::Source { staged, sent, .. } if *staged => {
                *sent = true;
                *staged = false;
            }
            _ => {}
        }
    }
    fn cancel(&mut self) {
        if let Self::Operation(back) = self {
            back.cancel();
        }
    }
}

#[derive(Clone, Copy)]
enum ExecutionMode {
    Normal,
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
    let slots = match mode {
        ExecutionMode::StoragePressure => resources.len() + 3,
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
        .map(|(name, encoded)| (name.clone(), store.store(encoded).unwrap()))
        .collect();
    for (name, resource) in resources {
        let binding =
            FixedTensorPortBinding::prepare(&resource.value_type, &resource.tensor).unwrap();
        fixtures.insert(name.clone(), store.store(binding.encoded()).unwrap());
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
    context.zip.validate_plan(&plan).unwrap();
    let received = Rc::new(std::cell::RefCell::new(None));
    let mut owners = BTreeMap::new();
    let mut drivers = Vec::new();
    for (index, gear) in fragment.placements.iter().enumerate() {
        if let Some(factory) = factories
            .iter()
            .find(|f| f.implementation_id() == &gear.implementation_id)
        {
            factory.budget(gear).unwrap();
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
            let owner = conduitos::expression_host_call::ExpressionHostCall::prepare(
                fragment,
                &lowered,
                &active,
                &gear.placement_id,
            )
            .unwrap();
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
                    reference: *reference,
                    staged: false,
                    sent: false,
                });
            } else {
                assert_eq!(name, "result");
                drivers.push(Driver::Sink {
                    received: received.clone(),
                    staged: None,
                });
            }
        }
    }
    drop(factories);
    drop(adopted); // drivers retain immutable admitted resource custody.
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
    let mut scheduler=FixedScheduler::<_,_,_,N,C,PORTS,C,C,C,1024,1024>::new_with_active_counts_and_host_calls(nodes,cords,specs.try_into().unwrap(),cord_specs.try_into().unwrap(),routes,bindings,drivers.try_into().unwrap_or_else(|_|panic!("capacity")),store,HostedSignLog::new(32768,32768*core::mem::size_of::<KernelEvent>()as u32).unwrap()).unwrap();
    let preparation = start.elapsed();
    let execute = Instant::now();
    for _ in 0..262144 {
        let status = match scheduler.step() {
            Ok(status) => status,
            Err(error) => {
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
            eprintln!("epoch scheduler settled: {status:?}");
            break;
        }
        if let Some(call) = scheduler.next_host_request() {
            if matches!(mode, ExecutionMode::CancelFirstExpression) {
                owners
                    .get_mut(&call.node)
                    .unwrap()
                    .cancel(call.node, call.call)
                    .unwrap();
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
            let output = owners
                .get_mut(&call.node)
                .unwrap()
                .invoke(call.node, call.call, call.request, input)
                .unwrap();
            let Ok(value) = scheduler.store_host_value(output) else {
                break;
            };
            scheduler
                .complete_host_call(
                    call.node,
                    call.request,
                    conduit_kernel::HostCallOutcome {
                        disposition: conduit_kernel::HostCallDisposition::Completed,
                        output: Some(BoundedValueRef::new(value, output.len() as u32).unwrap()),
                        failure: None,
                    },
                )
                .unwrap();
        }
        if received.borrow().is_some() {
            break;
        }
    }
    let execution = execute.elapsed();
    let encoded = received.borrow();
    if encoded.is_none() && matches!(mode, ExecutionMode::Normal) {
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
    let encoded = encoded.as_ref()?;
    let actual = StructuredInfoValue::from_canonical_bytes(encoded).unwrap();
    Some(EncodedResultAndTiming {
        value: actual,
        preparation,
        execution,
        nodes,
        cords,
    })
}
fn synthetic_resources(plan: &Plan) -> Resources {
    let types = conduit_ai::fixed_numeric_catalog::fixed_numeric_types().unwrap();
    plan.fragments[0]
        .placements
        .iter()
        .filter_map(|gear| {
            let name = gear.kind_id.as_str().strip_prefix("epoch-proof/")?;
            if name == "value" || name == "result" {
                return None;
            }
            let native = types
                .iter()
                .find(|ty| {
                    ty.value_type.profile().unwrap().value_kind() == &gear.outputs[0].value_kind
                })
                .unwrap();
            let dimensions: Vec<u64> =
                if let Some(shape) = native.name.strip_prefix("NumericF32MatrixRef") {
                    shape
                        .split('x')
                        .map(|dimension| dimension.parse().unwrap())
                        .collect()
                } else {
                    vec![native
                        .name
                        .strip_prefix("NumericF32BiasRef")
                        .unwrap()
                        .parse()
                        .unwrap()]
                };
            let mut values = vec![0.; dimensions.iter().product::<u64>() as usize];
            if name == "output_bias" {
                values.fill(0.25);
            }
            Some((
                name.into(),
                Resource::new(native.value_type.clone(), &dimensions, values),
            ))
        })
        .collect()
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
