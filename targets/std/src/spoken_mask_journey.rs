//! Producer-owned observations from the canonical screen-free Mask journey.
//!
//! These records describe semantic, planning, and output-effect truth. A spoken
//! artifact receipt is deliberately not evidence that a person heard the Show
//! or that room audio was captured and correlated back through the Mask.

use serde::{Deserialize, Serialize};

#[cfg(all(test, unix))]
mod cancellation_tests;
mod graph;
mod replay;
mod retained_artifact;
mod routes;
mod run;
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
    language: &conduit_language::LanguageRequest,
) -> Result<SpokenMaskExecution, String> {
    execute_mask(
        plot_name,
        execution_id,
        presentation,
        retained,
        None,
        false,
        language,
    )
    .map(|result| result.0)
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
    language: &conduit_language::LanguageRequest,
) -> Result<RetainedSpokenMaskExecution, String> {
    let (execution, artifact) = execute_mask(
        plot_name,
        execution_id,
        presentation,
        retained,
        Some((discovery, destination)),
        false,
        language,
    )?;
    Ok(RetainedSpokenMaskExecution {
        execution,
        artifact: artifact.ok_or("real speech omitted retained artifact metadata")?,
    })
}

/// Stream committed segments from an already validated Presenter candidate into
/// one acknowledged ordinary Mask Show. The original candidate is revalidated
/// against this exact Face; it is not an unvalidated model-token stream.
pub fn execute_retained_manifestation_mask_with_streaming_espeak(
    plot_name: &str,
    execution_id: &str,
    presentation: conduit_presentation::Presentation,
    retained: conduit_presentation::GeneratedManifestationCandidate,
    discovery: crate::hosted_speech_synthesis::EspeakDiscovery,
    destination: &std::path::Path,
    language: &conduit_language::LanguageRequest,
) -> Result<RetainedSpokenMaskExecution, String> {
    let (execution, artifact) = execute_mask(
        plot_name,
        execution_id,
        presentation,
        retained,
        Some((discovery, destination)),
        true,
        language,
    )?;
    Ok(RetainedSpokenMaskExecution {
        execution,
        artifact: artifact.ok_or("streamed Mask omitted acknowledged WAV")?,
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
    streaming: bool,
    language: &conduit_language::LanguageRequest,
) -> Result<(SpokenMaskExecution, Option<RetainedWavArtifact>), String> {
    let run::MaskRun {
        report,
        deliveries,
        plan,
        mask,
        destination,
        real_speech,
    } = run::run_mask(
        plot_name,
        execution_id,
        presentation,
        retained,
        run::SpeechPreparation {
            real,
            streaming,
            language,
        },
        &crate::RunControl::default(),
    )?;
    let _terminal_report = report;
    let delivery = deliveries
        .into_iter()
        .next()
        .ok_or("spoken Mask emitted no Show")?;
    let shown: conduit_presentation::ArtifactAcknowledgedSpokenShow =
        serde_json::from_slice(&delivery.bytes).map_err(|error| error.to_string())?;
    if shown.show.show.lifecycle != conduit_presentation::ManifestationLifecycle::Available
        || !destination.is_file()
    {
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
