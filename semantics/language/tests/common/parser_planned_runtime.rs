//! Ordinary retained Source numeric projection and exact resource model Plan/Play.
use super::fixture;
use conduit_ai::integer_categorical_step::{
    owner::CategoricalOperationFactory, PreparedCategoricalStep, CATEGORICAL_STEP_IMPLEMENTATION,
};
use conduit_composite::*;
use conduit_core::*;
use conduit_plot::*;
use conduitos::{
    expression_host_call::{ExpressionHostCall, ExpressionOperationFactory},
    structured_selector_host_call::{SelectorHostCall, SelectorOperationFactory},
};
use std::{collections::BTreeMap, sync::Arc};
fn source(kind: &str, score_bytes: u32) -> String {
    [fixture::parser_source(), include_str!("../../text_revision.conduit").into(),
     include_str!("../../lexical.conduit").into(), include_str!("../../parser_available.conduit").into(),
     include_str!("../../parser_scorer_v2.conduit").into(), include_str!("../../parser_numeric_ports.conduit").into(),
     format!("plot learned-model (\n features: LanguageParserV2ModelFeatures...| >> scores: LanguageParserCategoricalScores...| <= {score_bytes}B\n) {{\n projection: language-parser-v2-feature-indices\n model: {kind}\n features >> projection.features\n projection.indices >> model.indices\n model.scores >> scores\n}}\n")].join("\n")
}
enum Pure {
    Expression(ExpressionHostCall),
    Selector(SelectorHostCall),
}
pub struct Execution {
    kernel: KernelCompositeHost,
    pure: Vec<(conduit_kernel::NodeId, Pure)>,
    remaining_inferences: Option<u16>,
}
pub fn prepare(profile: Arc<PreparedCategoricalStep>) -> Execution {
    prepare_model_with_budget(profile, None)
}
#[allow(dead_code)]
pub fn prepare_with_inference_budget(
    profile: Arc<PreparedCategoricalStep>,
    maximum: u16,
) -> Execution {
    assert!(maximum > 0);
    maximum
        .checked_mul(4)
        .expect("journal item bound overflows");
    prepare_model_with_budget(profile, Some(maximum))
}
fn prepare_model_with_budget(
    profile: Arc<PreparedCategoricalStep>,
    maximum: Option<u16>,
) -> Execution {
    let model_contract = profile.contract(true).unwrap();
    let KindSemanticLaw::ValueContracts(contracts) = &model_contract.semantic_laws[0] else {
        panic!("exact numerical envelopes")
    };
    let score_bytes = contracts
        .iter()
        .find(|c| c.location == FrontValueLocation::Output(port_id("scores")))
        .unwrap()
        .contract
        .maximum_bytes;
    let document = source(&profile.kind_identity(true), score_bytes);
    prepare_source_with_storage(profile, document, "learned-model", maximum)
}
/// Execute a caller-authored bounded feature projection/model entry with the
/// exact selected numerical types and resource custody. No grammar is selected here.
#[allow(dead_code)] // Shared helper also serves authored-projection test targets.
pub fn prepare_source(
    profile: Arc<PreparedCategoricalStep>,
    document: String,
    entry: &str,
) -> Execution {
    prepare_source_with_storage(profile, document, entry, None)
}
/// Admit a finite batch before Play. Each input/output boundary round trip
/// retains four remote lifecycle Signs per child, measured by the journal test.
/// The caller supplies its finite loop bound; the harness enforces it before ingress.
#[allow(dead_code)]
pub fn prepare_source_with_inference_budget(
    profile: Arc<PreparedCategoricalStep>,
    document: String,
    entry: &str,
    maximum_inferences: u16,
) -> Execution {
    assert!(maximum_inferences > 0, "a finite batch must admit work");
    maximum_inferences
        .checked_mul(4)
        .expect("journal item bound overflows");
    prepare_source_with_storage(profile, document, entry, Some(maximum_inferences))
}
/// Distinct fixed aliases preserve the legacy 25-feature catalog identity.
#[allow(dead_code)]
pub fn prepare_proposal_window8_v2_source_with_inference_budget(
    profile: Arc<PreparedCategoricalStep>,
    document: String,
    entry: &str,
    maximum_inferences: u16,
) -> Execution {
    assert!(maximum_inferences > 0);
    maximum_inferences
        .checked_mul(4)
        .and_then(|items| items.checked_add(1024))
        .expect("combined retained journal bound overflows before Source preparation");
    prepare_source_with_aliases(
        profile,
        document,
        entry,
        Some(maximum_inferences),
        "LanguageParserWindow8ProposerV2CategoricalIndices",
        "LanguageParserWindow8ProposerV2CategoricalScores",
    )
}
fn prepare_source_with_storage(
    profile: Arc<PreparedCategoricalStep>,
    document: String,
    entry: &str,
    maximum_inferences: Option<u16>,
) -> Execution {
    prepare_source_with_aliases(
        profile,
        document,
        entry,
        maximum_inferences,
        "LanguageParserCategoricalIndices",
        "LanguageParserCategoricalScores",
    )
}
fn prepare_source_with_aliases(
    profile: Arc<PreparedCategoricalStep>,
    document: String,
    entry: &str,
    maximum_inferences: Option<u16>,
    indices_alias: &str,
    scores_alias: &str,
) -> Execution {
    let mut startup = StartupCatalog::new();
    let mut catalogs = ProfileCatalog::new();
    profile.install(&mut startup, &mut catalogs, true).unwrap();
    startup
        .insert_structured_type(indices_alias, profile.indices_type().clone())
        .unwrap();
    startup
        .insert_structured_type(scores_alias, profile.scores_type().clone())
        .unwrap();
    let checked = check_syntax_document(&parse_syntax_document(&document), &startup).unwrap();
    let expanded = expand_canonical_plot_for_authoring(&checked, entry, &catalogs).unwrap();
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
            "program" => conduitos::expression_host_call::offer(
                &PortableExpressionProgram::from_canonical_hex(encoded).unwrap(),
                temporal,
            )
            .unwrap(),
            "selector" => conduitos::structured_selector_host_call::offer(
                &StructuredSelector::from_canonical_hex(encoded).unwrap(),
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
                    byte_capacity: selected_offer(&b.gear_id).limits.max_queue_bytes.min(4096),
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
            connection_byte_capacity: 4096,
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
    assert_eq!(
        plan.fragments[0]
            .placements
            .iter()
            .filter(|p| p.implementation_id.as_str() == CATEGORICAL_STEP_IMPLEMENTATION)
            .count(),
        1
    );
    let factory = CategoricalOperationFactory::for_plan(&plan, &[profile]).unwrap();
    let definition = conduitos::protocol_artifact::AdmittedProtocolArtifact::admit(
        conduitos::protocol_artifact::ProtocolArtifactIdentity {
            source: plan.source_document_id.clone(),
            checked: plan.checked_plot_id.clone(),
            expanded: plan.expanded_plot_id.clone(),
            artifact: "test/learned-model-source".into(),
        },
        plan,
    )
    .unwrap()
    .into_definition();
    let fragment = &definition.internal_plan.fragments[0];
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(fragment).unwrap();
    let active = bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    let pure = fragment
        .placements
        .iter()
        .filter_map(|gear| {
            let owner = match gear.implementation_id.as_str() {
                conduitos::expression_host_call::IMPLEMENTATION => Pure::Expression(
                    ExpressionHostCall::prepare(fragment, &lowered, &active, &gear.placement_id)
                        .unwrap(),
                ),
                conduitos::structured_selector_host_call::IMPLEMENTATION => Pure::Selector(
                    SelectorHostCall::prepare(fragment, &lowered, &active, &gear.placement_id)
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
    registry
        .install(SelectorOperationFactory::default())
        .unwrap();
    registry.install(factory).unwrap();
    eprintln!("numeric Source Plan journal structural input: lowered_sign_items={}, active_nodes={}, active_cords={}, additional_model_calls={maximum_inferences:?}; full Kernel factory admission follows",lowered.sign_items,lowered.nodes.len(),lowered.cords.len());
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
    let mut execution = Execution {
        kernel,
        pure,
        remaining_inferences: maximum_inferences,
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
                Pure::Selector(p) => p.invoke(node, call.call, call.request, input).unwrap(),
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
            encoded: Vec::with_capacity(4096),
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
