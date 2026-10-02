//! Producer-owned observations from the canonical screen-free Mask journey.
//!
//! These records describe semantic, planning, and output-effect truth. A spoken
//! artifact receipt is deliberately not evidence that a person heard the Show
//! or that room audio was captured and correlated back through the Mask.

use serde::{Deserialize, Serialize};

mod replay;
mod retained_artifact;
mod routes;
#[cfg(test)]
mod tests;
use replay::{offer, Replay};
pub use retained_artifact::{RetainedSpokenMaskExecution, RetainedWavArtifact};
pub use routes::*;

/// One exact ordinary spoken Mask execution retained for producer evidence.
#[derive(Debug, Clone)]
pub struct SpokenMaskExecution {
    pub shown: conduit_presentation::ArtifactAcknowledgedSpokenShow,
    pub plan: conduit_core::Plan,
    pub mask: conduit_presentation::MaskPlot,
}

/// Execute the complete spoken Mask through planning, kernel Host Calls,
/// deterministic speech synthesis, and an acknowledged WAV artifact. The
/// supplied Manifestation is a result captured from the producer's preceding
/// live Presenter call; the adapter only correlates it to this exact request.
/// This proves an artifact effect, not playback or hearing.
pub fn execute_retained_manifestation_mask(
    plot_name: &str,
    execution_id: &str,
    presentation: conduit_presentation::Presentation,
    retained: conduit_presentation::GeneratedManifestationCandidate,
) -> Result<SpokenMaskExecution, String> {
    execute_mask(plot_name, execution_id, presentation, retained, None).map(|result| result.0)
}

/// Run a retained Presenter result through real, explicitly selected eSpeak and
/// retain its acknowledged WAV. This does not run Presenter inference or prove
/// playback, human hearing, or inward audio capture.
pub fn execute_retained_manifestation_mask_with_espeak(
    plot_name: &str,
    execution_id: &str,
    presentation: conduit_presentation::Presentation,
    retained: conduit_presentation::GeneratedManifestationCandidate,
    discovery: crate::hosted_speech_synthesis::EspeakDiscovery,
    destination: &std::path::Path,
) -> Result<RetainedSpokenMaskExecution, String> {
    let (execution, artifact) = execute_mask(
        plot_name,
        execution_id,
        presentation,
        retained,
        Some((discovery, destination)),
    )?;
    Ok(RetainedSpokenMaskExecution {
        execution,
        artifact: artifact.ok_or("real speech omitted retained artifact metadata")?,
    })
}

fn execute_mask(
    plot_name: &str,
    execution_id: &str,
    presentation: conduit_presentation::Presentation,
    retained: conduit_presentation::GeneratedManifestationCandidate,
    real: Option<(
        crate::hosted_speech_synthesis::EspeakDiscovery,
        &std::path::Path,
    )>,
) -> Result<(SpokenMaskExecution, Option<RetainedWavArtifact>), String> {
    use conduit_core::{
        BaseImplementationId, BootId, ConnectionTrack, HostId, OfferGeneration, PortDirection,
        SignId,
    };
    use conduit_planner::{
        default_expanded_placements, plan_expanded_authoring_with_options, ConnectionQueueLimits,
        ForeBoundaryKey, PlanningOptions,
    };
    use conduit_plot::{
        check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
        ProfileCatalog, StartupCatalog,
    };
    use conduit_presentation::{
        GenerativeNarratorRole, GenerativePresenterBounds, GenerativePresenterPolicy,
        GenerativePresenterRequest, ManifestationLifecycle, MaskPlot, PlannedMaskPlot,
    };
    use std::collections::BTreeMap;

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
    let real_speech = real.is_some();
    let destination = real
        .as_ref()
        .map(|(_, path)| path.to_path_buf())
        .unwrap_or_else(|| {
            std::env::temp_dir().join(format!(
                "conduit-spoken-mask-{}-{safe_id}.wav",
                std::process::id()
            ))
        });
    if real_speech && destination.exists() {
        return Err("retained WAV destination already exists".into());
    }
    if !real_speech {
        let _ = std::fs::remove_file(&destination);
    }
    let artifact = crate::hosted_wav_artifact::WavArtifactSelection::new(
        &destination,
        config.boot_id.clone(),
        config.offer_generation,
    )?;
    if let Some((discovery, _)) = real {
        let adapter = discovery
            .initialize(
                config.host_id.clone(),
                config.boot_id.clone(),
                config.offer_generation,
                conduit_core::AuthorityGrantId::from(format!("grant/{execution_id}/speech")),
                std::time::Duration::from_secs(10),
            )
            .map_err(|error| error.to_string())?;
        host.attach_espeak_speech_and_wav_artifact(adapter, artifact)?;
    } else {
        host.attach_deterministic_speech_and_wav_artifact(artifact)?;
    }
    let maximum_output_bytes = if real_speech { 131_072 } else { 32_768 };
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    conduit_presentation::install_mask_plot_value_aliases(&mut startup)?;
    conduit_presentation::install_spoken_mask_catalog(&mut startup, &mut profiles)?;
    conduit_ai::install_llm_semantic_catalog(&mut startup, &mut profiles)?;
    conduit_tongues::install_speech_synthesis_catalog(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_sound_catalogs(&mut startup, &mut profiles)?;
    let source = format!(
        r#"plot {plot_name} (
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
 voice: speech/synthesize(maximum-output-bytes = {maximum_output_bytes})
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
    let authoring = expand_canonical_plot_for_authoring(&checked, plot_name, &profiles)
        .map_err(|error| format!("expand spoken Mask: {error:?}"))?;
    let mask = MaskPlot::admit(&authoring).map_err(|error| format!("admit Mask: {error:?}"))?;
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
    let mut authority = vec![host.spoken_mask_artifact_authority_grant(&grant_id)?];
    if real_speech {
        authority.push(host.speech_synthesis_authority_grant()?);
    }
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
            authority_grants: &authority,
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundary_limits,
    )
    .map_err(|error| format!("plan spoken Mask: {error:?}"))?;
    let planned = PlannedMaskPlot::admit(&mask, &plan)
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
    host.run_spoken_mask_plot_to(
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
    let artifact = if real_speech {
        Some(retained_artifact::inspect(&destination, &shown.artifact)?)
    } else {
        let _ = std::fs::remove_file(destination);
        None
    };
    Ok((SpokenMaskExecution { shown, plan, mask }, artifact))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpokenMaskJourneyObservation {
    pub action_id: String,
    pub concrete_event: String,
    pub presentation_id: String,
    pub selected_mask_plot_id: Option<String>,
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
