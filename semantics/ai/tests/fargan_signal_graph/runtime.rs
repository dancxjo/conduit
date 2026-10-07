use super::{plan::*, shared::*, state::*};
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
const N: usize = 512;
const C: usize = 1024;
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

pub struct ResultAndTiming {
    pub pcm: [f32; 40],
    pub state: State,
    pub preparation: Duration,
    pub execution: Duration,
    pub nodes: usize,
    pub cords: usize,
}
#[derive(Clone, Copy)]
pub enum ExecutionMode {
    Normal,
    StoragePressure,
    CancelFirstExpression,
}
pub fn try_run_graph(
    schema: &SourceSchema,
    resources: &Resources,
    condition: [f32; 80],
    period: u16,
    state: &State,
    mode: ExecutionMode,
) -> Option<ResultAndTiming> {
    let planning = Instant::now();
    let plan = prepare_plan(schema);
    let planning = planning.elapsed();
    let mut fixtures = BTreeMap::new();
    fixtures.insert(
        "condition".to_owned(),
        vector(schema.ty("condition"), &condition)
            .canonical_bytes()
            .unwrap(),
    );
    fixtures.insert("state".to_owned(), state.encode(schema.ty("state")));
    let primitive = StructuredInfoValue::leaf(
        StructuredInfoType::leaf(kind_id("value/u16")).unwrap(),
        period.to_le_bytes().to_vec(),
    )
    .unwrap();
    let encoded = StructuredInfoValue::nominal(schema.ty("period").clone(), primitive)
        .unwrap()
        .canonical_bytes()
        .unwrap();
    fixtures.insert("period".to_owned(), encoded);
    let result = run_entry_plan(plan, resources, fixtures, mode)?;
    Some(ResultAndTiming {
        pcm: floats(field(&result.value, "pcm_f32")).try_into().unwrap(),
        state: State::from_result(&result.value),
        preparation: planning + result.preparation,
        execution: result.execution,
        nodes: result.nodes,
        cords: result.cords,
    })
}
pub struct EncodedResultAndTiming {
    pub value: StructuredInfoValue,
    pub preparation: Duration,
    pub execution: Duration,
    pub nodes: usize,
    pub cords: usize,
}
pub fn run_entry_plan(
    plan: Plan,
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
    .unwrap();
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
        if let Some(name) = gear.kind_id.as_str().strip_prefix("subframe-fixture/") {
            if let Some(resource) = resources.get(name) {
                adopted.insert(gear.placement_id.clone(), resource.adopt());
            }
        }
    }
    let factories =
        conduit_std_host::fixed_numeric::FixedNumericOperationFactory::for_plan(&plan, &adopted)
            .unwrap();
    let received = Rc::new(std::cell::RefCell::new(None));
    let mut owners = BTreeMap::new();
    let mut drivers = Vec::new();
    for (index, gear) in fragment.placements.iter().enumerate() {
        if gear.kind_id.as_str().starts_with("numeric/") {
            let factory = factories
                .iter()
                .find(|f| f.implementation_id() == &gear.implementation_id)
                .unwrap();
            factory.budget(gear).unwrap();
            drivers.push(Driver::Operation(
                factory.prepare(gear, &mut store).unwrap(),
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
            let name = gear
                .kind_id
                .as_str()
                .strip_prefix("subframe-fixture/")
                .unwrap();
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
    let mut bindings = conduit_kernel::FixedHostCallBindings::<256>::new(1);
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
    let mut scheduler=FixedScheduler::<_,_,_,N,C,PORTS,C,C,C,256,256>::new_with_active_counts_and_host_calls(nodes,cords,specs.try_into().unwrap(),cord_specs.try_into().unwrap(),routes,bindings,drivers.try_into().unwrap_or_else(|_|panic!("capacity")),store,HostedSignLog::new(32768,32768*core::mem::size_of::<KernelEvent>()as u32).unwrap()).unwrap();
    let preparation = start.elapsed();
    let execute = Instant::now();
    for _ in 0..8192 {
        if scheduler.step().is_err() {
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
                .unwrap()
                .to_vec();
            let Ok(value) = scheduler.store_host_value(&output) else {
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
pub fn run_graph(
    schema: &SourceSchema,
    resources: &Resources,
    condition: [f32; 80],
    period: u16,
    state: &State,
) -> ResultAndTiming {
    try_run_graph(
        schema,
        resources,
        condition,
        period,
        state,
        ExecutionMode::Normal,
    )
    .expect("one committed aggregate")
}
