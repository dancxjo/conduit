//! One semantic validator shared by allocation-backed and borrowed descriptors.

use crate::{
    RendezvousCandidate, RendezvousDescriptorRefusal, RendezvousLineFamily,
    MAX_RENDEZVOUS_ATTEMPTS_PER_CANDIDATE, MAX_RENDEZVOUS_ATTEMPT_MILLIS,
    MAX_RENDEZVOUS_CANDIDATES, MAX_RENDEZVOUS_TEXT_BYTES,
};

#[derive(Clone, Copy)]
pub(crate) struct RendezvousCandidateView<'a> {
    pub candidate_id: &'a str,
    pub line_family: RendezvousLineFamily,
    pub reachability: &'a str,
    pub server_identity: &'a str,
    pub transport_binding_sha256: [u8; 32],
    pub expires_at_millis: u64,
    pub maximum_attempts: u8,
    pub attempt_timeout_millis: u32,
}

impl<'a> From<&'a RendezvousCandidate> for RendezvousCandidateView<'a> {
    fn from(candidate: &'a RendezvousCandidate) -> Self {
        Self {
            candidate_id: &candidate.candidate_id,
            line_family: candidate.line_family,
            reachability: &candidate.reachability,
            server_identity: &candidate.authentication.server_identity,
            transport_binding_sha256: candidate.authentication.transport_binding_sha256,
            expires_at_millis: candidate.expires_at_millis,
            maximum_attempts: candidate.maximum_attempts,
            attempt_timeout_millis: candidate.attempt_timeout_millis,
        }
    }
}

pub(crate) fn validate_owned_candidates(
    candidates: &[RendezvousCandidate],
    now_millis: u64,
) -> Result<(), RendezvousDescriptorRefusal> {
    validate_candidate_views(
        candidates.iter().map(RendezvousCandidateView::from),
        now_millis,
    )
}

pub(crate) fn validate_candidate_views<'a>(
    candidates: impl ExactSizeIterator<Item = RendezvousCandidateView<'a>> + Clone,
    now_millis: u64,
) -> Result<(), RendezvousDescriptorRefusal> {
    if candidates.len() == 0 || candidates.len() > MAX_RENDEZVOUS_CANDIDATES {
        return Err(RendezvousDescriptorRefusal::CandidateBound);
    }
    for (index, candidate) in candidates.clone().enumerate() {
        if !bounded_text(candidate.candidate_id)
            || candidates
                .clone()
                .take(index)
                .any(|prior| prior.candidate_id == candidate.candidate_id)
        {
            return Err(RendezvousDescriptorRefusal::DuplicateCandidate);
        }
        if !bounded_text(candidate.reachability) {
            return Err(RendezvousDescriptorRefusal::InvalidReachability);
        }
        if !bounded_text(candidate.server_identity) || candidate.transport_binding_sha256 == [0; 32]
        {
            return Err(RendezvousDescriptorRefusal::MissingAuthentication);
        }
        if candidate.expires_at_millis <= now_millis {
            return Err(RendezvousDescriptorRefusal::Expired);
        }
        if candidate.maximum_attempts == 0
            || candidate.maximum_attempts > MAX_RENDEZVOUS_ATTEMPTS_PER_CANDIDATE
            || candidate.attempt_timeout_millis == 0
            || candidate.attempt_timeout_millis > MAX_RENDEZVOUS_ATTEMPT_MILLIS
        {
            return Err(RendezvousDescriptorRefusal::AttemptPolicy);
        }
        if candidate.line_family == RendezvousLineFamily::LocalLoopbackWebSocket
            && !loopback_reachability(candidate.reachability)
        {
            return Err(RendezvousDescriptorRefusal::InsecureRemoteWebSocket);
        }
        if candidate.line_family == RendezvousLineFamily::WebRtcDataChannel
            && !candidate.reachability.starts_with("webrtc-bootstrap:")
        {
            return Err(RendezvousDescriptorRefusal::InvalidReachability);
        }
    }
    Ok(())
}

fn bounded_text(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAX_RENDEZVOUS_TEXT_BYTES
}

fn loopback_reachability(value: &str) -> bool {
    value.starts_with("ws://127.0.0.1:") || value.starts_with("ws://[::1]:")
}
