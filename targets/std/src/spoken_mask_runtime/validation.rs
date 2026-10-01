//! Exact planned validation boundary for one spoken Mask session.

use super::SpokenMaskSemanticSession;
use conduit_presentation::{
    GeneratedManifestationCandidate, GeneratedValidationEnvelope, GeneratedValidationOutcome,
    GeneratedValidatorAssessment, GenerativePresenterRequest,
};

impl SpokenMaskSemanticSession {
    fn build_validation_envelope(
        &self,
        request: &[u8],
        candidate: &[u8],
    ) -> Result<Vec<u8>, String> {
        let request: GenerativePresenterRequest = serde_json::from_slice(request)
            .map_err(|error| format!("decode validation request: {error}"))?;
        let candidate: GeneratedManifestationCandidate = serde_json::from_slice(candidate)
            .map_err(|error| format!("decode generated candidate: {error}"))?;
        self.request
            .validate_candidate(&candidate)
            .map_err(|error| format!("invalid generated candidate: {error:?}"))?;
        if request != self.request {
            return Err("generated validation request is stale".into());
        }
        let encoded = serde_json::to_vec(&GeneratedValidationEnvelope { request, candidate })
            .map_err(|error| format!("encode generated validation envelope: {error}"))?;
        if encoded.len() > conduit_presentation::MAX_GENERATED_VALIDATION_ENVELOPE_BYTES {
            return Err("generated validation envelope exceeds its admitted bound".into());
        }
        Ok(encoded)
    }

    pub fn register_validation_request(&mut self, encoded: &[u8]) -> Result<(), String> {
        let request: GenerativePresenterRequest = serde_json::from_slice(encoded)
            .map_err(|error| format!("decode validation request: {error}"))?;
        if request != self.request || self.pending_validation_request.is_some() {
            return Err("generated validation request is stale or duplicated".into());
        }
        self.pending_validation_request = Some(encoded.to_vec());
        Ok(())
    }

    pub fn finish_validation_envelope(&mut self, candidate: &[u8]) -> Result<Vec<u8>, String> {
        let request = self
            .pending_validation_request
            .take()
            .ok_or_else(|| "generated candidate preceded its validation request".to_string())?;
        self.build_validation_envelope(&request, candidate)
    }

    pub fn assess_generated_envelope(&self, encoded: &[u8]) -> Result<Vec<u8>, String> {
        if encoded.len() > conduit_presentation::MAX_GENERATED_VALIDATION_ENVELOPE_BYTES {
            return Err("generated validation envelope exceeds its admitted bound".into());
        }
        let envelope: GeneratedValidationEnvelope = serde_json::from_slice(encoded)
            .map_err(|error| format!("decode generated validation envelope: {error}"))?;
        if envelope.request != self.request {
            return Err("generated validator received a stale request".into());
        }
        let validator = self
            .validator
            .as_ref()
            .ok_or_else(|| "generated validator session was already consumed".to_string())?;
        let assessment = validator.assess(
            &envelope,
            format!("assessment/{}", envelope.candidate.digest()),
        );
        serde_json::to_vec(&assessment)
            .map_err(|error| format!("encode generated validator assessment: {error}"))
    }

    pub fn register_generated_candidate(&mut self, encoded: &[u8]) -> Result<(), String> {
        let candidate: GeneratedManifestationCandidate = serde_json::from_slice(encoded)
            .map_err(|error| format!("decode generated candidate: {error}"))?;
        self.request
            .validate_candidate(&candidate)
            .map_err(|error| format!("invalid generated candidate: {error:?}"))?;
        if self.pending_candidate.replace(candidate).is_some() {
            return Err("spoken Mask registered more than one generated candidate".into());
        }
        Ok(())
    }

    pub fn retain_generated_assessment(&mut self, encoded: &[u8]) -> Result<Vec<u8>, String> {
        let assessment: GeneratedValidatorAssessment = serde_json::from_slice(encoded)
            .map_err(|error| format!("decode generated assessment: {error}"))?;
        let candidate = self
            .pending_candidate
            .take()
            .ok_or_else(|| "generated assessment preceded its candidate".to_string())?;
        let validator = self
            .validator
            .take()
            .ok_or_else(|| "generated validator session was already consumed".to_string())?;
        match validator
            .retain(
                &self.planned_mask.plan,
                &self.request,
                candidate.clone(),
                assessment,
            )
            .map_err(|error| format!("retain generated assessment: {error:?}"))?
        {
            GeneratedValidationOutcome::Accepted(generated) => {
                let encoded = serde_json::to_vec(&candidate)
                    .map_err(|error| format!("encode accepted manifestation handle: {error}"))?;
                self.register_accepted_manifestation(*generated)?;
                Ok(encoded)
            }
            GeneratedValidationOutcome::Terminal(receipt) => {
                self.terminal_validation = Some(*receipt);
                Err("generated semantic validation terminated without outward output".into())
            }
        }
    }
}
