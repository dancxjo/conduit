use conduit_body::{
    HostOfferProjection, OfferDisclosureStage, MAX_DISCLOSED_CAPABILITIES, MAX_DISCLOSED_RESOURCES,
};
use conduit_core::PROTOCOL_VERSION;

use super::BrowserAdmissionFrameError;

pub(super) fn validate(evidence: &HostOfferProjection) -> Result<(), BrowserAdmissionFrameError> {
    let invalid_summary = evidence.stage == OfferDisclosureStage::AdmittedMembership
        && (!evidence.capabilities.is_empty() || !evidence.resources.is_empty());
    let invalid_detail = evidence.stage == OfferDisclosureStage::Planning
        && (!evidence.capability_summary.is_empty()
            || evidence.capabilities.len() > MAX_DISCLOSED_CAPABILITIES
            || evidence.resources.len() > MAX_DISCLOSED_RESOURCES
            || (evidence.capabilities.is_empty() && evidence.resources.is_empty())
            || evidence
                .capabilities
                .windows(2)
                .any(|pair| pair[0].capability_id >= pair[1].capability_id)
            || evidence
                .resources
                .windows(2)
                .any(|pair| pair[0].pool_id >= pair[1].pool_id));
    if !matches!(
        evidence.stage,
        OfferDisclosureStage::AdmittedMembership | OfferDisclosureStage::Planning
    ) || evidence.protocol_version != PROTOCOL_VERSION
        || evidence.host_id.as_str().is_empty()
        || evidence.boot_id.as_str().is_empty()
        || evidence.observation_sign_id.as_str().is_empty()
        || evidence.freshness_sequence == 0
        || evidence.profile.is_none()
        || evidence.capability_summary.len() > MAX_DISCLOSED_CAPABILITIES
        || invalid_summary
        || invalid_detail
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
