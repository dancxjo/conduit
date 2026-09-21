//! Exact handoff from one generated Speech manifestation to Piper audio.

use conduit_presentation::{
    GeneratedContentRole, GeneratedManifestation, GeneratedManifestationDisposition,
    GenerativePresenterRequest,
};

use super::{PiperFailure, PiperSpeechAdapter, PiperSynthesisReceipt};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GeneratedSpeechRefusal {
    InvalidManifestation,
    TerminalManifestation,
    SpeechCardinality,
    InvalidSpeech,
    Synthesis(PiperFailure),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedSpeechReceipt {
    pub source_presentation_identity: String,
    pub source_presentation_revision: u64,
    pub generated_manifestation_identity: String,
    pub generation_run_identity: String,
    pub presenter_implementation_identity: String,
    pub presenter_provider_identity: String,
    pub presenter_model_identity: String,
    pub speech_sha256: String,
    pub voice_executable_sha256: String,
    pub voice_model_sha256: String,
    pub voice_config_sha256: String,
    pub synthesis: PiperSynthesisReceipt,
}

impl PiperSpeechAdapter {
    /// Synthesize the one public Speech segment from an exact, revalidated
    /// generated Manifestation. Presented thought is deliberately not voiced.
    pub fn synthesize_generated_speech(
        &mut self,
        request: &GenerativePresenterRequest,
        manifestation: &GeneratedManifestation,
        cancelled: impl FnMut() -> bool,
        consume: impl FnMut(&[u8]) -> Result<(), ()>,
    ) -> Result<GeneratedSpeechReceipt, GeneratedSpeechRefusal> {
        request
            .validate_manifestation(manifestation)
            .map_err(|_| GeneratedSpeechRefusal::InvalidManifestation)?;
        if !matches!(
            manifestation.disposition,
            GeneratedManifestationDisposition::Produced
                | GeneratedManifestationDisposition::Truncated
        ) {
            return Err(GeneratedSpeechRefusal::TerminalManifestation);
        }
        let mut speech_segments = manifestation
            .content
            .iter()
            .filter(|segment| segment.role == GeneratedContentRole::Speech);
        let speech = speech_segments
            .next()
            .ok_or(GeneratedSpeechRefusal::SpeechCardinality)?;
        if speech_segments.next().is_some() {
            return Err(GeneratedSpeechRefusal::SpeechCardinality);
        }
        let speech = core::str::from_utf8(&speech.bytes)
            .map_err(|_| GeneratedSpeechRefusal::InvalidSpeech)?;
        let synthesis = self
            .synthesize(speech, cancelled, consume)
            .map_err(GeneratedSpeechRefusal::Synthesis)?;
        if synthesis.text_sha256.is_empty() {
            return Err(GeneratedSpeechRefusal::InvalidSpeech);
        }
        Ok(GeneratedSpeechReceipt {
            source_presentation_identity: manifestation.source_presentation_identity.clone(),
            source_presentation_revision: manifestation.source_presentation_revision,
            generated_manifestation_identity: manifestation.manifestation_identity.clone(),
            generation_run_identity: manifestation.generation_run_identity.clone(),
            presenter_implementation_identity: manifestation
                .presenter_implementation_identity
                .clone(),
            presenter_provider_identity: manifestation.provider_identity.clone(),
            presenter_model_identity: manifestation.model_identity.clone(),
            speech_sha256: synthesis.text_sha256.clone(),
            voice_executable_sha256: self.discovery().executable_sha256.clone(),
            voice_model_sha256: self.discovery().model_sha256.clone(),
            voice_config_sha256: self.discovery().config_sha256.clone(),
            synthesis,
        })
    }
}
