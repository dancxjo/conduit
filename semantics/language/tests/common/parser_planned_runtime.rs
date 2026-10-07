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
}
pub fn prepare(profile: Arc<PreparedCategoricalStep>) -> Execution {
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
    let checked = check_syntax_document(
        &parse_syntax_document(&source(&profile.kind_identity(true), score_bytes)),
        &startup,
    )
    .unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, "learned-model", &catalogs).unwrap();
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
    let kernel = KernelCompositeHost::prepare_with_sign_storage(
        definition,
        &registry,
        KernelCompositeSignStorage {
            additional_local_items: 1024,
            additional_remote_items: 256,
        },
    )
    .unwrap();
    drop(registry);
    let mut execution = Execution { kernel, pure };
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
    pub fn infer(&mut self, sequence: u64, features: &StructuredInfoValue) -> StructuredInfoValue {
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
