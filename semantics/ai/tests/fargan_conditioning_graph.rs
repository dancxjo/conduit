#![cfg(feature = "kernel-step")]
use conduit_ai::{
    fixed_numeric_back::*, fixed_numeric_binding::*, fixed_numeric_catalog::*,
    fixed_numeric_codec::*, fixed_numeric_operations_back::*, fixed_numeric_preparation::*,
    fixed_numeric_window_back::*,
};
use conduit_core::*;
use conduit_data::*;
use conduit_kernel::{
    scheduler::{FixedScheduler, StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, FixedRoutes, HostedSignLog, HostedValueStore, KernelEvent, PortId as KPort,
    ValueRef, ValueStorage,
};
use conduit_plot::rust_binding::BoundedSequence;
use std::{collections::BTreeMap, rc::Rc};
const PORTS: usize = conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
fn tensor(shape: &[u64], bytes: &[u8]) -> TensorValue {
    let digest = tensor_content_digest(bytes);
    TensorValue {
        element: TensorElement::F32,
        dimensions: BoundedSequence::try_from_iter(shape.iter().copied()).unwrap(),
        axes: BoundedSequence::try_from_iter(shape.iter().map(|_| TensorAxis {
            role: TensorAxisRole::Feature,
            identity: None,
            unit: None,
        }))
        .unwrap(),
        content_digest: digest,
        backing: TensorBacking::Resource(BoundedResourceRef {
            identity: ResourceSemanticIdentity::from_digest(digest),
            content_profile: kind_id("tensor/elements-ieee754-f32-le@1"),
            access_class: ResourceClassId::from("test/read@1"),
            extent: ResourceExtent {
                bytes: bytes.len() as u64,
                items: Some(shape.iter().product()),
            },
            lifetime: ResourceLifetime {
                version: ResourceVersionIdentity::from_digest([1; 32]),
                expires_at: None,
            },
        }),
    }
}
fn access(tensor: &TensorValue) -> ResourceReferenceBinding {
    let TensorBacking::Resource(reference) = &tensor.backing else {
        panic!("resource fixture")
    };
    ResourceReferenceBinding {
        identity: reference.identity,
        version: reference.lifetime.version,
        content_profile: reference.content_profile.clone(),
        access_class: reference.access_class.clone(),
        handle: ResourceHandleId::from("numeric-test-immutable"),
        authority_contract: AuthorityContractId::from(TENSOR_READ_AUTHORITY),
        authority_grant: AuthorityGrantId::from("numeric-test-read"),
        maximum_bytes: reference.extent.bytes,
        maximum_items: reference.extent.items,
        availability: ResourceReferenceAvailability::Available,
    }
}
fn packed(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|x| x.to_le_bytes()).collect()
}
fn ty(name: &str) -> StructuredInfoType {
    fixed_numeric_type(name).unwrap()
}
fn fixture_kind(name: &str, value: &StructuredInfoType, source: bool) -> Kind {
    let port = PortDescriptor {
        port_id: port_id("value"),
        value_kind: if name == "op-test/index" {
            kind_id("value/u16")
        } else {
            value.profile().unwrap().value_kind().clone()
        },
        direction: if source {
            PortDirection::Output
        } else {
            PortDirection::Input
        },
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    };
    Kind {
        kind_id: kind_id(name),
        kind_contract_revision: KindIdentity::from(name),
        startup_parameters: vec![],
        shorthand: None,
        configuration: vec![],
        inputs: if source { vec![] } else { vec![port.clone()] },
        outputs: if source { vec![port.clone()] } else { vec![] },
        semantic_laws: vec![KindSemanticLaw::ValueContracts(vec![FrontValueContract {
            location: if source {
                FrontValueLocation::Output(port.port_id)
            } else {
                FrontValueLocation::Input(port.port_id)
            },
            contract: CheckedValueContract::new(
                port.value_kind,
                conduit_plot::maximum_prepared_transport_value_bytes(value).unwrap(),
                vec![],
            )
            .unwrap(),
        }])],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: 16_384,
        },
    }
}
fn offer(kind: Kind) -> CapabilityOffer {
    let name = kind.kind_id.as_str().to_string();
    BackOfferBuilder::new(
        kind,
        Back {
            capability_id: CapabilityId::from(name.clone()),
            execution_profile_id: ExecutionProfileId::from("numeric-test@1"),
            implementation_id: ImplementationId::from(name.clone()),
            artifact_id: ArtifactId::from(name),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build()
}

// This test runs the authored graph with generic numeric and ConduitOS pure
// expression owners. Synthetic tensors test topology, not pretrained parity.
const SOURCE: &str = include_str!("../../speech/fargan_conditioning.conduit");
const N: usize = 64;
const C: usize = 96;
struct Resource {
    ty: &'static str,
    tensor: TensorValue,
    bytes: Vec<u8>,
}
fn resource(ty: &'static str, dimensions: &[u64], values: Vec<f32>) -> Resource {
    let bytes = packed(&values);
    Resource {
        ty,
        tensor: tensor(dimensions, &bytes),
        bytes,
    }
}
fn expression_offer(program: &conduit_plot::PortableExpressionProgram) -> CapabilityOffer {
    conduitos::expression_host_call::offer(program, PortTemporal::Value).unwrap()
}
fn prepare_plan(resources: &BTreeMap<&str, Resource>) -> (Plan, StructuredInfoType) {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profiles = conduit_plot::ProfileCatalog::new();
    install_fixed_numeric_catalogs(&mut startup, &mut profiles).unwrap();
    let checked =
        conduit_plot::check_syntax_document(&conduit_plot::parse_syntax_document(SOURCE), &startup)
            .unwrap();
    let period = checked
        .native_types
        .iter()
        .find(|t| t.name == "FarganPeriod")
        .unwrap()
        .value_type
        .clone();
    let mut fixtures = vec![
        ("features", ty("NumericF32Vector20"), true),
        ("period", period.clone(), true),
        ("previous_period", period.clone(), true),
        ("history", ty("NumericHistory2x64"), true),
        ("condition", ty("NumericF32Vector320"), false),
        ("next_history", ty("NumericHistory2x64"), false),
        ("conditioned_period", period.clone(), false),
        ("next_period", period.clone(), false),
    ];
    for (name, resource) in resources {
        fixtures.push((name, ty(resource.ty), true));
    }
    let mut offers = vec![];
    let mut wrapper =
        String::from("\nplot conditioning-proof {\n inner: speech/fargan-conditioning\n");
    for (name, ty, input) in fixtures {
        let kind = fixture_kind(&format!("conditioning-fixture/{name}"), &ty, input);
        startup
            .insert(conduit_plot::KindSignature {
                kind: kind.kind_id.as_str().into(),
                startup_parameters: vec![],
            })
            .unwrap();
        profiles.insert_kind(kind.clone()).unwrap();
        offers.push(offer(kind));
        wrapper.push_str(&format!(" {name}: conditioning-fixture/{name}\n"));
        wrapper.push_str(&if input {
            format!(" {name}.value >> inner.{name}\n")
        } else {
            format!(" inner.{name} >> {name}.value\n")
        });
    }
    wrapper.push_str("}\n");
    let source = format!("{SOURCE}{wrapper}");
    let checked = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(&source),
        &startup,
    )
    .unwrap();
    let authoring = conduit_plot::expand_canonical_plot_for_authoring(
        &checked,
        "conditioning-proof",
        &profiles,
    )
    .unwrap();
    assert!(authoring.input_bindings.is_empty() && authoring.output_bindings.is_empty());
    let plot = &authoring.expanded;
    offers.extend([
        fixed_embedding_offer::<224, 12>().unwrap(),
        fixed_concatenate_offer::<20, 12>().unwrap(),
        fixed_affine_offer::<32, 64>().unwrap(),
        fixed_tanh_offer::<64>().unwrap(),
        fixed_window_offer().unwrap(),
        fixed_affine_offer::<192, 128>().unwrap(),
        fixed_tanh_offer::<128>().unwrap(),
        fixed_affine_offer::<128, 320>().unwrap(),
        fixed_tanh_offer::<320>().unwrap(),
    ]);
    for gear in &plot.gears {
        if let [entry] = gear.configuration.as_slice() {
            let ConfigurationValue::Text(encoded) = &entry.value else {
                panic!("encoded pure program")
            };
            assert_eq!(entry.key, "program");
            let p = conduit_plot::PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
            let selected = expression_offer(&p);
            if !offers
                .iter()
                .any(|o| o.capability_id == selected.capability_id)
            {
                offers.push(selected);
            }
        }
    }
    let hosts = [HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: "conditioning-proof".into(),
        boot_id: "conditioning-boot".into(),
        offer_generation: OfferGeneration(1),
        profile: "synthetic-conditioning@1".into(),
        bases: vec![],
        resources: vec![],
        planner_capabilities: vec![],
        capabilities: offers,
    }];
    let placements = conduit_planner::default_expanded_placements(plot, &hosts).unwrap();
    let plan = conduit_planner::plan_expanded_canonical(
        plot,
        &hosts,
        &placements,
        &["conduit.base/local@1".into()],
    )
    .unwrap();
    (plan, period)
}
enum Driver<'a> {
    Source {
        reference: ValueRef,
        staged: bool,
        sent: bool,
    },
    Operation(Box<dyn StepBack<PORTS> + 'a>),
    Sink {
        name: String,
        received: Rc<std::cell::RefCell<BTreeMap<String, Vec<u8>>>>,
    },
    Inactive,
}
impl StepBack<PORTS> for Driver<'_> {
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
            Self::Sink { name, received } => {
                let Some(input) = bytes.input(KPort(0)) else {
                    return StepOutcome::Await;
                };
                received.borrow_mut().insert(name.clone(), input.to_vec());
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
fn values(resource: &Resource) -> Vec<f64> {
    resource
        .bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| f64::from(f32::from_le_bytes(*b)))
        .collect()
}
fn affine(input: &[f64], weights: &Resource, bias: &Resource) -> Vec<f64> {
    let w = values(weights);
    let b = values(bias);
    let outputs = b.len();
    (0..outputs)
        .map(|o| {
            (b[o]
                + input
                    .iter()
                    .enumerate()
                    .map(|(i, x)| x * w[i * outputs + o])
                    .sum::<f64>())
            .tanh()
        })
        .collect()
}
#[test]
fn checked_conditioning_source_executes_complete_plan_with_atomic_history_and_delayed_pitch() {
    let mut resources = BTreeMap::new();
    resources.insert(
        "embedding_weights",
        resource(
            "NumericEmbedding224x12",
            &[224, 12],
            (0..224 * 12)
                .map(|i| ((i % 31) as f32 - 15.) * 0.01)
                .collect(),
        ),
    );
    for (name, bias_name, kind, bias_kind, input, output) in [
        (
            "dense1_weights",
            "dense1_bias",
            "NumericF32MatrixRef32x64",
            "NumericF32BiasRef64",
            32,
            64,
        ),
        (
            "conv_weights",
            "conv_bias",
            "NumericF32MatrixRef192x128",
            "NumericF32BiasRef128",
            192,
            128,
        ),
        (
            "dense2_weights",
            "dense2_bias",
            "NumericF32MatrixRef128x320",
            "NumericF32BiasRef320",
            128,
            320,
        ),
    ] {
        resources.insert(
            name,
            resource(
                kind,
                &[input, output],
                (0..input * output)
                    .map(|i| ((i % 17) as f32 - 8.) * 0.0003)
                    .collect(),
            ),
        );
        resources.insert(
            bias_name,
            resource(
                bias_kind,
                &[output],
                (0..output).map(|i| ((i % 7) as f32 - 3.) * 0.002).collect(),
            ),
        );
    }
    let features = core::array::from_fn::<_, 20, _>(|i| i as f32 * 0.01 - 0.1);
    let history = core::array::from_fn::<_, 128, _>(|i| i as f32 * 0.001 - 0.05);
    let (decoded, next) = run_graph(&resources, features, history, 73, 69);
    let mut joined: Vec<f64> = features.iter().map(|v| f64::from(*v)).collect();
    joined.extend(&values(&resources["embedding_weights"])[(73 - 32) * 12..(74 - 32) * 12]);
    let dense = affine(
        &joined,
        &resources["dense1_weights"],
        &resources["dense1_bias"],
    );
    let mut window: Vec<f64> = history.iter().map(|v| f64::from(*v)).collect();
    window.extend(&dense);
    let conv = affine(&window, &resources["conv_weights"], &resources["conv_bias"]);
    let expected = affine(
        &conv,
        &resources["dense2_weights"],
        &resources["dense2_bias"],
    );
    for (a, b) in decoded.iter().zip(&expected) {
        assert!((f64::from(*a) - b).abs() < 2e-6);
    }
    for i in 0..64 {
        assert_eq!(next[i], history[i + 64]);
        assert!((f64::from(next[i + 64]) - dense[i]).abs() < 2e-6);
    }
}
fn run_graph(
    resources: &BTreeMap<&str, Resource>,
    features: [f32; 20],
    history: [f32; 128],
    period_value: u16,
    previous_value: u16,
) -> ([f32; 320], [f32; 128]) {
    let (plan, period_type) = prepare_plan(resources);
    assert!(verify_plan(&plan));
    let fragment = &plan.fragments[0];
    let (lowered, _) = conduit_plan_lowering::lowering::lower_plan_fragment_from_plan(
        &plan,
        &fragment.fragment_id,
    )
    .unwrap();
    assert!(lowered.node_specs.len() <= N && lowered.cords.len() <= C);
    let active = bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    let mut store = HostedValueStore::new(256, 16384, 256 * 16384).unwrap();
    let mut fixtures = BTreeMap::new();
    let mut feature_codec = FixedF32VectorCodec::<20>::prepare(&ty("NumericF32Vector20")).unwrap();
    fixtures.insert(
        "features",
        store
            .store(feature_codec.encode(&features).unwrap())
            .unwrap(),
    );
    let mut history_codec = FixedF32VectorCodec::<128>::prepare_history().unwrap();
    fixtures.insert(
        "history",
        store
            .store(history_codec.encode(&history).unwrap())
            .unwrap(),
    );
    for (name, value) in [
        ("period", period_value),
        ("previous_period", previous_value),
    ] {
        let primitive = StructuredInfoValue::leaf(
            StructuredInfoType::leaf(kind_id("value/u16")).unwrap(),
            value.to_le_bytes().to_vec(),
        )
        .unwrap();
        let encoded = StructuredInfoValue::nominal(period_type.clone(), primitive)
            .unwrap()
            .canonical_bytes()
            .unwrap();
        fixtures.insert(name, store.store(&encoded).unwrap());
    }
    for (name, r) in resources {
        let binding = FixedTensorPortBinding::prepare(&ty(r.ty), &r.tensor).unwrap();
        fixtures.insert(name, store.store(binding.encoded()).unwrap());
    }
    let received = Rc::new(std::cell::RefCell::new(BTreeMap::new()));
    let mut owners = BTreeMap::new();
    let mut drivers = Vec::new();
    for (index, gear) in fragment.placements.iter().enumerate() {
        let fuel = lowered.node_specs[index].maximum_step_fuel;
        let op: Box<dyn StepBack<PORTS> + '_> = match gear.kind_id.as_str() {
            "numeric/embedding224x12" => {
                let r = &resources["embedding_weights"];
                Box::new(
                    FixedEmbeddingBack::<'_, 224, 12>::prepare_planned::<PORTS>(
                        gear,
                        fuel,
                        &r.tensor,
                        &r.bytes,
                        &access(&r.tensor),
                    )
                    .unwrap(),
                )
            }
            "numeric/concatenate20x12" => Box::new(
                FixedConcatenateBack::<20, 12, 32>::prepare_planned::<PORTS>(gear, fuel).unwrap(),
            ),
            "numeric/tanh64" => {
                Box::new(FixedTanhBack::<64>::prepare_planned::<PORTS>(gear, fuel).unwrap())
            }
            "numeric/tanh128" => {
                Box::new(FixedTanhBack::<128>::prepare_planned::<PORTS>(gear, fuel).unwrap())
            }
            "numeric/tanh320" => {
                Box::new(FixedTanhBack::<320>::prepare_planned::<PORTS>(gear, fuel).unwrap())
            }
            "numeric/history2x64" => {
                Box::new(FixedWindowBack::prepare_planned::<PORTS>(gear, fuel).unwrap())
            }
            kind if kind.starts_with("numeric/dense") => {
                macro_rules! dense {
                    ($i:literal,$o:literal,$w:literal,$b:literal) => {{
                        let w = &resources[$w];
                        let b = &resources[$b];
                        Box::new(
                            FixedAffineBack::<$i, $o>::prepare_planned::<PORTS>(
                                gear,
                                fuel,
                                &w.tensor,
                                &w.bytes,
                                &access(&w.tensor),
                                &b.tensor,
                                &b.bytes,
                                &access(&b.tensor),
                            )
                            .unwrap(),
                        ) as Box<dyn StepBack<PORTS>>
                    }};
                }
                match kind {
                    "numeric/dense32x64" => dense!(32, 64, "dense1_weights", "dense1_bias"),
                    "numeric/dense192x128" => dense!(192, 128, "conv_weights", "conv_bias"),
                    "numeric/dense128x320" => dense!(128, 320, "dense2_weights", "dense2_bias"),
                    _ => panic!("unknown dense"),
                }
            }
            _ if gear.implementation_id.as_str()
                == conduitos::expression_host_call::IMPLEMENTATION =>
            {
                let owner = conduitos::expression_host_call::ExpressionHostCall::prepare(
                    fragment,
                    &lowered,
                    &active,
                    &gear.placement_id,
                )
                .unwrap();
                owners.insert(conduit_kernel::NodeId(index as u16), owner);
                Box::new(conduit_kernel::scheduler::HostCallBack::new(
                    gear.host_calls[0].maximum_input_bytes,
                ))
            }
            name => {
                let short = name.strip_prefix("conditioning-fixture/").unwrap();
                if let Some(reference) = fixtures.get(short) {
                    drivers.push(Driver::Source {
                        reference: *reference,
                        staged: false,
                        sent: false,
                    });
                } else {
                    drivers.push(Driver::Sink {
                        name: short.into(),
                        received: received.clone(),
                    });
                }
                continue;
            }
        };
        drivers.push(Driver::Operation(op));
    }
    let nodes = drivers.len();
    while drivers.len() < N {
        drivers.push(Driver::Inactive);
    }
    let cords = lowered.cords.len();
    let mut routes = FixedRoutes::<C, C>::new(1);
    for r in &lowered.routes {
        routes
            .install(r.source_node, r.source_port, r.range, &r.targets)
            .unwrap();
    }
    routes.seal().unwrap();
    let mut bindings = conduit_kernel::FixedHostCallBindings::<32>::new(1);
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
    let mut scheduler=FixedScheduler::<_,_,_,N,C,PORTS,C,C,C,32,32>::new_with_active_counts_and_host_calls(nodes,cords,specs.try_into().unwrap(),cord_specs.try_into().unwrap(),routes,bindings,drivers.try_into().unwrap_or_else(|_|panic!("capacity")),store,HostedSignLog::new(8192,8192*core::mem::size_of::<KernelEvent>()as u32).unwrap()).unwrap();
    for _ in 0..2048 {
        scheduler.step().unwrap();
        if let Some(call) = scheduler.next_host_request() {
            let input = scheduler.values().get(call.input.value).unwrap();
            let output = owners
                .get_mut(&call.node)
                .unwrap()
                .invoke(call.node, call.call, call.request, input)
                .unwrap()
                .to_vec();
            let value = scheduler.store_host_value(&output).unwrap();
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
        if received.borrow().len() == 4 {
            break;
        }
    }
    let actual = received.borrow();
    assert_eq!(actual.len(), 4);
    let mut decoded = [0.; 320];
    FixedF32VectorCodec::<320>::prepare(&ty("NumericF32Vector320"))
        .unwrap()
        .decode(&actual["condition"], &mut decoded)
        .unwrap();
    let mut next = [0.; 128];
    history_codec
        .decode(&actual["next_history"], &mut next)
        .unwrap();
    for (name, value) in [
        ("conditioned_period", previous_value),
        ("next_period", period_value),
    ] {
        let p = StructuredInfoValue::leaf(
            StructuredInfoType::leaf(kind_id("value/u16")).unwrap(),
            value.to_le_bytes().to_vec(),
        )
        .unwrap();
        assert_eq!(
            actual[name],
            StructuredInfoValue::nominal(period_type.clone(), p)
                .unwrap()
                .canonical_bytes()
                .unwrap()
        );
    }
    (decoded, next)
}
/// Optional pinned-weight proof: ordinary CI never downloads or redistributes
/// pretrained data. Explicit invocation needs the local development oracle.
#[test]
#[ignore = "requires explicitly supplied pinned local model and scalar oracle trace"]
fn pinned_scalar_model_conditioning_differential() {
    let root = std::path::PathBuf::from(
        std::env::var_os("CONDUIT_FARGAN_DEVELOPMENT_FIXTURE")
            .expect("explicit local fixture path"),
    );
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("resources/manifest.json")).unwrap())
            .unwrap();
    assert_eq!(
        manifest["upstream_sha"],
        "503d81b138d76621aae4b12786e90de48aa8db3a"
    );
    let blob = std::fs::read(root.join("resources/f32.bin")).unwrap();
    assert_eq!(blob.len(), 3_272_868);
    use sha2::{Digest, Sha256};
    assert_eq!(
        format!("{:x}", Sha256::digest(&blob)),
        "d35f0510da476716183a33caafe45cc095e48d496f06de2db7655e780a385e47"
    );
    let mut resources = BTreeMap::new();
    for (name, array, kind, shape) in [
        (
            "embedding_weights",
            "cond_net_pembed_weights_float",
            "NumericEmbedding224x12",
            vec![224, 12],
        ),
        (
            "dense1_weights",
            "cond_net_fdense1_weights_float",
            "NumericF32MatrixRef32x64",
            vec![32, 64],
        ),
        (
            "dense1_bias",
            "cond_net_fdense1_bias",
            "NumericF32BiasRef64",
            vec![64],
        ),
        (
            "conv_weights",
            "cond_net_fconv1_weights_float",
            "NumericF32MatrixRef192x128",
            vec![192, 128],
        ),
        (
            "conv_bias",
            "cond_net_fconv1_bias",
            "NumericF32BiasRef128",
            vec![128],
        ),
        (
            "dense2_weights",
            "cond_net_fdense2_weights_float",
            "NumericF32MatrixRef128x320",
            vec![128, 320],
        ),
        (
            "dense2_bias",
            "cond_net_fdense2_bias",
            "NumericF32BiasRef320",
            vec![320],
        ),
    ] {
        let a = manifest["arrays"]
            .as_array()
            .unwrap()
            .iter()
            .find(|a| a["name"] == array)
            .unwrap();
        let offset = a["profiles"]["f32"]["offset"].as_u64().unwrap() as usize;
        let length = a["profiles"]["f32"]["bytes"].as_u64().unwrap() as usize;
        let bytes = &blob[offset..offset + length];
        assert_eq!(
            format!("{:x}", Sha256::digest(bytes)),
            a["sha256"].as_str().unwrap()
        );
        resources.insert(
            name,
            resource(
                kind,
                &shape,
                bytes
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|b| f32::from_le_bytes(*b))
                    .collect(),
            ),
        );
    }
    let inputs = std::fs::read(root.join("conditions.f32le")).unwrap();
    let oracle = std::fs::read(root.join("conditioning.trace")).unwrap();
    assert_eq!(inputs.len(), 24 * 20 * 4);
    assert_eq!(oracle.len(), 24 * (320 + 128 + 2) * 4);
    assert_eq!(
        format!("{:x}", Sha256::digest(&oracle)),
        "ca634e3e31af975329ed3db41d38cf1d8c05a164ed0d8b16bcebcd270e5faacd"
    );
    let mut history = [0.; 128];
    let mut previous = 69;
    let mut condition_error = 0f32;
    let mut history_error = 0f32;
    for (frame, input) in inputs.as_chunks::<80>().0.iter().enumerate() {
        let features = core::array::from_fn(|i| {
            f32::from_le_bytes(input[i * 4..i * 4 + 4].try_into().unwrap())
        });
        let record = &oracle[frame * 1800..(frame + 1) * 1800];
        let period =
            u16::try_from(i32::from_le_bytes(record[1796..1800].try_into().unwrap())).unwrap();
        assert_eq!(
            i32::from_le_bytes(record[1792..1796].try_into().unwrap()),
            i32::from(previous)
        );
        let (condition, next) = run_graph(&resources, features, history, period, previous);
        for (i, value) in condition.iter().enumerate() {
            let reference = f32::from_le_bytes(record[i * 4..i * 4 + 4].try_into().unwrap());
            condition_error = condition_error.max((value - reference).abs());
        }
        for (i, value) in next.iter().enumerate() {
            let start = 1280 + i * 4;
            let reference = f32::from_le_bytes(record[start..start + 4].try_into().unwrap());
            history_error = history_error.max((value - reference).abs());
        }
        history = next;
        previous = period;
    }
    eprintln!("24-frame source Plan conditioning max_abs={condition_error}, history max_abs={history_error}; scalar libm reference differs from upstream approximation");
    assert!(condition_error < 0.002);
    assert!(history_error < 0.001);
}
