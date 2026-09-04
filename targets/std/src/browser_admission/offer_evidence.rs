use conduit_body::{HostOfferProjection, OfferDisclosureStage, MAX_DISCLOSED_CAPABILITIES};
use conduit_core::PROTOCOL_VERSION;

use super::BrowserAdmissionFrameError;

pub(super) fn validate(evidence: &HostOfferProjection) -> Result<(), BrowserAdmissionFrameError> {
    if evidence.stage != OfferDisclosureStage::AdmittedMembership
        || evidence.protocol_version != PROTOCOL_VERSION
        || evidence.host_id.as_str().is_empty()
        || evidence.boot_id.as_str().is_empty()
        || evidence.observation_sign_id.as_str().is_empty()
        || evidence.freshness_sequence == 0
        || evidence.profile.is_none()
        || evidence.capability_summary.len() > MAX_DISCLOSED_CAPABILITIES
        || !evidence.capabilities.is_empty()
        || !evidence.resources.is_empty()
        || evidence.capability_summary.iter().any(|summary| {
            summary.capability_id.as_str().is_empty()
                || summary.implementation_id.as_str().is_empty()
        })
        || evidence
            .capability_summary
            .windows(2)
            .any(|pair| pair[0].capability_id >= pair[1].capability_id)
    {
        Err(BrowserAdmissionFrameError::InvalidOfferEvidence)
    } else {
        Ok(())
    }
}
