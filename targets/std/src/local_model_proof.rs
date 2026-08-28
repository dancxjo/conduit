//! Repository-only live local-model proof through ordinary Form, Plan, and Play.

use crate::hosted_local_model::{HostedLocalModelAdapter, OllamaLocalModelAdapter};
use crate::{StdHost, StdHostComposition, StdHostConfig, TimerAdapter};
use conduit_ai::LocalModelKindProfile;
use conduit_core::{BaseImplementationId, BootId, HostId, OfferGeneration};
use conduit_form::{check_syntax_document, parse_syntax_document, ProfileCatalog, StartupCatalog};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LocalModelLiveProofReceipt {
    pub proof_class: &'static str,
    pub host_id: String,
    pub boot_id: String,
    pub model_content_identity: String,
    pub generate_plan_id: String,
    pub classify_plan_id: String,
    pub extract_plan_id: String,
    pub interpret_plan_id: String,
    pub implementation_identity: String,
    pub generate_play_completed: bool,
    pub classify_play_completed: bool,
    pub extract_play_completed: bool,
    pub interpret_play_completed: bool,
}

struct NoopTimer;

impl TimerAdapter for NoopTimer {
    fn wait(&mut self, _duration: std::time::Duration) {}
}

pub fn run(
    adapter: OllamaLocalModelAdapter,
) -> Result<LocalModelLiveProofReceipt, Box<dyn std::error::Error>> {
    let model_content_identity = adapter.offer().identity.model_content_identity.clone();
    let mut host = StdHost::new_with_local_model(
        StdHostConfig {
            host_id: HostId::from("host/local-ollama-proof"),
            boot_id: BootId::from("boot/local-ollama-proof"),
            offer_generation: OfferGeneration(1),
        },
        StdHostComposition::minimal(),
        Box::new(adapter),
    )?;
    for profile in [
        LocalModelKindProfile::Generate,
        LocalModelKindProfile::ClassifyFiniteLabels,
        LocalModelKindProfile::ExtractValidatedInfo,
        LocalModelKindProfile::InterpretSignEvidence,
    ] {
        let contract = conduit_ai::llm_contract(profile.kind()).expect("proof profiles are L0");
        host.advertisement.capabilities.extend([
            crate::installed_std::test_local_model_io::source_offer(
                contract.inputs[0].value_kind.as_str(),
            ),
            crate::installed_std::test_local_model_io::sink_offer(
                contract.outputs[0].value_kind.as_str(),
            ),
        ]);
    }
    let generate = run_profile(&mut host, LocalModelKindProfile::Generate)?;
    let classify = run_profile(&mut host, LocalModelKindProfile::ClassifyFiniteLabels)?;
    let extract = run_profile(&mut host, LocalModelKindProfile::ExtractValidatedInfo)?;
    let interpret = run_profile(&mut host, LocalModelKindProfile::InterpretSignEvidence)?;
    Ok(LocalModelLiveProofReceipt {
        proof_class: "live-local-model",
        host_id: host.advertisement.host_id.as_str().into(),
        boot_id: host.advertisement.boot_id.as_str().into(),
        model_content_identity,
        generate_plan_id: generate.0,
        classify_plan_id: classify.0,
        extract_plan_id: extract.0,
        interpret_plan_id: interpret.0,
        implementation_identity: conduit_ai::LOCAL_MODEL_IMPLEMENTATION.into(),
        generate_play_completed: generate.1,
        classify_play_completed: classify.1,
        extract_play_completed: extract.1,
        interpret_play_completed: interpret.1,
    })
}

fn run_profile(
    host: &mut StdHost,
    profile: LocalModelKindProfile,
) -> Result<(String, bool), Box<dyn std::error::Error>> {
    let contract = conduit_ai::llm_contract(profile.kind()).expect("proof profiles are L0");
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    conduit_ai::install_llm_semantic_catalog(&mut startup, &mut profiles)?;
    crate::installed_std::test_local_model_io::install_catalog(
        &mut startup,
        &mut profiles,
        contract.inputs[0].value_kind.as_str(),
        contract.outputs[0].value_kind.as_str(),
    );
    let source = format!(
        "form run {{\n source: conduit-test/local-model-request\n model: {}(4096, 1, 4096, 4096, 0)\n sink: conduit-test/local-model-result\n source.value > model.request\n model.result > sink.value\n}}\n",
        profile.kind()
    );
    let checked =
        check_syntax_document(&parse_syntax_document(&source), &startup).map_err(|error| {
            format!(
                "local-model proof Form check: {} {}",
                error.code, error.message
            )
        })?;
    let expanded =
        conduit_form::expand_canonical_form(&checked, "run", &profiles).map_err(|error| {
            format!(
                "local-model proof expansion: {} {}",
                error.code, error.message
            )
        })?;
    let hosts = vec![host.advertisement().clone()];
    let placements = conduit_planner::default_expanded_placements(&expanded, &hosts)?;
    let connection_bases = BTreeMap::new();
    let line_candidates = BTreeMap::new();
    let plan = conduit_planner::plan_expanded_canonical_with_options(
        &expanded,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &connection_bases,
            line_candidates: &line_candidates,
            connection_item_capacity: 4,
            connection_byte_capacity: 4_096,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )?;
    let plan_id = plan.plan_id.as_str().to_string();
    let fragment = plan
        .fragments
        .into_iter()
        .next()
        .ok_or("local-model proof Plan has no fragment")?;
    let connection_limits = fragment
        .connections
        .iter()
        .map(|connection| (connection.item_capacity, connection.byte_capacity))
        .collect::<Vec<_>>();
    let mut output = Vec::new();
    let report = host
        .run_fragment_to(fragment, &mut output, &mut NoopTimer)
        .map_err(|error| {
            format!(
                "{} live Plan/Play: {error}; connection limits {connection_limits:?}",
                profile.kind()
            )
        })?;
    Ok((plan_id, report.kernel.is_some()))
}
