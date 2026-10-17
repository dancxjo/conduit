//! Test-only finite numerical Plan/Play driver; no linguistic decisions live here.
#![allow(dead_code)]
use conduit_ai::{integer_categorical_step::*, *};
use conduit_core::*;
use conduit_kernel::{
    FixedRoutes, HostedSignLog, HostedValueStore, KernelEvent, PortId as KPort, ValueRef,
    scheduler::{FixedScheduler, StepBack, StepInputBytes, StepIo, StepOutcome},
};
use conduit_language::parser_model::*;
use std::{cell::RefCell, rc::Rc, sync::Arc};
const PORTS: usize = conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
pub const MAX_INVOCATIONS: u64 = 4096;
fn adopt(
    artifact: ModelArtifact,
    signature: ModelSignature,
    bytes: &[u8],
) -> Arc<AdmittedModelResource> {
    let reference = &artifact.content;
    let binding = ResourceReferenceBinding {
        identity: reference.identity,
        version: reference.lifetime.version,
        content_profile: reference.content_profile.clone(),
        access_class: reference.access_class.clone(),
        handle: "test/parser-model-handle".into(),
        authority_contract: MODEL_READ_AUTHORITY.into(),
        authority_grant: "test/parser-model-read".into(),
        maximum_bytes: reference.extent.bytes,
        maximum_items: Some(1),
        availability: ResourceReferenceAvailability::Available,
    };
    Arc::new(AdmittedModelResource::adopt(artifact, signature, Arc::from(bytes), &binding).unwrap())
}
fn residence() -> ResourceContentOffer {
    let (artifact, _) = english_parser_scorer_descriptor();
    let reference = artifact.content;
    ResourceContentOffer {
        contract: ResourceContentRequirement {
            identity: reference.identity,
            version: reference.lifetime.version,
            content_profile: reference.content_profile,
            maximum_bytes: ENGLISH_PARSER_MODEL_BYTES as u32,
            maximum_items: 1,
            retention: ResourceRetention::Play,
            sharing: ResourceSharing::ImmutableReadMany,
            access: ResourceAccessMode::ReadPublished,
            generation_slots: 1,
            reader_leases: 16,
            publication_slots: 0,
            sensitive: false,
        },
        owner_host: "parser-model-test".into(),
        owner_boot: "parser-model-test-boot".into(),
        base_id: "parser-model-test-base".into(),
        residence_profile: "test/owned-immutable-memory@1".into(),
    }
}
fn fixture_kind(name: &str, ty: &StructuredInfoType, source: bool, flow: bool) -> Kind {
    let port = PortDescriptor {
        port_id: port_id("value"),
        value_kind: ty.profile().unwrap().value_kind().clone(),
        direction: if source {
            PortDirection::Output
        } else {
            PortDirection::Input
        },
        temporal: if flow {
            PortTemporal::Flow { closes: true }
        } else {
            PortTemporal::Value
        },
        abnormal_kind: None,
    };
    let bound = conduit_plot::maximum_prepared_canonical_value_bytes(ty).unwrap();
    Kind {
        kind_id: name.into(),
        kind_contract_revision: "test/fixture@1".into(),
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
            contract: CheckedValueContract::new(port.value_kind, bound, vec![]).unwrap(),
        }])],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: 16384,
        },
    }
}
fn plan(
    profile: &PreparedCategoricalStep,
    flow: bool,
    resource_present: bool,
) -> Result<Plan, String> {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profiles = conduit_plot::ProfileCatalog::new();
    profile.install(&mut startup, &mut profiles, flow)?;
    let mut offers = vec![profile.offer(flow)?];
    for (name, ty, source) in [
        ("categorical-test/source", profile.indices_type(), true),
        ("categorical-test/sink", profile.scores_type(), false),
    ] {
        let kind = fixture_kind(name, ty, source, flow);
        startup.insert(conduit_plot::KindSignature {
            kind: name.into(),
            startup_parameters: vec![],
        })?;
        profiles
            .insert_kind(kind.clone())
            .map_err(|e| format!("{e:?}"))?;
        offers.push(
            BackOfferBuilder::new(
                kind,
                Back {
                    capability_id: name.into(),
                    execution_profile_id: "test/fixture@1".into(),
                    implementation_id: name.into(),
                    artifact_id: name.into(),
                    host_calls: vec![],
                    resource_requirements: vec![],
                    authority_requirements: vec![],
                },
            )
            .build(),
        );
    }
    let source = format!(
        "plot categorical-proof {{\n source: categorical-test/source\n model: {}\n sink: categorical-test/sink\n source.value >> model.indices\n model.scores >> sink.value\n}}\n",
        profile.kind_identity(flow)
    );
    let plot = conduit_plot::parse_with_startup(&source, &startup, &profiles)
        .map_err(|e| format!("{e:?}"))?;
    let hosts = [HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: "parser-model-test".into(),
        boot_id: "parser-model-test-boot".into(),
        offer_generation: OfferGeneration(1),
        profile: "test/host@1".into(),
        bases: vec![],
        resources: if resource_present {
            vec![profile.resource_offer().clone()]
        } else {
            vec![]
        },
        planner_capabilities: vec![],
        capabilities: offers,
    }];
    let placements =
        conduit_planner::default_placements(&plot, &hosts).map_err(|e| format!("{e:?}"))?;
    conduit_planner::plan_with_connection_limits(
        &plot,
        &hosts,
        &placements,
        &["conduit.base/local@1".into()],
        1,
        conduit_plot::maximum_prepared_canonical_value_bytes(profile.indices_type())
            .unwrap()
            .max(
                conduit_plot::maximum_prepared_canonical_value_bytes(profile.scores_type())
                    .unwrap(),
            ),
    )
    .map_err(|e| format!("{e:?}"))
}
fn gear(plan: &Plan) -> &PlannedGear {
    plan.fragments[0]
        .placements
        .iter()
        .find(|g| g.implementation_id.as_str() == CATEGORICAL_STEP_IMPLEMENTATION)
        .unwrap()
}

#[derive(Default)]
struct Shared {
    pending: Option<ValueRef>,
    output: Option<Vec<u8>>,
    receipts: Vec<CategoricalStepReceipt>,
}
enum Driver {
    Source(Rc<RefCell<Shared>>, bool),
    Model(Box<CategoricalStepBack>, Rc<RefCell<Shared>>, bool),
    Sink(Rc<RefCell<Shared>>, Option<Vec<u8>>),
}
impl StepBack<PORTS> for Driver {
    fn step(&mut self, io: &mut StepIo<PORTS>, bytes: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        match self {
            Self::Source(shared, staged) => {
                let Some(input) = shared.borrow().pending else {
                    return StepOutcome::Await;
                };
                if !io.output_ready(KPort(0)) {
                    return StepOutcome::Await;
                }
                io.send(KPort(0), input).unwrap();
                *staged = true;
                StepOutcome::Progress
            }
            Self::Model(back, _, staged) => {
                let outcome = back.step(io, bytes);
                *staged = matches!(outcome, StepOutcome::Progress);
                outcome
            }
            Self::Sink(_, staged) => {
                let Some(input) = bytes.input(KPort(0)) else {
                    return StepOutcome::Await;
                };
                *staged = Some(input.to_vec());
                io.consume(KPort(0)).unwrap();
                StepOutcome::Progress
            }
        }
    }
    fn prepared_output(&self, port: KPort) -> Option<&[u8]> {
        match self {
            Self::Model(back, _, _) => {
                <CategoricalStepBack as StepBack<PORTS>>::prepared_output(back.as_ref(), port)
            }
            _ => None,
        }
    }
    fn step_committed(&mut self) {
        match self {
            Self::Source(shared, staged) if *staged => {
                shared.borrow_mut().pending = None;
                *staged = false;
            }
            Self::Model(back, shared, staged) if *staged => {
                <CategoricalStepBack as StepBack<PORTS>>::step_committed(back.as_mut());
                shared
                    .borrow_mut()
                    .receipts
                    .push(back.last_receipt().unwrap());
                *staged = false;
            }
            Self::Sink(shared, staged) => {
                if let Some(output) = staged.take() {
                    shared.borrow_mut().output = Some(output);
                }
            }
            _ => {}
        }
    }
    fn cancel(&mut self) {
        if let Self::Model(back, _, staged) = self {
            *staged = false;
            <CategoricalStepBack as StepBack<PORTS>>::cancel(back.as_mut());
        }
    }
}
type Scheduler = FixedScheduler<Driver, HostedValueStore, HostedSignLog, 3, 2, PORTS, 2, 3, 2>;

/// One immutable adopted model and sealed Plan reused for bounded sequential calls.
/// Panics on failed admission, invalid input, pressure, or failure to settle.
pub struct ModelDriver {
    pub plan: Plan,
    profile: Arc<PreparedCategoricalStep>,
    shared: Rc<RefCell<Shared>>,
    scheduler: Scheduler,
}
impl ModelDriver {
    pub fn new() -> Self {
        let (artifact, signature) = english_parser_scorer_descriptor();
        let resource = adopt(artifact, signature, ENGLISH_PARSER_WEIGHTS);
        let profile = Arc::new(
            prepare_english_parser_scorer(
                Some(resource),
                "test/parser-model-pool".into(),
                residence(),
            )
            .unwrap(),
        );
        let plan = plan(&profile, true, true).unwrap();
        assert!(verify_plan(&plan));
        // Verify ordinary owner selection before preparing its concrete Step;
        // concrete custody lets the test retain the owner's exact commit receipt.
        owner::CategoricalOperationFactory::for_plan(&plan, std::slice::from_ref(&profile))
            .unwrap();
        let fragment = &plan.fragments[0];
        let (lowered, _) = conduit_plan_lowering::lowering::lower_plan_fragment_from_plan(
            &plan,
            &fragment.fragment_id,
        )
        .unwrap();
        let shared = Rc::new(RefCell::new(Shared::default()));
        let drivers: Vec<_> = fragment
            .placements
            .iter()
            .map(|gear| match gear.kind_id.as_str() {
                "categorical-test/source" => Driver::Source(shared.clone(), false),
                "categorical-test/sink" => Driver::Sink(shared.clone(), None),
                _ => Driver::Model(
                    Box::new(
                        CategoricalStepBack::prepare_planned::<PORTS>(
                            gear,
                            2,
                            profile.clone(),
                            true,
                        )
                        .unwrap(),
                    ),
                    shared.clone(),
                    false,
                ),
            })
            .collect();
        let mut routes = FixedRoutes::<3, 2>::new(1);
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
        let scheduler = FixedScheduler::new(
            lowered.node_specs.try_into().unwrap(),
            lowered
                .cords
                .into_iter()
                .map(|cord| cord.spec)
                .collect::<Vec<_>>()
                .try_into()
                .unwrap(),
            routes,
            drivers
                .try_into()
                .unwrap_or_else(|_| panic!("driver count")),
            HostedValueStore::new(16, 16384, 16 * 16384).unwrap(),
            HostedSignLog::new(60000, 60000 * core::mem::size_of::<KernelEvent>() as u32).unwrap(),
        )
        .unwrap();
        Self {
            plan,
            profile,
            shared,
            scheduler,
        }
    }
    pub fn score(&mut self, indices: [u64; 25]) -> [i64; 76] {
        assert!(
            self.invocations() < MAX_INVOCATIONS,
            "finite invocation budget"
        );
        let StructuredInfoTypeShape::Collection { element, .. } =
            self.profile.indices_type().shape()
        else {
            panic!("indices collection")
        };
        let bytes = StructuredInfoValue::collection(
            self.profile.indices_type().clone(),
            indices
                .iter()
                .map(|index| {
                    StructuredInfoValue::leaf(element.clone(), index.to_le_bytes().to_vec())
                        .unwrap()
                })
                .collect(),
        )
        .unwrap()
        .canonical_bytes()
        .unwrap();
        let input_identity = semantic_digest("numeric/categorical-indices@1", &bytes);
        let reference = self.scheduler.store_host_value(&bytes).unwrap();
        assert!(self.shared.borrow().pending.is_none());
        assert!(self.shared.borrow().output.is_none());
        self.shared.borrow_mut().pending = Some(reference);
        let expected = self.invocations() + 1;
        for _ in 0..32 {
            self.scheduler.step().unwrap();
            if self.shared.borrow().output.is_some() {
                break;
            }
        }
        let output = self
            .shared
            .borrow_mut()
            .output
            .take()
            .expect("model frame settled within finite scheduler budget");
        assert_eq!(self.invocations(), expected);
        let receipt = self.last_receipt().unwrap();
        assert_eq!(receipt.input, input_identity);
        assert_eq!(
            receipt.output,
            semantic_digest("numeric/categorical-scores@1", &output)
        );
        assert_eq!(
            receipt.model_content,
            self.profile.resource().artifact().content_identity()
        );
        assert_eq!(receipt.work_units, 1900);
        let value = StructuredInfoValue::from_canonical_bytes(&output).unwrap();
        let StructuredInfoValueShape::Collection(values) = value.shape() else {
            panic!("scores collection")
        };
        values
            .iter()
            .map(|value| {
                let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
                    panic!("score leaf")
                };
                i64::from_le_bytes(bytes.try_into().unwrap())
            })
            .collect::<Vec<_>>()
            .try_into()
            .unwrap()
    }
    pub fn invocations(&self) -> u64 {
        self.shared.borrow().receipts.len() as u64
    }
    pub fn last_receipt(&self) -> Option<CategoricalStepReceipt> {
        self.shared.borrow().receipts.last().copied()
    }
    pub fn receipts(&self) -> Vec<CategoricalStepReceipt> {
        self.shared.borrow().receipts.clone()
    }
}
