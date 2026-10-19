#[path = "compact_store.rs"]
mod counted;
use super::*;
use conduit_ai::fixed_numeric_operations_back::*;
use conduit_composite::KernelOperationFactory;
use conduit_kernel::scheduler::CompactF32Layout;
use counted::Counted;
use std::cell::RefCell;

enum Back {
    Source(ValueRef, bool),
    Numeric(Box<dyn StepBack<PORTS>>),
    Sink(Rc<RefCell<Option<Vec<u8>>>>),
}
impl StepBack<PORTS> for Back {
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        match self {
            Self::Source(value, sent) => {
                if *sent {
                    return StepOutcome::Complete;
                }
                if !io.output_ready(KPort(0)) {
                    return StepOutcome::Await;
                }
                io.send(KPort(0), *value).unwrap();
                *sent = true;
                StepOutcome::Progress
            }
            Self::Numeric(back) => back.step(io, inputs),
            Self::Sink(received) => {
                let Some(bytes) = inputs.input(KPort(0)) else {
                    return StepOutcome::Await;
                };
                *received.borrow_mut() = Some(bytes.to_vec());
                io.consume(KPort(0)).unwrap();
                StepOutcome::Complete
            }
        }
    }
    fn prepared_output(&self, port: KPort) -> Option<&[u8]> {
        match self {
            Self::Numeric(back) => back.prepared_output(port),
            _ => None,
        }
    }
    fn compact_f32_input(&self, port: KPort) -> Option<CompactF32Layout> {
        match self {
            Self::Numeric(back) => back.compact_f32_input(port),
            _ => None,
        }
    }
    fn compact_f32_output(&self) -> Option<(KPort, CompactF32Layout)> {
        match self {
            Self::Numeric(back) => back.compact_f32_output(),
            _ => None,
        }
    }
    fn step_committed(&mut self) {
        if let Self::Numeric(back) = self {
            back.step_committed();
        }
    }
    fn cancel(&mut self) {
        if let Self::Numeric(back) = self {
            back.cancel();
        }
    }
}
#[derive(Debug, PartialEq)]
struct ResultRow {
    output: Option<Vec<u8>>,
    events: Vec<KernelEvent>,
    physical: usize,
    canonical_values: u64,
    compact_values: u64,
    compact_bytes: u64,
}
fn run<const W: usize>(mode: u8, overflow: bool) -> ResultRow {
    const N: usize = 8;
    const C: usize = 8;
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profiles = conduit_plot::ProfileCatalog::new();
    install_fixed_numeric_catalogs(&mut startup, &mut profiles).unwrap();
    let ty = fixed_numeric_type(&format!("NumericF32Vector{W}")).unwrap();
    let endpoints = [
        fixture("compact/source-a", &ty, true),
        fixture("compact/source-b", &ty, true),
        fixture("compact/sink", &ty, false),
    ];
    for endpoint in &endpoints {
        startup
            .insert(conduit_plot::KindSignature {
                kind: endpoint.kind_id.as_str().into(),
                startup_parameters: vec![],
            })
            .unwrap();
        profiles
            .insert_kind(Kind {
                kind_id: endpoint.kind_id.clone(),
                kind_contract_revision: endpoint.kind_contract_revision.clone(),
                startup_parameters: vec![],
                shorthand: None,
                configuration: vec![],
                inputs: endpoint.inputs.clone(),
                outputs: endpoint.outputs.clone(),
                semantic_laws: vec![KindSemanticLaw::ValueContracts(
                    endpoint.semantic_contract.value_contracts().to_vec(),
                )],
                limits: endpoint.limits.clone(),
            })
            .unwrap();
    }
    let operations = [
        FixedElementwiseOperation::Add,
        FixedElementwiseOperation::Sigmoid,
        FixedElementwiseOperation::Complement,
        FixedElementwiseOperation::Multiply,
    ];
    let mut offers = endpoints.to_vec();
    for op in operations {
        offers.push(fixed_elementwise_offer::<W>(op).unwrap());
    }
    offers.push(fixed_tanh_offer::<W>().unwrap());
    let source = format!(
        "plot compact-proof {{
        a: compact/source-a
        b: compact/source-b
        add: numeric/add{W}
        sigmoid: numeric/sigmoid{W}
        complement: numeric/complement{W}
        multiply: numeric/multiply{W}
        tanh: numeric/tanh{W}
        sink: compact/sink
        a.value >> add.left
        b.value >> add.right
        add.result >> sigmoid.value
        sigmoid.result >> complement.value
        complement.result >> multiply.left
        b.value >> multiply.right
        multiply.result >> tanh.value
        tanh.result >> sink.value
    }}"
    );
    let plot = conduit_plot::parse_with_startup(&source, &startup, &profiles).unwrap();
    let hosts = [HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("compact-host"),
        boot_id: BootId::from("compact-boot"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("compact@1"),
        bases: vec![],
        resources: vec![],
        planner_capabilities: vec![],
        capabilities: offers,
    }];
    let placements = conduit_planner::default_placements(&plot, &hosts).unwrap();
    let plan = conduit_planner::plan_with_connection_limits(
        &plot,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        1,
        16384,
    )
    .unwrap();
    let factories =
        conduit_ai::operation_owners::fixed_numeric::FixedNumericOperationFactory::for_plan(
            &plan,
            &std::collections::BTreeMap::new(),
        )
        .unwrap();
    let fragment = &plan.fragments[0];
    let (lowered, _) = conduit_plan_lowering::lowering::lower_plan_fragment_from_plan(
        &plan,
        &fragment.fragment_id,
    )
    .unwrap();
    let mut inner = HostedValueStore::new(32, 16384, 32 * 16384).unwrap();
    inner.prepare_compact_f32();
    let mut store = Counted {
        inner,
        canonical: 0,
        compact: 0,
        physical: 0,
    };
    let mut codec = FixedF32VectorCodec::<W>::prepare(&ty).unwrap();
    let a = store
        .store(
            codec
                .encode(&[if overflow { f32::MAX } else { -0.75 }; W])
                .unwrap(),
        )
        .unwrap();
    let b = store
        .store(
            codec
                .encode(&[if overflow { f32::MAX } else { 0.25 }; W])
                .unwrap(),
        )
        .unwrap();
    let output = Rc::new(RefCell::new(None));
    let mut nodes = Vec::new();
    let mut drivers = Vec::new();
    for (index, gear) in fragment.placements.iter().enumerate() {
        let kind = gear.kind_id.as_str();
        let fuel = lowered.node_specs[index].maximum_step_fuel;
        let driver = match kind {
            "compact/source-a" => Back::Source(a, false),
            "compact/source-b" => Back::Source(b, false),
            "compact/sink" => Back::Sink(output.clone()),
            _ => {
                let owner: Box<dyn StepBack<PORTS>> = if mode == 2 {
                    let factory = factories
                        .iter()
                        .find(|factory| factory.implementation_id() == &gear.implementation_id)
                        .unwrap();
                    let mut forged = gear.clone();
                    forged.implementation_id = ImplementationId::from("unselected/compact@1");
                    assert!(factory.prepare_compact_reusable(&forged).is_err());
                    factory.prepare_compact_reusable(gear).unwrap()
                } else if kind.starts_with("numeric/tanh") {
                    Box::new(FixedTanhBack::<W>::prepare_planned::<PORTS>(gear, fuel).unwrap())
                } else {
                    let operation = operations
                        .into_iter()
                        .find(|op| {
                            fixed_elementwise_offer::<W>(*op).unwrap().kind_id == gear.kind_id
                        })
                        .unwrap();
                    Box::new(
                        FixedElementwiseBack::<W>::prepare_planned::<PORTS>(gear, fuel, operation)
                            .unwrap(),
                    )
                };
                nodes.push((kind.to_string(), index));
                Back::Numeric(owner)
            }
        };
        drivers.push(driver);
    }
    let order: Vec<_> = ["add", "sigmoid", "complement", "multiply", "tanh"]
        .into_iter()
        .map(|name| {
            nodes
                .iter()
                .find(|(kind, _)| kind == &format!("numeric/{name}{W}"))
                .unwrap()
                .1
        })
        .collect();
    let mut routes = FixedRoutes::<N, C>::new(1);
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
    let mut scheduler = FixedScheduler::<_, _, _, N, C, PORTS, C, N, C>::new(
        lowered.node_specs.try_into().unwrap(),
        lowered
            .cords
            .into_iter()
            .map(|c| c.spec)
            .collect::<Vec<_>>()
            .try_into()
            .unwrap(),
        routes,
        drivers.try_into().unwrap_or_else(|_| panic!("drivers")),
        store,
        HostedSignLog::new(1024, 1024 * core::mem::size_of::<KernelEvent>() as u32).unwrap(),
    )
    .unwrap();
    if mode == 1 {
        scheduler
            .bind_execution_dag(
                &order
                    .iter()
                    .map(|n| conduit_kernel::NodeId(*n as u16))
                    .collect::<Vec<_>>(),
            )
            .unwrap();
    }
    if mode == 2 {
        scheduler
            .bind_compact_f32_dag(
                &order
                    .iter()
                    .map(|n| conduit_kernel::NodeId(*n as u16))
                    .collect::<Vec<_>>(),
            )
            .unwrap();
    }
    let mut physical = 0;
    for _ in 0..128 {
        physical += 1;
        match scheduler.step() {
            Err(_) => {
                assert!(overflow);
                break;
            }
            Ok(conduit_kernel::scheduler::SchedulerStatus::Drained) => break,
            _ => {}
        }
    }
    if overflow {
        assert!(output.borrow().is_none());
    } else {
        assert!(output.borrow().is_some());
    }
    let row = ResultRow {
        output: output.borrow().clone(),
        events: scheduler.signs().events().collect(),
        physical,
        canonical_values: scheduler.values().canonical,
        compact_values: scheduler.values().compact,
        compact_bytes: scheduler.values().physical,
    };
    row
}
// The unfused scheduler can attempt a multi-input Back before its second
// input arrives. Those real fuel grants remain in its own evidence. A selected
// DAG has a different physical schedule, so compare complete per-node logical
// transactions separately; compact and canonical versions of that same DAG
// must retain the entire identical global event sequence, including fuel.
fn logical_trace(
    row: &ResultRow,
    node: u16,
    failure: bool,
) -> Vec<(Option<KPort>, conduit_kernel::KernelEventKind)> {
    use conduit_kernel::KernelEventKind::*;
    row.events
        .iter()
        .filter(|event| event.node == conduit_kernel::NodeId(node))
        .filter(|event| event.kind != StepFuelGranted)
        .filter(|event| !failure || event.kind != BackCompleted)
        .map(|event| (event.port, event.kind))
        .collect()
}
fn compare<const W: usize>() {
    for overflow in [false, true] {
        let canonical = run::<W>(0, overflow);
        let dispatch = run::<W>(1, overflow);
        let compact = run::<W>(2, overflow);
        assert_eq!(canonical.output, dispatch.output);
        assert_eq!(dispatch.output, compact.output);
        assert_eq!(dispatch.events, compact.events);
        assert_eq!(dispatch.physical, compact.physical);
        for node in 0..8 {
            assert_eq!(
                logical_trace(&canonical, node, overflow),
                logical_trace(&compact, node, overflow)
            );
        }
        if overflow {
            assert_eq!(compact.canonical_values, 2);
            assert_eq!(compact.compact_values, 0);
            assert_eq!(
                compact
                    .events
                    .iter()
                    .filter(|e| e.kind == conduit_kernel::KernelEventKind::BackFailed)
                    .count(),
                1
            );
        } else {
            assert!(compact.physical < canonical.physical);
            assert_eq!(canonical.canonical_values, 7);
            assert_eq!(dispatch.canonical_values, 7);
            assert_eq!(dispatch.compact_values, 0);
            assert_eq!(compact.canonical_values, 3);
            assert_eq!(compact.compact_values, 4);
            assert_eq!(compact.compact_bytes, (W * 16) as u64);
        }
    }
}
#[test]
fn compact_real_pointwise_owners_match_canonical_at_40_lanes() {
    compare::<40>();
}
#[test]
fn compact_real_fargan_width_128_matches_canonical() {
    compare::<128>();
}
#[test]
fn compact_real_fargan_width_160_matches_canonical() {
    compare::<160>();
}
