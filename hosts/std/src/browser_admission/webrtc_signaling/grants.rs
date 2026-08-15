//! Atomic admission of Body-owned planned session grants.

use conduit_wire::{encode_session_frame_into, SessionBinding};

use super::{
    BrowserWebRtcRendezvous, BrowserWebRtcRendezvousRefusal, GrantedSession,
    MAX_WEBRTC_NEGOTIATIONS, MAX_WEBRTC_SESSION_HELLO_BYTES,
};

impl BrowserWebRtcRendezvous {
    pub fn deactivate_grants(&mut self) -> Vec<conduit_core::LinkBindingId> {
        self.grants.clear();
        self.negotiations
            .drain(..)
            .map(|negotiation| negotiation.negotiation_id)
            .collect()
    }

    pub fn replace_grants<'a>(
        &mut self,
        bindings: impl IntoIterator<Item = &'a SessionBinding>,
    ) -> Result<Vec<Vec<u8>>, BrowserWebRtcRendezvousRefusal> {
        if !self.negotiations.is_empty() {
            return Err(BrowserWebRtcRendezvousRefusal::InvalidStage);
        }
        let mut replacement = Self::default();
        let mut hellos = Vec::with_capacity(MAX_WEBRTC_NEGOTIATIONS);
        for binding in bindings {
            hellos.push(replacement.grant(binding)?);
        }
        self.grants = replacement.grants;
        Ok(hellos)
    }

    pub fn preflight_grants<'a>(
        &self,
        bindings: impl IntoIterator<Item = &'a SessionBinding>,
    ) -> Result<Vec<Vec<u8>>, BrowserWebRtcRendezvousRefusal> {
        let mut candidate = self.clone();
        candidate.replace_grants(bindings)
    }

    pub fn grant(
        &mut self,
        binding: &SessionBinding,
    ) -> Result<Vec<u8>, BrowserWebRtcRendezvousRefusal> {
        binding
            .validate()
            .map_err(|_| BrowserWebRtcRendezvousRefusal::SessionMismatch)?;
        let negotiation_id = binding.attachment.link_binding_id.clone();
        if self
            .grants
            .iter()
            .any(|grant| grant.negotiation_id == negotiation_id)
        {
            return Err(BrowserWebRtcRendezvousRefusal::DuplicateGrant);
        }
        if self.grants.len() == MAX_WEBRTC_NEGOTIATIONS {
            return Err(BrowserWebRtcRendezvousRefusal::CapacityExhausted);
        }
        let mut encoded = [0; MAX_WEBRTC_SESSION_HELLO_BYTES];
        let length = encode_session_frame_into(
            binding.hello_frame(),
            &mut encoded,
            binding.limits.maximum_payload_bytes,
            MAX_WEBRTC_SESSION_HELLO_BYTES as u32,
        )
        .map_err(|_| BrowserWebRtcRendezvousRefusal::SessionMismatch)?;
        let session_hello = encoded[..length].to_vec();
        self.grants.push(GrantedSession {
            negotiation_id,
            session_hello: session_hello.clone(),
        });
        Ok(session_hello)
    }
}
