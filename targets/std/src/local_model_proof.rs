//! Repository-only live local-model proof through ordinary plot, Plan, and Play.

use crate::hosted_local_model::{
    HostedLocalModelAdapter, LocalModelAdapterTerminal, OllamaLocalModelAdapter,
};
#[path = "local_model_proof/execution.rs"]
mod execution;
use execution::{run_expanded, run_profile, sha256};

use crate::{StdHost, StdHostComposition, StdHostConfig, TimerAdapter};
use conduit_ai::LocalModelKindProfile;
use conduit_core::{BaseImplementationId, BootId, HostId, OfferGeneration};
use conduit_plot::{check_syntax_document, parse_syntax_document, ProfileCatalog, StartupCatalog};
use conduit_presentation::{GeneratedManifestationCandidate, GenerativePresenterRequest};
use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LocalModelLiveProofReceipt {
    pub proof_class: String,
    pub host_id: String,
    pub boot_id: String,
    pub model_content_identity: String,
    pub generate_plan_id: String,
    pub classify_plan_id: String,
    pub extract_plan_id: String,
    pub interpret_plan_id: String,
    pub present_plan_id: String,
    pub house_plan_id: String,
    pub implementation_identity: String,
    pub generate_play_completed: bool,
    pub classify_play_completed: bool,
    pub extract_play_completed: bool,
    pub interpret_play_completed: bool,
    pub present_play_completed: bool,
    pub house_play_completed: bool,
    pub house_response_bytes: u32,
    pub house_response_sha256: String,
    pub house_speech: conduit_tongues::SpeechRunReceipt,
    pub presenter_requests: Vec<PresenterRequestProofReceipt>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PresenterRequestProofReceipt {
    pub plan_id: String,
    pub play_completed: bool,
    pub request_identity: String,
    pub source_presentation_identity: String,
    pub source_presentation_revision: u64,
    pub policy_revision: String,
    pub manifestation: GeneratedManifestationCandidate,
}

/// One exact Face Presenter Plan/Play, without making unrelated model kinds
/// prerequisites for the candidate. The owner Face remains external to this
/// local model Host; its identity is bound by the request and candidate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LocalModelPresenterProofReceipt {
    pub proof_class: String,
    pub host_id: String,
    pub boot_id: String,
    pub model_content_identity: String,
    pub implementation_identity: String,
    pub presenter: PresenterRequestProofReceipt,
}

struct CapturingLocalModelAdapter {
    inner: Box<dyn HostedLocalModelAdapter>,
    generated_text: Arc<Mutex<Option<Vec<u8>>>>,
    presenter_manifestations: Arc<Mutex<Vec<GeneratedManifestationCandidate>>>,
}

impl HostedLocalModelAdapter for CapturingLocalModelAdapter {
    fn offer(&self) -> &conduit_ai::LocalModelOffer {
        self.inner.offer()
    }

    fn execute(
        &mut self,
        placement: &conduit_core::PlannedGear,
        input: &[u8],
        output: &mut Vec<u8>,
    ) -> LocalModelAdapterTerminal {
        let terminal = self.inner.execute(placement, input, output);
        if placement.kind_id.as_str() == conduit_ai::LLM_GENERATE_KIND
            && matches!(
                terminal,
                LocalModelAdapterTerminal::Produced | LocalModelAdapterTerminal::Truncated
            )
        {
            if let Ok(result) = serde_json::from_slice::<conduit_ai::ModelDerivedResult>(output) {
                if result.payload_kind == conduit_ai::GENERATED_RESULT_VALUE_KIND {
                    let Ok(mut captured) = self.generated_text.lock() else {
                        return LocalModelAdapterTerminal::Failed;
                    };
                    *captured = Some(result.payload);
                }
            }
        }
        if placement.kind_id.as_str() == conduit_ai::LLM_PRESENT_KIND
            && matches!(
                terminal,
                LocalModelAdapterTerminal::Produced | LocalModelAdapterTerminal::Truncated
            )
        {
            let Ok(manifestation) =
                serde_json::from_slice::<GeneratedManifestationCandidate>(output)
            else {
                return LocalModelAdapterTerminal::Failed;
            };
            let Ok(mut captured) = self.presenter_manifestations.lock() else {
                return LocalModelAdapterTerminal::Failed;
            };
            captured.push(manifestation);
        }
        terminal
    }
}

struct NoopTimer;

impl TimerAdapter for NoopTimer {
    fn wait(&mut self, _duration: std::time::Duration) {}
}

pub fn run(
    adapter: OllamaLocalModelAdapter,
    presenter_requests: &[GenerativePresenterRequest],
) -> Result<LocalModelLiveProofReceipt, Box<dyn std::error::Error>> {
    let proof_class = if adapter
        .offer()
        .identity
        .runtime_version
        .contains("fixture-http")
    {
        "ollama-http-fixture"
    } else {
        "live-local-model"
    };
    let model_content_identity = adapter.offer().identity.model_content_identity.clone();
    let generated_text = Arc::new(Mutex::new(None));
    let presenter_manifestations = Arc::new(Mutex::new(Vec::new()));
    let mut additional_capabilities = Vec::new();
    for profile in [
        LocalModelKindProfile::Generate,
        LocalModelKindProfile::ClassifyFiniteLabels,
        LocalModelKindProfile::ExtractValidatedInfo,
        LocalModelKindProfile::InterpretSignEvidence,
        LocalModelKindProfile::PresentSemanticFront,
    ] {
        let contract = conduit_ai::llm_contract(profile.kind()).expect("proof profiles are L0");
        additional_capabilities.extend([
            crate::installed_std::test_local_model_io::source_offer(
                contract.inputs[0].value_kind.as_str(),
            ),
            crate::installed_std::test_local_model_io::sink_offer(
                contract.outputs[0].value_kind.as_str(),
            ),
        ]);
    }
    additional_capabilities
        .extend(crate::installed_std::test_local_model_io::house_source_offers());
    additional_capabilities.push(crate::installed_std::recorded_speech_back::offer());
    additional_capabilities
        .push(crate::installed_std::test_local_model_io::house_text_sink_offer());
    let mut host = StdHost::new_with_local_model_capabilities(
        StdHostConfig {
            host_id: HostId::from("host/local-ollama-proof"),
            boot_id: BootId::from("boot/local-ollama-proof"),
            offer_generation: OfferGeneration(1),
        },
        StdHostComposition::minimal(),
        Box::new(CapturingLocalModelAdapter {
            inner: Box::new(adapter),
            generated_text: Arc::clone(&generated_text),
            presenter_manifestations: Arc::clone(&presenter_manifestations),
        }),
        additional_capabilities,
    )?;
    let generate = run_profile(&mut host, LocalModelKindProfile::Generate)?;
    let classify = run_profile(&mut host, LocalModelKindProfile::ClassifyFiniteLabels)?;
    let extract = run_profile(&mut host, LocalModelKindProfile::ExtractValidatedInfo)?;
    let interpret = run_profile(&mut host, LocalModelKindProfile::InterpretSignEvidence)?;
    let mut present = None;
    if presenter_requests.is_empty() {
        present = Some(run_profile(
            &mut host,
            LocalModelKindProfile::PresentSemanticFront,
        )?);
        presenter_manifestations
            .lock()
            .map_err(|_| "presenter proof capture lock is poisoned")?
            .clear();
    }
    let mut presenter_receipts = Vec::with_capacity(presenter_requests.len());
    for request in presenter_requests {
        request
            .validate()
            .map_err(|error| format!("invalid supplied Presenter request: {error:?}"))?;
        let encoded = serde_json::to_vec(request)?;
        let execution =
            crate::installed_std::test_local_model_io::with_generative_presenter_request(
                encoded,
                || run_profile(&mut host, LocalModelKindProfile::PresentSemanticFront),
            )?;
        present.get_or_insert_with(|| execution.clone());
        let manifestation = presenter_manifestations
            .lock()
            .map_err(|_| "presenter proof capture lock is poisoned")?
            .pop()
            .ok_or("Presenter proof produced no captured candidate")?;
        request
            .validate_candidate(&manifestation)
            .map_err(|error| format!("invalid provider candidate: {error:?}"))?;
        presenter_receipts.push(PresenterRequestProofReceipt {
            plan_id: execution.0,
            play_completed: execution.1,
            request_identity: request.request_identity.clone(),
            source_presentation_identity: request
                .semantic_data
                .source_presentation_identity
                .clone(),
            source_presentation_revision: request.semantic_data.source_presentation_revision,
            policy_revision: request.policy.template_contract_revision.clone(),
            manifestation,
        });
    }
    let present = present.ok_or("local-model proof did not execute a Presenter request")?;
    *generated_text
        .lock()
        .map_err(|_| "local proof response capture lock is poisoned")? = None;
    let house = run_house(&mut host)?;
    let house_response = generated_text
        .lock()
        .map_err(|_| "local proof response capture lock is poisoned")?
        .take()
        .ok_or("House model produced no captured text response")?;
    let house_response = String::from_utf8(house_response)?;
    let house_response_bytes = u32::try_from(house_response.len())?;
    let house_response_sha256 = sha256(house_response.as_bytes());
    let house_speech = conduit_tongues::run_speech_text(
        &house_response,
        conduit_tongues::OutputCondition::DegradedWavArtifact,
        conduit_tongues::SpeechFault::None,
    )?;
    Ok(LocalModelLiveProofReceipt {
        proof_class: proof_class.into(),
        host_id: host.advertisement.host_id.as_str().into(),
        boot_id: host.advertisement.boot_id.as_str().into(),
        model_content_identity,
        generate_plan_id: generate.0,
        classify_plan_id: classify.0,
        extract_plan_id: extract.0,
        interpret_plan_id: interpret.0,
        present_plan_id: present.0,
        house_plan_id: house.0,
        implementation_identity: conduit_ai::LOCAL_MODEL_IMPLEMENTATION.into(),
        generate_play_completed: generate.1,
        classify_play_completed: classify.1,
        extract_play_completed: extract.1,
        interpret_play_completed: interpret.1,
        present_play_completed: present.1,
        house_play_completed: house.1,
        house_response_bytes,
        house_response_sha256,
        house_speech,
        presenter_requests: presenter_receipts,
    })
}

pub fn run_presenter_only(
    adapter: OllamaLocalModelAdapter,
    request: &GenerativePresenterRequest,
) -> Result<LocalModelPresenterProofReceipt, Box<dyn std::error::Error>> {
    request
        .validate()
        .map_err(|error| format!("invalid supplied Presenter request: {error:?}"))?;
    let proof_class = if adapter
        .offer()
        .identity
        .runtime_version
        .contains("fixture-http")
    {
        "ollama-http-fixture"
    } else {
        "live-local-model"
    };
    let model_content_identity = adapter.offer().identity.model_content_identity.clone();
    let presenter_manifestations = Arc::new(Mutex::new(Vec::new()));
    let contract = conduit_ai::llm_contract(LocalModelKindProfile::PresentSemanticFront.kind())
        .expect("Presenter profile is L0");
    let additional_capabilities = vec![
        crate::installed_std::test_local_model_io::source_offer(
            contract.inputs[0].value_kind.as_str(),
        ),
        crate::installed_std::test_local_model_io::sink_offer(
            contract.outputs[0].value_kind.as_str(),
        ),
    ];
    let mut host = StdHost::new_with_local_model_capabilities(
        StdHostConfig {
            host_id: HostId::from("host/local-ollama-presenter-proof"),
            boot_id: BootId::from("boot/local-ollama-presenter-proof"),
            offer_generation: OfferGeneration(1),
        },
        StdHostComposition::minimal(),
        Box::new(CapturingLocalModelAdapter {
            inner: Box::new(adapter),
            generated_text: Arc::new(Mutex::new(None)),
            presenter_manifestations: Arc::clone(&presenter_manifestations),
        }),
        additional_capabilities,
    )?;
    let encoded = serde_json::to_vec(request)?;
    let (plan_id, play_completed) =
        crate::installed_std::test_local_model_io::with_generative_presenter_request(
            encoded,
            || run_profile(&mut host, LocalModelKindProfile::PresentSemanticFront),
        )?;
    let manifestation = presenter_manifestations
        .lock()
        .map_err(|_| "Presenter proof capture lock is poisoned")?
        .pop()
        .ok_or("Presenter proof produced no captured candidate")?;
    request
        .validate_candidate(&manifestation)
        .map_err(|error| format!("invalid provider candidate: {error:?}"))?;
    Ok(LocalModelPresenterProofReceipt {
        proof_class: proof_class.into(),
        host_id: host.advertisement.host_id.as_str().into(),
        boot_id: host.advertisement.boot_id.as_str().into(),
        model_content_identity,
        implementation_identity: conduit_ai::LOCAL_MODEL_IMPLEMENTATION.into(),
        presenter: PresenterRequestProofReceipt {
            plan_id,
            play_completed,
            request_identity: request.request_identity.clone(),
            source_presentation_identity: request
                .semantic_data
                .source_presentation_identity
                .clone(),
            source_presentation_revision: request.semantic_data.source_presentation_revision,
            policy_revision: request.policy.template_contract_revision.clone(),
            manifestation,
        },
    })
}

pub(crate) fn run_house(host: &mut StdHost) -> Result<(String, bool), Box<dyn std::error::Error>> {
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    conduit_text::install_text_catalogs(&mut startup, &mut profiles)?;
    conduit_tongues::install_speech_recognition_catalog(&mut startup, &mut profiles)?;
    conduit_ai::install_llm_semantic_catalog(&mut startup, &mut profiles)?;
    conduit_ai::install_model_text_catalog(&mut startup, &mut profiles)?;
    conduit_tongues::install_house_conversation_catalog(&mut startup, &mut profiles)?;
    crate::installed_std::test_local_model_io::install_catalog(
        &mut startup,
        &mut profiles,
        conduit_ai::GENERATION_REQUEST_VALUE_KIND,
        conduit_ai::GENERATED_RESULT_VALUE_KIND,
    );
    crate::installed_std::test_local_model_io::install_house_source_catalog(
        &mut startup,
        &mut profiles,
    );
    crate::installed_std::test_local_model_io::install_house_text_sink_catalog(
        &mut startup,
        &mut profiles,
    );
    let source = format!(
        "{}\n{}\nplot house-live-proof {{\n audio: {}\n recognize: speech/recognize\n recognized: speech/recognition-to-text\n addresses: {}\n addressed: addressed-utterance\n context: {}\n house: house-conversation\n sink: {}\n audio.value >> recognize.audio\n recognize.result >> recognized.result\n recognized.text >> addressed.recognized\n addresses.value >> addressed.addresses\n addressed.detection >> house.detection\n context.value >> house.context\n house.response >> sink.value\n}}\n",
        include_str!("../../../plots/addressed-utterance/main.conduit"),
        include_str!("../../../plots/house-conversation/main.conduit"),
        crate::installed_std::test_local_model_io::HOUSE_AUDIO_SOURCE_KIND,
        crate::installed_std::test_local_model_io::HOUSE_ADDRESSES_SOURCE_KIND,
        crate::installed_std::test_local_model_io::HOUSE_CONTEXT_SOURCE_KIND,
        crate::installed_std::test_local_model_io::HOUSE_TEXT_SINK_KIND,
    );
    let checked =
        check_syntax_document(&parse_syntax_document(&source), &startup).map_err(|error| {
            format!(
                "House live proof Plot check: {} {}",
                error.code, error.message
            )
        })?;
    let expanded = conduit_plot::expand_canonical_plot(&checked, "house-live-proof", &profiles)
        .map_err(|error| {
            format!(
                "House live proof expansion: {} {}",
                error.code, error.message
            )
        })?;
    run_expanded(host, expanded, "House", 4_096)
}
