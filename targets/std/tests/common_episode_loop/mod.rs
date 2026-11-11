//! Shared harness for the basic episode loop proof: ordinary source -> check
//! -> expand -> Plan -> one kernel Play on the std Host.
//!
//! Deterministic conformance uses the fenced fake. The opt-in hosted proof
//! supplies the production local model adapter to the same installed graph.
#![allow(dead_code)]

use conduit_body::{Body, BodyConversationContext, BodyConversationContextBasis};
use conduit_chat::{EpisodeObservation, EpisodeRecord};
use conduit_core::{
    BaseImplementationId, BootId, CheckedPlotId, ConnectionTrack, HostId, ObservationKind,
    OfferGeneration, PlanFragment, PortDirection, SignId, SourceDocumentId, TerminalDisposition,
};
use conduit_planner::{
    default_expanded_placements, ConnectionQueueLimits, ForeBoundaryKey, PlanningOptions,
};
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use conduit_std_host::hosted_local_model::{HostedLocalModelAdapter, LocalModelAdapterTerminal};
use conduit_std_host::{
    ExternalForeDelivery, ExternalForeInput, ExternalForeOutputAdapter, RunControl, StdHost,
    StdHostComposition, StdHostConfig, StdRunReport, TimerAdapter,
};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

pub const PROGRAM: &str = include_str!("../../../../plots/episode-loop/main.conduit");

/// Fenced deterministic fake: answers from the exact prompt it was given.
struct FencedFakeEpisodeModel {
    offer: conduit_ai::LocalModelOffer,
    requests: Arc<Mutex<Vec<String>>>,
    terminal_on_call: Option<(usize, LocalModelAdapterTerminal)>,
}

impl HostedLocalModelAdapter for FencedFakeEpisodeModel {
    fn offer(&self) -> &conduit_ai::LocalModelOffer {
        &self.offer
    }

    fn current_pool_health(&self) -> conduit_core::PoolRealizationHealth {
        conduit_core::PoolRealizationHealth::Ready
    }

    fn execute(
        &mut self,
        placement: &conduit_core::PlannedGear,
        input: &[u8],
        output: &mut Vec<u8>,
    ) -> LocalModelAdapterTerminal {
        output.clear();
        let mut requests = self.requests.lock().unwrap();
        requests.push(String::from_utf8(input.to_vec()).unwrap());
        let call = requests.len();
        if let Some((failing, terminal)) = self.terminal_on_call {
            if failing == call {
                return terminal;
            }
        }
        let prompt: serde_json::Value = serde_json::from_slice(input).unwrap();
        let answer = if let Some(evidence) = prompt["evidence"].as_array() {
            format!("{} selected evidence items", evidence.len())
        } else {
            let now = prompt["observation"]["content"].as_str().unwrap();
            match prompt["recalled"].as_array().unwrap().last() {
                None => format!("No memory yet; I notice: {now}"),
                Some(memory) => format!(
                    "I recall {} and now notice: {now}",
                    memory["observation"]["content"].as_str().unwrap()
                ),
            }
        };
        let contract = conduit_ai::llm_contract(placement.kind_id.as_str()).unwrap();
        let payload = answer.into_bytes();
        let result = conduit_ai::ModelDerivedResult {
            provenance: conduit_ai::ModelResultProvenance::ModelDerived,
            payload_kind: contract.result_payload_kind.as_str().into(),
            accounting: conduit_ai::ModelWorkAccounting {
                input_bytes: input.len() as u64,
                context_items: 1,
                output_bytes: payload.len() as u64,
                work_units: 1,
                history_items: 0,
            },
            payload,
            implementation_identity: "fenced-fake/episode-model".into(),
            request_identity: format!("request/fake-{call}"),
            run_identity: format!("run/fake-{call}"),
            confidence: None,
            disposition: conduit_ai::ModelResultDisposition::Produced,
            determinism: self.offer.determinism,
        };
        output.extend_from_slice(&serde_json::to_vec(&result).unwrap());
        LocalModelAdapterTerminal::Produced
    }
}

fn offer() -> conduit_ai::LocalModelOffer {
    use conduit_ai::*;
    LocalModelOffer {
        identity: LocalModelIdentity {
            runtime_name: "fenced-fake".into(),
            runtime_version: "1".into(),
            runtime_build_identity: "fenced-fake/build-1".into(),
            model_name: "episode-model".into(),
            model_content_identity: "sha256-fenced-fake-episode".into(),
            architecture: "fixture".into(),
            parameter_profile: "tiny".into(),
            quantization: "exact".into(),
        },
        limits: LocalModelLimits {
            work: LlmWorkBounds::new(4_096, 1, 4_096, 4_096, 0).unwrap(),
            model_bytes: 1,
            admitted_memory_mib: 1,
            compute: LocalModelComputeNeed {
                minimum_lanes: 1,
                preferred_lanes: 1,
                maximum_lanes: 1,
                minimum_service_guarantee: conduit_core::ComputeServiceGuarantee::Shared,
            },
            maximum_in_flight: 1,
            maximum_queue_items: 1,
            maximum_queue_bytes: 16_384,
            cancellation_supported: true,
            cache_policy: LocalModelCachePolicy::OneLoadedModelUntilShutdown,
        },
        supported_profiles: vec![LocalModelKindProfile::GenerateFlow],
        initialized: true,
        lifecycle: LocalModelLifecycleState::Ready,
        determinism: LlmDeterminismProfile::ProviderNondeterministic,
    }
}

pub fn context() -> BodyConversationContext {
    let body = Body::born(
        SourceDocumentId::from("source/episode-loop"),
        CheckedPlotId::from("checked/episode-loop"),
        1,
        SignId::from("sign/episode-loop/born"),
    )
    .unwrap();
    let (_, wake) = body
        .wake(2, SignId::from("sign/episode-loop/wake"))
        .unwrap();
    BodyConversationContext {
        schema: "conduit.body/conversation-context-value@2".into(),
        display_name: "Episode Body".into(),
        body_id: wake.body_id.clone(),
        wake_id: wake.wake_id.clone(),
        wake_sequence: 2,
        basis: BodyConversationContextBasis {
            body_id: wake.body_id,
            wake_id: wake.wake_id,
            wake_sequence: 2,
            revision: 1,
        },
        hosts: vec![],
        active_plots: vec!["episode-loop".into()],
        current_plan_id: None,
        active_play_id: None,
        lines: vec![],
        recent_sign_ids: vec![],
    }
}

pub struct Harness {
    pub host: StdHost,
    pub requests: Arc<Mutex<Vec<String>>>,
    pub fragment: PlanFragment,
    pub plan_id: String,
    pub authoring: conduit_plot::ExpandedAuthoringPlot,
}

pub fn harness(terminal_on_call: Option<(usize, LocalModelAdapterTerminal)>) -> Harness {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let model = FencedFakeEpisodeModel {
        offer: offer(),
        requests: requests.clone(),
        terminal_on_call,
    };
    let mut harness = harness_with_model(Box::new(model));
    harness.requests = requests;
    harness
}

pub fn harness_with_model(model: Box<dyn HostedLocalModelAdapter>) -> Harness {
    plan_with_model(
        model,
        PROGRAM,
        "episode-loop",
        &[
            ("context", PortDirection::Input, 32_768),
            ("observations", PortDirection::Input, 32_768),
            ("episodes", PortDirection::Output, 32_768),
        ],
        StdHostComposition::minimal(),
    )
}

pub fn experience_episode_harness(program: &str) -> Harness {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let model = FencedFakeEpisodeModel {
        offer: offer(),
        requests: requests.clone(),
        terminal_on_call: None,
    };
    let mut harness = experience_episode_harness_with_model(program, Box::new(model));
    harness.requests = requests;
    harness
}

pub fn experience_episode_harness_with_model(
    program: &str,
    model: Box<dyn HostedLocalModelAdapter>,
) -> Harness {
    plan_with_model(
        model,
        program,
        "experience-episode-loop",
        &[
            ("context", PortDirection::Input, 32_768),
            (
                "experiences",
                PortDirection::Input,
                conduit_chat::MAXIMUM_EPISODE_EXPERIENCE_BYTES as u32,
            ),
            ("episodes", PortDirection::Output, 32_768),
        ],
        StdHostComposition::minimal(),
    )
}

pub fn fenced_model() -> (Box<dyn HostedLocalModelAdapter>, Arc<Mutex<Vec<String>>>) {
    fenced_model_with_terminal(None)
}

pub fn fenced_model_with_terminal(
    terminal_on_call: Option<(usize, LocalModelAdapterTerminal)>,
) -> (Box<dyn HostedLocalModelAdapter>, Arc<Mutex<Vec<String>>>) {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let model = FencedFakeEpisodeModel {
        offer: offer(),
        requests: requests.clone(),
        terminal_on_call,
    };
    (Box::new(model), requests)
}

pub fn evidence_harness(program: &str) -> Harness {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let model = FencedFakeEpisodeModel {
        offer: offer(),
        requests: requests.clone(),
        terminal_on_call: None,
    };
    let mut harness = evidence_harness_with_model(program, Box::new(model));
    harness.requests = requests;
    harness
}

pub fn evidence_harness_with_model(
    program: &str,
    model: Box<dyn HostedLocalModelAdapter>,
) -> Harness {
    plan_with_model(
        model,
        program,
        "experience-response",
        &[
            ("evidence", PortDirection::Input, 65_536),
            ("responses", PortDirection::Output, 2_560),
        ],
        StdHostComposition::minimal().with_experience_evidence(),
    )
}

fn plan_with_model(
    model: Box<dyn HostedLocalModelAdapter>,
    program: &str,
    entry: &str,
    fore: &[(&str, PortDirection, u32)],
    composition: StdHostComposition,
) -> Harness {
    plan_host(
        model_host(model, composition),
        program,
        entry,
        fore,
        composition.experience_evidence,
        |_, _| Ok(()),
    )
}

pub fn model_host(
    model: Box<dyn HostedLocalModelAdapter>,
    composition: StdHostComposition,
) -> StdHost {
    StdHost::new_with_local_model(
        StdHostConfig {
            host_id: HostId::from("host/episode-loop"),
            boot_id: BootId::from("boot/episode-loop"),
            offer_generation: OfferGeneration(1),
        },
        composition,
        model,
    )
    .unwrap()
}

mod planning;
pub use planning::plan_host;

pub fn observation(identity: &str, content: &str, retain: bool) -> ExternalForeInput {
    ExternalForeInput {
        front_port_id: conduit_core::port_id("observations"),
        track: ConnectionTrack::Payload,
        bytes: EpisodeObservation::new(content.into(), identity.into(), retain)
            .unwrap()
            .encode()
            .unwrap(),
    }
}

pub fn inputs(observations: &[(&str, &str, bool)]) -> Vec<ExternalForeInput> {
    let mut inputs = vec![ExternalForeInput {
        front_port_id: conduit_core::port_id("context"),
        track: ConnectionTrack::Payload,
        bytes: conduit_chat::encode_body_conversation_context(&context()).unwrap(),
    }];
    inputs.extend(
        observations
            .iter()
            .map(|(identity, content, retain)| observation(identity, content, *retain)),
    );
    inputs
}

#[derive(Default)]
pub struct Collector(pub Vec<ExternalForeDelivery>);

impl ExternalForeOutputAdapter for Collector {
    fn deliver(&mut self, output: ExternalForeDelivery) -> Result<(), String> {
        self.0.push(output);
        Ok(())
    }
}

pub struct NoopTimer;

impl TimerAdapter for NoopTimer {
    fn wait(&mut self, _duration: std::time::Duration) {}
}

pub fn run(
    harness: &mut Harness,
    inputs: &[ExternalForeInput],
    outputs: &mut dyn ExternalForeOutputAdapter,
    control: &RunControl,
) -> Result<StdRunReport, String> {
    harness.host.run_external_plot_sequence_controlled_to(
        harness.fragment.clone(),
        inputs,
        outputs,
        &mut Vec::new(),
        &mut NoopTimer,
        control,
    )
}

pub fn records(deliveries: &[ExternalForeDelivery]) -> Vec<EpisodeRecord> {
    deliveries
        .iter()
        .map(|delivery| {
            assert_eq!(delivery.front_port_id.as_str(), "episodes");
            EpisodeRecord::decode(&delivery.bytes).unwrap()
        })
        .collect()
}

pub fn terminal(report: &StdRunReport) -> &TerminalDisposition {
    match report.observations.last().map(|event| &event.kind) {
        Some(ObservationKind::PlanTerminal { disposition }) => disposition,
        other => panic!("missing Plan terminal: {other:?}"),
    }
}
