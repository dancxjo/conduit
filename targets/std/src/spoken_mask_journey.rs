//! Producer-owned observations from the canonical screen-free Mask journey.
//!
//! These records describe semantic, planning, and output-effect truth. A spoken
//! artifact receipt is deliberately not evidence that a person heard the Show
//! or that room audio was captured and correlated back through the Mask.

use serde::{Deserialize, Serialize};

mod routes;
pub use routes::*;

/// One exact ordinary spoken Mask execution retained for producer evidence.
#[derive(Debug, Clone)]
pub struct SpokenMaskExecution {
    pub shown: conduit_presentation::ArtifactAcknowledgedSpokenShow,
    pub plan: conduit_core::Plan,
    pub mask: conduit_presentation::MaskForm,
}

/// Execute the complete spoken Mask through planning, kernel Host Calls,
/// deterministic speech synthesis, and an acknowledged WAV artifact. The
/// supplied Manifestation is a result captured from the producer's preceding
/// live Presenter call; the adapter only correlates it to this exact request.
/// This proves an artifact effect, not playback or hearing.
pub fn execute_retained_manifestation_mask(
    form_name: &str,
    execution_id: &str,
    presentation: conduit_presentation::Presentation,
    retained: conduit_presentation::GeneratedManifestationCandidate,
) -> Result<SpokenMaskExecution, String> {
    use conduit_core::{
        BaseImplementationId, BootId, ConnectionTrack, HostId, OfferGeneration, PortDirection,
        SignId,
    };
    use conduit_form::{
        check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
        ProfileCatalog, StartupCatalog,
    };
    use conduit_planner::{
        default_expanded_placements, plan_expanded_authoring_with_options, ConnectionQueueLimits,
        ForeBoundaryKey, PlanningOptions,
    };
    use conduit_presentation::{
        GenerativeNarratorRole, GenerativePresenterBounds, GenerativePresenterPolicy,
        GenerativePresenterRequest, ManifestationLifecycle, MaskForm, PlannedMaskForm,
    };
    use std::collections::BTreeMap;

    struct Replay {
        offer: conduit_ai::LocalModelOffer,
        retained: conduit_presentation::GeneratedManifestationCandidate,
    }
    impl crate::hosted_local_model::HostedLocalModelAdapter for Replay {
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
        ) -> crate::hosted_local_model::LocalModelAdapterTerminal {
            use crate::hosted_local_model::LocalModelAdapterTerminal;
            if placement.kind_id.as_str() != conduit_ai::LLM_PRESENT_KIND {
                return LocalModelAdapterTerminal::Refused;
            }
            let Ok(request) = serde_json::from_slice::<GenerativePresenterRequest>(input) else {
                return LocalModelAdapterTerminal::Failed;
            };
            let mut manifestation = self.retained.clone();
            manifestation.request_identity = request.request_identity;
            manifestation.source_presentation_identity =
                request.semantic_data.source_presentation_identity;
            manifestation.source_presentation_revision =
                request.semantic_data.source_presentation_revision;
            manifestation.template_contract_revision = request.policy.template_contract_revision;
            manifestation.candidate_identity = manifestation.digest();
            match serde_json::to_vec(&manifestation) {
                Ok(bytes) => {
                    output.clear();
                    output.extend(bytes);
                    LocalModelAdapterTerminal::Produced
                }
                Err(_) => LocalModelAdapterTerminal::Failed,
            }
        }
    }
    fn offer(
        retained: &conduit_presentation::GeneratedManifestationCandidate,
    ) -> conduit_ai::LocalModelOffer {
        use conduit_ai::{
            LlmDeterminismProfile, LlmWorkBounds, LocalModelCachePolicy, LocalModelComputeNeed,
            LocalModelIdentity, LocalModelKindProfile, LocalModelLifecycleState, LocalModelLimits,
            LocalModelOffer,
        };
        LocalModelOffer {
            identity: LocalModelIdentity {
                runtime_name: retained.provider_identity.clone(),
                runtime_version: "retained-producer-result".into(),
                runtime_build_identity: retained.presenter_implementation_identity.clone(),
                model_name: retained.model_identity.clone(),
                model_content_identity: retained.generation_run_identity.clone(),
                architecture: "retained-manifestation".into(),
                parameter_profile: "exact-result".into(),
                quantization: "producer-owned".into(),
            },
            limits: LocalModelLimits {
                work: LlmWorkBounds::reviewed_default(),
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
                maximum_queue_bytes: 262_144,
                cancellation_supported: true,
                cache_policy: LocalModelCachePolicy::OneLoadedModelUntilShutdown,
            },
            supported_profiles: vec![LocalModelKindProfile::PresentSemanticFront],
            initialized: true,
            lifecycle: LocalModelLifecycleState::Ready,
            determinism: LlmDeterminismProfile::ProviderNondeterministic,
        }
    }
    #[derive(Default)]
    struct Collector(Vec<crate::ExternalForeDelivery>);
    impl crate::ExternalForeOutputAdapter for Collector {
        fn deliver(&mut self, output: crate::ExternalForeDelivery) -> Result<(), String> {
            self.0.push(output);
            Ok(())
        }
    }
    struct NoopTimer;
    impl crate::TimerAdapter for NoopTimer {
        fn wait(&mut self, _: std::time::Duration) {}
    }

    let safe_id = execution_id.replace('/', "-");
    let config = crate::StdHostConfig {
        host_id: HostId::from(format!("host/{execution_id}")),
        boot_id: BootId::from(format!("boot/{execution_id}")),
        offer_generation: OfferGeneration(1),
    };
    let mut host = crate::StdHost::new_with_local_model(
        config.clone(),
        crate::StdHostComposition::minimal(),
        Box::new(Replay {
            offer: offer(&retained),
            retained,
        }),
    )?;
    let destination = std::env::temp_dir().join(format!(
        "conduit-spoken-mask-{}-{safe_id}.wav",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&destination);
    host.attach_deterministic_speech_and_wav_artifact(
        crate::hosted_wav_artifact::WavArtifactSelection::new(
            &destination,
            config.boot_id.clone(),
            config.offer_generation,
        )?,
    )?;
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    conduit_presentation::install_mask_form_value_aliases(&mut startup)?;
    conduit_presentation::install_spoken_mask_catalog(&mut startup, &mut profiles)?;
    conduit_ai::install_llm_semantic_catalog(&mut startup, &mut profiles)?;
    conduit_tongues::install_speech_synthesis_catalog(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_sound_catalogs(&mut startup, &mut profiles)?;
    let source = format!(
        r#"form {form_name} (
 >> face: Presentation
 interaction: FaceInteraction...| >>
 show: Show >>
) {{
 request: presentation/adapt-generative-request
 language: llm/present(16384, 1, 4096, 16384, 0)
 envelope: presentation/build-generated-validation-envelope
 validator: presentation/generated-semantic-validator
 accepted: presentation/retain-generated-validation
 speech: presentation/generated-manifestation-speech
 voice: speech/synthesize(maximum-output-bytes = 32768)
 convert: audio/convert-pcm-profile(output-sample-rate-hz = 48000, output-channel-layout = "stereo-left-right")
 artifact: presentation/spoken-artifact
 shown: presentation/artifact-acknowledged-show
 no-input: presentation/no-interaction
 face >> request.presentation
 request.request >> language.request
 request.request >> envelope.request
 language.result >> envelope.candidate
 language.result >> accepted.candidate
 envelope.envelope >> validator.envelope
 validator.assessment >> accepted.assessment
 accepted.manifestation >> speech.manifestation
 accepted.manifestation >> shown.manifestation
 speech.speech >> voice.text
 voice.audio >> convert.audio
 convert.converted >> artifact.audio
 artifact.receipt >> shown.artifact
 shown.show >> show
 no-input.interaction >> interaction
}}
"#
    );
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup)
        .map_err(|error| format!("check spoken Mask: {error:?}"))?;
    let authoring = expand_canonical_form_for_authoring(&checked, form_name, &profiles)
        .map_err(|error| format!("expand spoken Mask: {error:?}"))?;
    let mask = MaskForm::admit(&authoring).map_err(|error| format!("admit Mask: {error:?}"))?;
    let hosts = [host.advertisement().clone()];
    let placements = default_expanded_placements(&authoring.expanded, &hosts)
        .map_err(|error| format!("place spoken Mask: {error:?}"))?;
    let boundary_limits = authoring
        .front
        .inputs()
        .iter()
        .map(|port| (PortDirection::Input, port))
        .chain(
            authoring
                .front
                .outputs()
                .iter()
                .map(|port| (PortDirection::Output, port)),
        )
        .map(|(direction, port)| {
            (
                ForeBoundaryKey {
                    direction,
                    front_port_id: port.port_id.clone(),
                    track: ConnectionTrack::Payload,
                },
                ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: 524_288,
                },
            )
        })
        .collect();
    let connection_bases = BTreeMap::new();
    let line_candidates = BTreeMap::new();
    let grant_id = format!("grant/{execution_id}");
    let authority = host.spoken_mask_artifact_authority_grant(&grant_id)?;
    let plan = plan_expanded_authoring_with_options(
        &authoring,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        PlanningOptions {
            connection_bases: &connection_bases,
            line_candidates: &line_candidates,
            connection_item_capacity: 1,
            connection_byte_capacity: 16_384,
            authority_grants: std::slice::from_ref(&authority),
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundary_limits,
    )
    .map_err(|error| format!("plan spoken Mask: {error:?}"))?;
    let planned = PlannedMaskForm::admit(&mask, &plan)
        .map_err(|error| format!("seal spoken Mask: {error:?}"))?;
    let request = GenerativePresenterRequest::from_presentation(
        format!("request/{execution_id}"),
        GenerativePresenterPolicy {
            template_contract_revision: "template/spoken-mask@1".into(),
            narrator_role: GenerativeNarratorRole::TransientFirstPersonBodyNarrator,
            instructions: "Speak one truthful sentence from the supplied Presentation.".into(),
        },
        presentation.clone(),
        None,
        GenerativePresenterBounds::reviewed_default(),
    )
    .map_err(|error| format!("prepare Presenter request: {error:?}"))?;
    let front_subject = presentation
        .subjects
        .iter()
        .find(|subject| subject.role == conduit_presentation::PresentationRole::Body)
        .map(|subject| subject.identity.clone())
        .ok_or("spoken Mask Face has no Body subject")?;
    let preparation = crate::spoken_mask_runtime::SpokenMaskPreparation {
        request,
        presentation: presentation.clone(),
        planned_mask: planned,
        front_subject,
        target_subject: format!("artifact/{execution_id}"),
        prepared_sign: SignId::from(format!("sign/{execution_id}/prepared")),
        available_sign: SignId::from(format!("sign/{execution_id}/available")),
    };
    let mut collector = Collector::default();
    host.run_spoken_mask_form_to(
        plan.fragments[0].clone(),
        preparation,
        &[crate::ExternalForeInput {
            front_port_id: conduit_core::port_id("face"),
            track: ConnectionTrack::Payload,
            bytes: serde_json::to_vec(&presentation).map_err(|error| error.to_string())?,
        }],
        &mut collector,
        &mut Vec::new(),
        &mut NoopTimer,
    )
    .map_err(|error| format!("execute spoken Mask: {error:?}"))?;
    let delivery = collector
        .0
        .into_iter()
        .next()
        .ok_or("spoken Mask emitted no Show")?;
    let shown: conduit_presentation::ArtifactAcknowledgedSpokenShow =
        serde_json::from_slice(&delivery.bytes).map_err(|error| error.to_string())?;
    if shown.show.show.lifecycle != ManifestationLifecycle::Available || !destination.is_file() {
        return Err("spoken Mask did not retain an available artifact Show".into());
    }
    let _ = std::fs::remove_file(destination);
    Ok(SpokenMaskExecution { shown, plan, mask })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpokenMaskJourneyObservation {
    pub action_id: String,
    pub concrete_event: String,
    pub presentation_id: String,
    pub selected_mask_form_id: Option<String>,
    pub plan_id: String,
    pub selected_route_id: Option<String>,
    pub show_id: Option<String>,
    pub receipt_ids: Vec<String>,
}

impl SpokenMaskJourneyObservation {
    pub fn action(&self) -> Option<conduit_presentation::MaskJourneyAction> {
        conduit_presentation::MASK_JOURNEY_ACTIONS
            .into_iter()
            .find(|action| action.id() == self.action_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpokenMaskJourneyEvidence {
    pub schema: String,
    pub observations: Vec<SpokenMaskJourneyObservation>,
    /// Always false for artifact-only proof. Human hearing requires separate
    /// attended evidence; physical re-observation requires capture + correlation.
    pub human_hearing_observed: bool,
    pub inward_face_interaction_observed: bool,
}

impl SpokenMaskJourneyEvidence {
    pub const SCHEMA: &'static str = "conduit.std/spoken-mask-journey@1";

    pub fn validate(&self) -> Result<(), &'static str> {
        if self.schema != Self::SCHEMA {
            return Err("unknown spoken Mask journey evidence schema");
        }
        if self.observations.len() != conduit_presentation::MASK_JOURNEY_ACTIONS.len() {
            return Err("spoken Mask journey does not contain ten observations");
        }
        for (observation, action) in self
            .observations
            .iter()
            .zip(conduit_presentation::MASK_JOURNEY_ACTIONS)
        {
            if observation.action_id != action.id() {
                return Err("spoken Mask journey observations are out of order");
            }
        }
        let Some(presentation) = self.observations.first().map(|item| &item.presentation_id) else {
            return Err("spoken Mask journey omitted its Presentation");
        };
        if self
            .observations
            .iter()
            .any(|item| &item.presentation_id != presentation)
        {
            return Err("spoken Mask journey changed Presentation identity");
        }
        if self.human_hearing_observed || self.inward_face_interaction_observed {
            return Err("artifact-only spoken Mask proof claimed sensory observation");
        }
        Ok(())
    }
}
