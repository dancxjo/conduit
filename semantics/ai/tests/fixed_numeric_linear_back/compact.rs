//! A checked activation -> matrix -> activation Plot with immutable admission intact.
use super::*;
use conduit_ai::{
    fixed_numeric_compact::CompactNumericBack,
    fixed_numeric_compact_linear::CompactLinearBack,
    fixed_numeric_operations_back::{fixed_tanh_offer, FixedTanhBack},
};
use conduit_kernel::{
    scheduler::{CompactF32Layout, SchedulerError, SchedulerStatus},
    Failure, NodeId, StorageError,
};
use std::cell::RefCell;
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

#[cfg(feature = "kernel-operation-owners")]
#[path = "factory.rs"]
mod factory;

struct Values {
    inner: HostedValueStore,
    canonical: u64,
    compact: u64,
    raw_bytes: u64,
}
impl ValueStorage for Values {
    fn item_capacity(&self) -> u16 {
        self.inner.item_capacity()
    }
    fn byte_capacity(&self) -> u32 {
        self.inner.byte_capacity()
    }
    fn used_items(&self) -> u16 {
        self.inner.used_items()
    }
    fn used_bytes(&self) -> u32 {
        self.inner.used_bytes()
    }
    fn store(&mut self, b: &[u8]) -> Result<ValueRef, StorageError> {
        let v = self.inner.store(b)?;
        self.canonical += 1;
        Ok(v)
    }
    fn get(&self, v: ValueRef) -> Result<&[u8], StorageError> {
        self.inner.get(v)
    }
    fn retain(&mut self, v: ValueRef) -> Result<(), StorageError> {
        self.inner.retain(v)
    }
    fn release(&mut self, v: ValueRef) -> Result<(), StorageError> {
        self.inner.release(v)
    }
    fn reference_count(&self, v: ValueRef) -> Result<u16, StorageError> {
        self.inner.reference_count(v)
    }
    fn clear(&mut self) {
        self.inner.clear()
    }
    fn compact_f32_available(&self, n: u32) -> (u16, u32) {
        self.inner.compact_f32_available(n)
    }
    fn compact_f32(&self, v: ValueRef) -> Result<Option<&[u8]>, StorageError> {
        self.inner.compact_f32(v)
    }
    fn store_compact_f32(&mut self, b: &[u8], n: u32) -> Result<ValueRef, StorageError> {
        let v = self.inner.store_compact_f32(b, n)?;
        self.compact += 1;
        self.raw_bytes += b.len() as u64;
        Ok(v)
    }
}
enum Back<'a> {
    Source(ValueRef, bool),
    Operation(Box<dyn StepBack<PORTS> + 'a>),
    Sink(Box<FixedF32VectorCodec<4>>, Rc<RefCell<Option<[u32; 4]>>>),
}
impl StepBack<PORTS> for Back<'_> {
    fn compact_f32_input(&self, p: KPort) -> Option<CompactF32Layout> {
        match self {
            Self::Operation(b) => b.compact_f32_input(p),
            _ => None,
        }
    }
    fn compact_f32_output(&self) -> Option<(KPort, CompactF32Layout)> {
        match self {
            Self::Operation(b) => b.compact_f32_output(),
            _ => None,
        }
    }
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        match self {
            Self::Source(v, sent) => {
                if *sent {
                    return StepOutcome::Complete;
                }
                if !io.output_ready(KPort(0)) {
                    return StepOutcome::Await;
                }
                io.send(KPort(0), *v).unwrap();
                *sent = true;
                StepOutcome::Complete
            }
            Self::Operation(b) => b.step(io, inputs),
            Self::Sink(codec, result) => {
                let Some(bytes) = inputs.input(KPort(0)) else {
                    return StepOutcome::Await;
                };
                let mut out = [0.; 4];
                codec.decode(bytes, &mut out).unwrap();
                *result.borrow_mut() = Some(out.map(f32::to_bits));
                io.consume(KPort(0)).unwrap();
                StepOutcome::Complete
            }
        }
    }
    fn step_committed(&mut self) {
        if let Self::Operation(b) = self {
            b.step_committed();
        }
    }
    fn prepared_output(&self, p: KPort) -> Option<&[u8]> {
        match self {
            Self::Operation(b) => b.prepared_output(p),
            _ => None,
        }
    }
    fn cancel(&mut self) {
        if let Self::Operation(b) = self {
            b.cancel();
        }
    }
}
fn plan() -> Plan {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    install_fixed_numeric_catalogs(&mut startup, &mut profile).unwrap();
    let mut offers = Vec::new();
    for (name, ty, source) in [
        ("numeric-test/input", "NumericF32Vector192", true),
        ("numeric-test/weights", "NumericF32MatrixRef192x4", true),
        ("numeric-test/sink", "NumericF32Vector4", false),
    ] {
        let offer = fixture(name, &fixed_numeric_type(ty).unwrap(), source);
        startup
            .insert(conduit_plot::KindSignature {
                kind: name.into(),
                startup_parameters: vec![],
            })
            .unwrap();
        profile
            .insert_kind(Kind {
                kind_id: offer.kind_id.clone(),
                kind_contract_revision: offer.kind_contract_revision.clone(),
                startup_parameters: vec![],
                shorthand: None,
                configuration: vec![],
                inputs: offer.inputs.clone(),
                outputs: offer.outputs.clone(),
                semantic_laws: vec![KindSemanticLaw::ValueContracts(
                    offer.semantic_contract.value_contracts().to_vec(),
                )],
                limits: offer.limits.clone(),
            })
            .unwrap();
        offers.push(offer);
    }
    offers.push(fixed_linear_offer::<192, 4>().unwrap());
    offers.push(fixed_tanh_offer::<4>().unwrap());
    offers.push(fixed_tanh_offer::<192>().unwrap());
    let source="plot compact-matrix {\n input: numeric-test/input\n weights: numeric-test/weights\n input_activation: numeric/tanh192\n linear: numeric/linear192x4\n activation: numeric/tanh4\n sink: numeric-test/sink\n input.value >> input_activation.value\n input_activation.result >> linear.value\n weights.value >> linear.weights\n linear.result >> activation.value\n activation.result >> sink.value\n}\n";
    let plot = conduit_plot::parse_with_startup(source, &startup, &profile).unwrap();
    let hosts = [HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: "numeric-test".into(),
        boot_id: "numeric-test-boot".into(),
        offer_generation: OfferGeneration(1),
        profile: "numeric-test@1".into(),
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
    assert!(verify_plan(&plan));
    plan
}
struct ResultWitness {
    output: Option<[u32; 4]>,
    events: Vec<KernelEvent>,
    failure: Option<SchedulerError>,
    canonical: u64,
    compact: u64,
    raw_bytes: u64,
}
#[derive(Clone, Copy, Debug)]
enum Fault {
    None,
    ResourceVersion,
    Overflow,
}
fn run(plan: &Plan, mode: u8, fault: Fault) -> ResultWitness {
    let packed = bytes(&if matches!(fault, Fault::Overflow) {
        vec![f32::MAX; 192 * 4]
    } else {
        (0..192)
            .flat_map(|_| [0.25, -0.5, 0.75, -1.])
            .collect::<Vec<_>>()
    });
    let tensor = resource_tensor(&[192, 4], &packed);
    let fragment = &plan.fragments[0];
    let (lowered, _) =
        conduit_plan_lowering::lowering::lower_plan_fragment_from_plan(plan, &fragment.fragment_id)
            .unwrap();
    let mut inner = HostedValueStore::new(16, 16384, 16 * 16384).unwrap();
    inner.prepare_compact_f32();
    let mut values = Values {
        inner,
        canonical: 0,
        compact: 0,
        raw_bytes: 0,
    };
    let input = values
        .store(
            FixedF32VectorCodec::<192>::prepare(
                &fixed_numeric_type("NumericF32Vector192").unwrap(),
            )
            .unwrap()
            .encode(
                &[if matches!(fault, Fault::Overflow) {
                    1.0
                } else {
                    0.005
                }; 192],
            )
            .unwrap(),
        )
        .unwrap();
    let mut incoming = tensor.clone();
    if matches!(fault, Fault::ResourceVersion) {
        let TensorBacking::Resource(r) = &mut incoming.backing else {
            unreachable!()
        };
        r.lifetime.version = ResourceVersionIdentity::from_digest([9; 32]);
    }
    let weights = values
        .store(
            FixedTensorPortBinding::prepare(
                &fixed_numeric_type("NumericF32MatrixRef192x4").unwrap(),
                &incoming,
            )
            .unwrap()
            .encoded(),
        )
        .unwrap();
    let output = Rc::new(RefCell::new(None));
    let mut linear_node = None;
    let mut tanh_node = None;
    let mut input_activation_node = None;
    let drivers: Vec<_> = fragment
        .placements
        .iter()
        .enumerate()
        .map(|(n, g)| match g.kind_id.as_str() {
            "numeric-test/input" => Back::Source(input, false),
            "numeric-test/weights" => Back::Source(weights, false),
            "numeric-test/sink" => Back::Sink(
                Box::new(
                    FixedF32VectorCodec::prepare(&fixed_numeric_type("NumericF32Vector4").unwrap())
                        .unwrap(),
                ),
                output.clone(),
            ),
            "numeric/linear192x4" => {
                linear_node = Some(NodeId(n as u16));
                #[cfg(feature = "kernel-operation-owners")]
                if mode == 4 {
                    return Back::Operation(factory::prepare(plan, &tensor, &packed, g));
                }

                let b = FixedLinearBack::prepare_planned::<PORTS>(
                    g,
                    lowered.node_specs[n].maximum_step_fuel,
                    &tensor,
                    &packed,
                    &access(&tensor),
                )
                .unwrap();
                if mode == 0 || mode == 3 {
                    Back::Operation(Box::new(b))
                } else {
                    let c = CompactLinearBack::from_linear(b).unwrap();
                    assert_eq!(c.admitted_access(), &access_receipt(&tensor));
                    assert_eq!(c.compact_scratch_bytes(), 16);
                    assert!(
                        <CompactLinearBack<192, 4> as StepBack<PORTS>>::compact_f32_input(
                            &c,
                            KPort(1)
                        )
                        .is_none()
                    );
                    Back::Operation(Box::new(c))
                }
            }
            "numeric/tanh192" => {
                input_activation_node = Some(NodeId(n as u16));
                let b = FixedTanhBack::<192>::prepare_planned::<PORTS>(
                    g,
                    lowered.node_specs[n].maximum_step_fuel,
                )
                .unwrap();
                if mode == 0 || mode == 3 {
                    Back::Operation(Box::new(b))
                } else {
                    Back::Operation(Box::new(CompactNumericBack::from_tanh(b).unwrap()))
                }
            }
            "numeric/tanh4" => {
                tanh_node = Some(NodeId(n as u16));
                let b = FixedTanhBack::<4>::prepare_planned::<PORTS>(
                    g,
                    lowered.node_specs[n].maximum_step_fuel,
                )
                .unwrap();
                if mode == 0 || mode == 3 {
                    Back::Operation(Box::new(b))
                } else {
                    Back::Operation(Box::new(CompactNumericBack::from_tanh(b).unwrap()))
                }
            }
            _ => panic!("unexpected owner"),
        })
        .collect();
    let mut routes = FixedRoutes::<6, 5>::new(1);
    for r in &lowered.routes {
        routes
            .install(r.source_node, r.source_port, r.range, &r.targets)
            .unwrap();
    }
    routes.seal().unwrap();
    let mut kernel = FixedScheduler::<_, _, _, 6, 5, PORTS, 5, 6, 5>::new(
        lowered.node_specs.try_into().unwrap(),
        lowered
            .cords
            .into_iter()
            .map(|c| c.spec)
            .collect::<Vec<_>>()
            .try_into()
            .unwrap(),
        routes,
        drivers
            .try_into()
            .unwrap_or_else(|_| panic!("driver count")),
        values,
        HostedSignLog::new(1024, 1024 * core::mem::size_of::<KernelEvent>() as u32).unwrap(),
    )
    .unwrap();
    if mode == 3 {
        kernel
            .bind_execution_dag(&[
                input_activation_node.unwrap(),
                linear_node.unwrap(),
                tanh_node.unwrap(),
            ])
            .unwrap();
    }
    if mode == 2 || mode == 4 {
        kernel
            .bind_compact_f32_dag_with_canonical_inputs(
                &[
                    input_activation_node.unwrap(),
                    linear_node.unwrap(),
                    tanh_node.unwrap(),
                ],
                &[(linear_node.unwrap(), KPort(1))],
            )
            .unwrap();
    }
    let mut failure = None;
    let mut terminal = false;
    let (_, alloc) = allocation_probe::observe(|| {
        for _ in 0..128 {
            match kernel.step() {
                Ok(SchedulerStatus::Drained) => {
                    terminal = true;
                    break;
                }
                Ok(_) => {}
                Err(e) => {
                    failure = Some(e);
                    kernel.cancel().unwrap();
                    terminal = true;
                    break;
                }
            }
        }
    });
    assert!(terminal);
    assert_eq!((alloc.allocations, alloc.reallocations), (0, 0));
    let result = ResultWitness {
        output: *output.borrow(),
        events: kernel.signs().events().collect(),
        failure,
        canonical: kernel.values().canonical,
        compact: kernel.values().compact,
        raw_bytes: kernel.values().raw_bytes,
    };
    assert_eq!(kernel.values().used_items(), 0);
    // Evidence reconstruction stays outside the observed Play allocation scope.
    eprintln!(
        "COMPACT_LINEAR_CONTROL {}",
        serde_json::json!({
            "schema":"conduit.compact-linear-control@2", "mode":mode, "fault":format!("{fault:?}"),
            "source_document_id":plan.source_document_id.as_str(), "plan_id":fragment.plan_id.as_str(),
            "roles":fragment.placements.iter().enumerate().map(|(n,g)| (g.kind_id.as_str(),n)).collect::<Vec<_>>(),
            "output_bits":result.output, "failure":format!("{:?}",result.failure),
            "canonical_stores":result.canonical, "compact_stores":result.compact, "raw_bytes":result.raw_bytes,
            "play_allocations":alloc.allocations, "play_reallocations":alloc.reallocations,
        "logical_invocations":kernel.logical_invocations(), "physical_decisions":kernel.decisions(),
            "events":result.events.iter().map(|e|serde_json::json!({"sequence":e.sequence,"node":e.node.0,"port":e.port.map(|p|p.0),"request":e.request.map(|r|r.0),"kind":format!("{:?}",e.kind)})).collect::<Vec<_>>()
        })
    );
    result
}
fn access_receipt(t: &TensorValue) -> AdmittedResourceAccess {
    conduit_ai::fixed_numeric_preparation::admit_tensor_access(t, &access(t)).unwrap()
}
#[test]
fn compact_linear_activation_keeps_exact_outputs_signs_and_canonical_resource_input() {
    let plan = plan();
    let reference = run(&plan, 0, Fault::None);
    assert!(reference.failure.is_none());
    assert!(reference.output.is_some());
    let grouped_reference = run(&plan, 3, Fault::None);
    assert_eq!(grouped_reference.output, reference.output);
    let modes: &[u8] = if cfg!(feature = "kernel-operation-owners") {
        &[1, 2, 4]
    } else {
        &[1, 2]
    };
    for &mode in modes {
        let candidate = run(&plan, mode, Fault::None);
        assert_eq!(candidate.output, reference.output);
        let same_schedule = if mode == 2 || mode == 4 {
            &grouped_reference
        } else {
            &reference
        };
        assert_eq!(candidate.events, same_schedule.events);
        for node in 0..6 {
            let logical = |events: &[KernelEvent]| {
                events
                    .iter()
                    .filter(|e| {
                        e.node == NodeId(node)
                            && e.kind != conduit_kernel::KernelEventKind::StepFuelGranted
                    })
                    .map(|e| (e.port, e.request, e.kind))
                    .collect::<Vec<_>>()
            };
            assert_eq!(logical(&candidate.events), logical(&reference.events));
        }
        assert!(candidate.failure.is_none());
        assert_eq!(
            (candidate.compact, candidate.raw_bytes),
            if mode == 2 || mode == 4 {
                (2, 784)
            } else {
                (0, 0)
            }
        );
        assert_eq!(
            reference.canonical - candidate.canonical,
            if mode == 2 || mode == 4 { 2 } else { 0 }
        );
    }
}
#[test]
fn resource_or_arithmetic_failure_preserves_the_frontier_without_matrix_publication() {
    let plan = plan();
    for fault in [Fault::ResourceVersion, Fault::Overflow] {
        let reference = run(&plan, 0, fault);
        assert!(matches!(
            reference.failure,
            Some(SchedulerError::BackFailed(Failure { detail: 1403, .. }))
        ));
        assert!(reference.output.is_none());
        let modes: &[u8] = if cfg!(feature = "kernel-operation-owners") {
            &[1, 2, 4]
        } else {
            &[1, 2]
        };
        for &mode in modes {
            let candidate = run(&plan, mode, fault);
            assert_eq!(candidate.failure, reference.failure);
            assert_eq!(candidate.events, reference.events);
            assert!(candidate.output.is_none());
            // The input activation precedes the failed matrix in both paths.
            assert_eq!(
                candidate.compact,
                if mode == 2 || mode == 4 { 1 } else { 0 }
            );
            assert_eq!(
                candidate.raw_bytes,
                if mode == 2 || mode == 4 { 768 } else { 0 }
            );
            assert_eq!(
                reference.canonical - candidate.canonical,
                if mode == 2 || mode == 4 { 1 } else { 0 }
            );
        }
    }
}
