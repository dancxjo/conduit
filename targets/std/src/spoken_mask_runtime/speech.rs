//! Speech is projected only from the exact semantically accepted manifestation.
use super::*;

impl SpokenMaskSemanticSession {
    pub fn validate_and_extract_speech(&self, encoded: &[u8]) -> Result<Vec<u8>, String> {
        self.validate_and_extract_speech_bounded(encoded, conduit_tongues::MAXIMUM_TEXT_BYTES)
    }

    /// The bound is the exact selected projection Host Call output allowance.
    pub fn validate_and_extract_speech_bounded(
        &self,
        encoded: &[u8],
        maximum: u32,
    ) -> Result<Vec<u8>, String> {
        if ![conduit_tongues::MAXIMUM_TEXT_BYTES, 1024].contains(&maximum) {
            return Err("unsupported selected speech projection bound".into());
        }
        if encoded.len() > self.request.bounds.maximum_output_bytes as usize {
            return Err("generated manifestation exceeds its admitted bound".into());
        }
        let candidate: GeneratedManifestationCandidate = serde_json::from_slice(encoded)
            .map_err(|error| format!("decode accepted manifestation handle: {error}"))?;
        let generated = self
            .generated
            .as_ref()
            .ok_or_else(|| "speech extraction preceded semantic validation".to_string())?;
        if generated.candidate() != &candidate {
            return Err("speech extraction used a stale manifestation handle".into());
        }
        self.extract_accepted_speech_bounded(generated, maximum)
    }

    pub fn extract_accepted_speech(
        &self,
        generated: &GeneratedManifestation,
    ) -> Result<Vec<u8>, String> {
        self.extract_accepted_speech_bounded(generated, conduit_tongues::MAXIMUM_TEXT_BYTES)
    }

    fn extract_accepted_speech_bounded(
        &self,
        generated: &GeneratedManifestation,
        maximum: u32,
    ) -> Result<Vec<u8>, String> {
        self.request
            .validate_candidate(generated.candidate())
            .map_err(|error| format!("invalid accepted generated manifestation: {error:?}"))?;
        let mut speech_segments = generated
            .content()
            .iter()
            .filter(|segment| segment.role == conduit_presentation::GeneratedContentRole::Speech);
        let speech = speech_segments
            .next()
            .ok_or_else(|| "generated manifestation has no outward Speech".to_string())?;
        if speech_segments.next().is_some() {
            return Err("generated manifestation has more than one outward Speech".into());
        }
        core::str::from_utf8(&speech.bytes)
            .map_err(|_| "generated outward Speech is not UTF-8".to_string())?;
        if speech.bytes.len() > maximum as usize {
            return Err(
                "generated outward Speech exceeds the selected speech projection bound".into(),
            );
        }
        Ok(speech.bytes.clone())
    }
}
