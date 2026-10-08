//! Ordinary retained Source numeric projection and exact resource model Plan/Play.
use actual_expression_owner::expression_host_call::{
    ExpressionHostCall, ExpressionOperationFactory,
};
use conduit_ai::integer_categorical_step::{
    owner::CategoricalOperationFactory, PreparedCategoricalStep, CATEGORICAL_STEP_IMPLEMENTATION,
};
use conduit_composite::*;
use conduit_core::*;
use conduit_plot::*;
use std::{collections::BTreeMap, rc::Rc, sync::Arc};
enum Pure {
    Expression(ExpressionHostCall),
}
pub struct Execution {
    kernel: KernelCompositeHost,
    pure: Vec<(conduit_kernel::NodeId, Pure)>,
    remaining_inferences: Option<u16>,
    pub source_document: Rc<String>,
    pub checked_source: Rc<CheckedSyntaxDocument>,
    pub model_catalog: Rc<ProfileCatalog>,
    pub expanded_source: ExpandedAuthoringPlot,
    pub original_plan: std::rc::Rc<Plan>,
    input_payload: ValuePayload,
    output_payload: ValuePayload,
    input_port: PortId,
    output_port: PortId,
    pub entry: String,
    pub input_type_bytes: Vec<u8>,
    pub output_type_bytes: Vec<u8>,
}
pub fn prepare_source_with_storage(
    profile: Arc<PreparedCategoricalStep>,
    document: String,
    entry: &str,
    maximum_inferences: Option<u16>,
) -> Execution {
    let parsed = parse_syntax_document(&document);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let mut startup = StartupCatalog::new();
    let mut catalogs = ProfileCatalog::new();
    profile.install(&mut startup, &mut catalogs, true).unwrap();
    startup
        .insert_structured_type(
            "LanguageParserCategoricalIndices",
            profile.indices_type().clone(),
        )
        .unwrap();
    startup
        .insert_structured_type(
            "LanguageParserCategoricalScores",
            profile.scores_type().clone(),
        )
        .unwrap();
    eprintln!("checking full Source for {entry}");
    let checked = check_syntax_document(&parsed, &startup).unwrap();
    prepare_checked_source_with_catalog(
        profile,
        document,
        checked,
        Rc::new(catalogs),
        entry,
        maximum_inferences,
    )
}

pub fn prepare_checked_source(
    profile: Arc<PreparedCategoricalStep>,
    document: impl Into<Rc<String>>,
    checked: impl Into<Rc<CheckedSyntaxDocument>>,
    entry: &str,
    maximum_inferences: Option<u16>,
) -> Execution {
    let mut startup = StartupCatalog::new();
    let mut catalogs = ProfileCatalog::new();
    profile.install(&mut startup, &mut catalogs, true).unwrap();
    prepare_checked_source_with_catalog(
        profile,
        document,
        checked,
        Rc::new(catalogs),
        entry,
        maximum_inferences,
    )
}
/// Reuses the complete original model-installed catalog. No reinstallation,
/// Type-only import or Source/law filtering occurs between fixed port owners.
pub fn prepare_checked_source_with_catalog(
    profile: Arc<PreparedCategoricalStep>,
    document: impl Into<Rc<String>>,
    checked: impl Into<Rc<CheckedSyntaxDocument>>,
    catalogs: Rc<ProfileCatalog>,
    entry: &str,
    maximum_inferences: Option<u16>,
) -> Execution {
    let document = document.into();
    let checked = checked.into();
    eprintln!("expanding checked Source for {entry}");
    let expanded = expand_canonical_plot_for_authoring(&checked, entry, &catalogs).unwrap();
    eprintln!("planning expanded Source for {entry}");
    assert_eq!(expanded.front.inputs().len(), 1);
    assert_eq!(expanded.front.outputs().len(), 1);
    let mut capabilities = vec![profile.offer(true).unwrap()];
    for gear in &expanded.expanded.gears {
        if gear.configuration.is_empty() {
            continue;
        }
        let [entry] = gear.configuration.as_slice() else {
            panic!("one retained program")
        };
        let ConfigurationValue::Text(encoded) = &entry.value else {
            panic!("retained bytes")
        };
        let temporal = gear.checked_front().inputs()[0].temporal;
        capabilities.push(match entry.key.as_str() {
            "program" => actual_expression_owner::expression_host_call::offer(
                &PortableExpressionProgram::from_canonical_hex(encoded).unwrap(),
                temporal,
            )
            .unwrap(),
            _ => panic!("generic pure Source owner"),
        });
    }
    let host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: "test/model-host".into(),
        boot_id: "test/model-boot".into(),
        offer_generation: OfferGeneration(1),
        profile: "test/learned-model".into(),
        bases: vec![],
        resources: vec![profile.resource_offer().clone()],
        capabilities,
        planner_capabilities: vec![],
    };
    let hosts = [host];
    let choices = conduit_planner::default_expanded_placements(&expanded.expanded, &hosts).unwrap();
    let selected_offer = |gear: &GearId| {
        let choice = &choices.by_gear[gear];
        hosts[0]
            .capabilities
            .iter()
            .find(|offer| offer.capability_id == choice.capability_id)
            .unwrap()
    };
    let connections = expanded
        .expanded
        .connections
        .iter()
        .map(|cord| {
            (
                (
                    cord.source_gear_id.clone(),
                    cord.source_port_id.clone(),
                    cord.sink_gear_id.clone(),
                    cord.sink_port_id.clone(),
                ),
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: selected_offer(&cord.source_gear_id)
                        .limits
                        .max_queue_bytes
                        .min(selected_offer(&cord.sink_gear_id).limits.max_queue_bytes),
                },
            )
        })
        .collect();
    let boundaries = expanded
        .input_bindings
        .iter()
        .map(|b| (PortDirection::Input, b))
        .chain(
            expanded
                .output_bindings
                .iter()
                .map(|b| (PortDirection::Output, b)),
        )
        .map(|(direction, b)| {
            (
                conduit_planner::ForeBoundaryKey {
                    direction,
                    front_port_id: b.front_port_id.clone(),
                    track: b.track,
                },
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: selected_offer(&b.gear_id)
                        .limits
                        .max_queue_bytes
                        .min(MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32),
                },
            )
        })
        .collect();
    let plan = conduit_planner::plan_expanded_authoring_with_connection_limits(
        &expanded,
        &hosts,
        &choices,
        &["conduit.base/local@1".into()],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &connections,
        &boundaries,
    )
    .unwrap();
    assert!(verify_plan(&plan));
    assert_eq!(plan.fragments.len(), 1);
    assert!(
        plan.fragments[0]
            .placements
            .iter()
            .filter(|p| p.implementation_id.as_str() == CATEGORICAL_STEP_IMPLEMENTATION)
            .count()
            <= 1
    );
    let factory = CategoricalOperationFactory::for_plan(&plan, &[profile]).unwrap();
    let original_plan = std::rc::Rc::new(plan.clone());
    let definition = definition_from_original_plan(plan);
    let fragment = &definition.internal_plan.fragments[0];
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(fragment).unwrap();
    let active = bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    let pure = fragment
        .placements
        .iter()
        .filter_map(|gear| {
            let owner = match gear.implementation_id.as_str() {
                actual_expression_owner::expression_host_call::IMPLEMENTATION => Pure::Expression(
                    ExpressionHostCall::prepare(fragment, &lowered, &active, &gear.placement_id)
                        .unwrap(),
                ),
                CATEGORICAL_STEP_IMPLEMENTATION => return None,
                other => panic!("foreign owner {other}"),
            };
            Some((
                lowered
                    .identity
                    .node_for_placement(&gear.placement_id)
                    .unwrap(),
                owner,
            ))
        })
        .collect();
    let mut registry = KernelOperationRegistry::new();
    registry
        .install(ExpressionOperationFactory::default())
        .unwrap();
    registry.install(factory).unwrap();
    let kernel = KernelCompositeHost::prepare_with_sign_storage(
        definition,
        &registry,
        KernelCompositeSignStorage {
            // Remote lifecycle records occupy main journal slots as well.
            additional_local_items: maximum_inferences.map_or(1024, |count| {
                count
                    .checked_mul(4)
                    .and_then(|items| items.checked_add(1024))
                    .expect("combined retained journal bound overflows")
            }),
            additional_remote_items: maximum_inferences
                .map_or(256, |count| count.checked_mul(4).unwrap()),
        },
    )
    .unwrap();
    drop(registry);
    let input_binding = &expanded.input_bindings[0];
    let output_binding = &expanded.output_bindings[0];
    let program_for = |gear: &GearId| {
        let gear = expanded
            .expanded
            .gears
            .iter()
            .find(|g| &g.gear_id == gear)
            .unwrap();
        let ConfigurationValue::Text(hex) = &gear.configuration[0].value else {
            panic!("pure endpoint")
        };
        PortableExpressionProgram::from_canonical_hex(hex).unwrap()
    };
    let input_type_bytes = program_for(&input_binding.gear_id)
        .input_type
        .canonical_bytes()
        .unwrap();
    let output_type_bytes = program_for(&output_binding.gear_id)
        .output_type
        .canonical_bytes()
        .unwrap();
    let input_port = &kernel.definition().boundary.input_fronts[0].external_port;
    let output_port = &kernel.definition().boundary.output_fronts[0].external_port;
    let input_payload = ValuePayload {
        value_kind: input_port.value_kind.clone(),
        encoded: Vec::with_capacity(MAXIMUM_STRUCTURED_CANONICAL_BYTES),
    };
    let output_payload = ValuePayload {
        value_kind: output_port.value_kind.clone(),
        encoded: Vec::with_capacity(MAXIMUM_STRUCTURED_CANONICAL_BYTES),
    };
    let input_port = input_port.port_id.clone();
    let output_port = output_port.port_id.clone();
    let mut execution = Execution {
        kernel,
        pure,
        remaining_inferences: maximum_inferences,
        source_document: document,
        checked_source: checked,
        model_catalog: catalogs,
        expanded_source: expanded,
        original_plan,
        input_payload,
        output_payload,
        input_port,
        output_port,
        entry: entry.into(),
        input_type_bytes,
        output_type_bytes,
    };
    execution.kernel.start().unwrap();
    execution
}
impl Execution {
    fn step(&mut self) {
        self.kernel.step().unwrap();
        if let Some(request) = self.kernel.next_host_request() {
            let node = self
                .kernel
                .host_request_view(&request)
                .unwrap()
                .request
                .node;
            let obligation = self.kernel.host_request_obligation(&request).unwrap();
            let admitted = self
                .kernel
                .admit_host_request(
                    &request,
                    &obligation.host,
                    &obligation.resources,
                    &obligation.authorities,
                )
                .unwrap();
            let call = *self
                .kernel
                .admitted_host_request_view(&admitted)
                .unwrap()
                .request;
            let input = self.kernel.host_request_input(&admitted).unwrap();
            let (_, owner) = self.pure.iter_mut().find(|(n, _)| *n == node).unwrap();
            let bytes = match owner {
                Pure::Expression(p) => {
                    Some(p.invoke(node, call.call, call.request, input).unwrap())
                }
            };
            if let Some(bytes) = bytes {
                self.kernel
                    .complete_host_call_bytes(&admitted, bytes)
                    .unwrap();
            } else {
                panic!("projection cannot refuse valid input");
            }
        }
    }
}
impl Execution {
    #[allow(dead_code)] // Used by the finite-journal diagnostic target.
    pub fn signs_snapshot(&self) -> BTreeMap<HostId, Vec<conduit_kernel::KernelEvent>> {
        self.kernel.signs()
    }
    pub fn infer(&mut self, sequence: u64, features: &StructuredInfoValue) -> StructuredInfoValue {
        if let Some(remaining) = &mut self.remaining_inferences {
            assert!(
                *remaining > 0,
                "declared inference batch exhausted before ingress"
            );
            *remaining -= 1;
        }
        let input = self.kernel.definition().boundary.input_fronts[0]
            .external_port
            .clone();
        let output = self.kernel.definition().boundary.output_fronts[0]
            .external_port
            .clone();
        let payload = ValuePayload {
            value_kind: input.value_kind,
            encoded: features.canonical_bytes().unwrap(),
        };
        assert!(matches!(
            self.kernel
                .admit_input(&input.port_id, sequence, &payload)
                .unwrap(),
            conduit_kernel::scheduler::RemoteIngressOutcome::Accepted { .. }
        ));
        let mut result = ValuePayload {
            value_kind: output.value_kind,
            encoded: Vec::with_capacity(MAXIMUM_STRUCTURED_CANONICAL_BYTES),
        };
        for _ in 0..100 {
            self.step();
            if let Some(received) = self
                .kernel
                .output_into(&output.port_id, &mut result)
                .unwrap()
            {
                assert_eq!(received, sequence);
                let scores = StructuredInfoValue::from_canonical_bytes(&result.encoded).unwrap();
                self.kernel
                    .complete_output(&output.port_id, received)
                    .unwrap();
                return scores;
            }
        }
        panic!("bounded authored projection/model output");
    }
}

// Packaging from the original sealed Fore; this never changes or reseals Plan
// identities. KernelCompositeHost validates the complete original definition.
fn definition_from_original_plan(plan: Plan) -> KernelCompositeDefinition {
    assert!(verify_plan(&plan));
    let f = &plan.fragments[0];
    let mut boundary = KernelCompositeBoundary {
        input_fronts: vec![],
        output_fronts: vec![],
    };
    let mut contracts = vec![];
    let mut maximum = 1;
    for fore in &f.fore_ports {
        assert_eq!(fore.track, ConnectionTrack::Payload);
        let placement = f
            .placements
            .iter()
            .find(|p| p.placement_id == fore.placement_id)
            .unwrap();
        let ports = if fore.direction == PortDirection::Input {
            &placement.inputs
        } else {
            &placement.outputs
        };
        let mut port = ports
            .iter()
            .find(|p| p.port_id == fore.gear_port_id)
            .unwrap()
            .clone();
        assert_eq!(port.value_kind, fore.value_kind);
        port.port_id = fore.front_port_id.clone();
        if let Some(contract) = &fore.value_contract {
            maximum = maximum.max(contract.maximum_bytes);
            contracts.push(FrontValueContract {
                location: if fore.direction == PortDirection::Input {
                    FrontValueLocation::Input(port.port_id.clone())
                } else {
                    FrontValueLocation::Output(port.port_id.clone())
                },
                contract: contract.clone(),
            });
        }
        let binding = KernelCompositeFrontBinding {
            external_port: port,
            internal_child: f.host_id.clone(),
            internal_placement_id: fore.placement_id.clone(),
            internal_port_id: fore.gear_port_id.clone(),
            terminal: CompositeFrontTerminal::Independent,
        };
        if fore.direction == PortDirection::Input {
            boundary.input_fronts.push(binding);
        } else {
            boundary.output_fronts.push(binding);
        }
    }
    let external = capability_offer_from_parts! {
        semantic_contract: KindSemanticContract { configuration: vec![], laws: vec![KindSemanticLaw::ValueContracts(contracts)] },
        startup_parameters: vec![], shorthand: None,
        capability_id: "test/production-custody".into(), kind_id: "test/production-custody".into(),
        kind_contract_revision: "test/production-custody@1".into(),
        implementation: ImplementationOffer { execution_profile_id: "test/production-custody@1".into(),
            implementation_id: "test/production-custody@1".into(), artifact_id: "test/production-custody@1".into() },
        inputs: boundary.input_fronts.iter().map(|f| f.external_port.clone()).collect(),
        outputs: boundary.output_fronts.iter().map(|f| f.external_port.clone()).collect(),
        host_calls: vec![], resource_requirements: vec![], authority_requirements: vec![],
        limits: CapabilityLimits { max_active_instances: 1, max_queue_items: 1, max_queue_bytes: maximum },
    };
    KernelCompositeDefinition {
        host_id: f.host_id.clone(),
        boot_id: f.boot_id.clone(),
        offer_generation: f.offer_generation,
        profile: "test/production-custody@1".into(),
        external_capability: external,
        internal_plan: plan,
        boundary,
        failure_translation: FailureReason::CompositeCapabilityFailed,
    }
}

impl Execution {
    pub fn transact_canonical(
        &mut self,
        sequence: u64,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<usize, &'static str> {
        if let Some(remaining) = &mut self.remaining_inferences {
            if *remaining == 0 {
                return Err("invocation pressure");
            }
            *remaining -= 1;
        }
        if input.len() > self.input_payload.encoded.capacity() {
            return Err("input pressure");
        }
        self.input_payload.encoded.clear();
        self.input_payload.encoded.extend_from_slice(input);
        if !matches!(
            self.kernel
                .admit_input(&self.input_port, sequence, &self.input_payload)
                .map_err(|_| "ingress")?,
            conduit_kernel::scheduler::RemoteIngressOutcome::Accepted { .. }
        ) {
            return Err("ingress refused");
        }
        for _ in 0..8192 {
            self.step();
            if let Some(received) = self
                .kernel
                .output_into(&self.output_port, &mut self.output_payload)
                .map_err(|_| "output")?
            {
                if received != sequence || self.output_payload.encoded.len() > output.len() {
                    return Err("output pressure");
                }
                let length = self.output_payload.encoded.len();
                output[..length].copy_from_slice(&self.output_payload.encoded);
                self.kernel
                    .complete_output(&self.output_port, received)
                    .map_err(|_| "output completion")?;
                return Ok(length);
            }
        }
        Err("finite target step envelope")
    }
}

impl crate::parser_session_canonical_ingress::ParserCanonicalSourceExecutor for Execution {
    type Error = &'static str;
    fn cancel(&mut self) {
        let _ = self.kernel.cancel();
    }
    fn entry(&self) -> &str {
        &self.entry
    }
    fn input_type_bytes(&self) -> &[u8] {
        &self.input_type_bytes
    }
    fn output_type_bytes(&self) -> &[u8] {
        &self.output_type_bytes
    }
    fn transact(
        &mut self,
        ordinal: u64,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<usize, Self::Error> {
        self.transact_canonical(ordinal, input, output)
    }
}
impl crate::numeric_custody::ParserNumericExecutor for Execution {
    type Error = &'static str;
    fn plan(&self) -> &Plan {
        &self.kernel.definition().internal_plan
    }
    fn transact(
        &mut self,
        ordinal: u64,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<usize, Self::Error> {
        self.transact_canonical(ordinal, input, output)
    }
    fn cancel(&mut self) {
        let _ = self.kernel.cancel();
    }
}
