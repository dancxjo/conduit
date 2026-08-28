//! Atomic admission of Body-owned planned session grants.

use conduit_wire::{encode_session_frame_into, SessionBinding};

use super::{
    BrowserWebRtcGrant, BrowserWebRtcRendezvous, BrowserWebRtcRendezvousRefusal, BrowserWebRtcRole,
    GrantedSession, MAX_WEBRTC_NEGOTIATIONS, MAX_WEBRTC_SESSION_HELLO_BYTES,
};

impl BrowserWebRtcRendezvous {
    pub fn grant_for_endpoint(
        &self,
        host_id: &conduit_core::HostId,
        boot_id: &conduit_core::BootId,
        index: u16,
    ) -> (u16, Option<BrowserWebRtcGrant>) {
        let matches = |grant: &&GrantedSession| {
            (&grant.source_host_id == host_id && &grant.source_boot_id == boot_id)
                || (&grant.sink_host_id == host_id && &grant.sink_boot_id == boot_id)
        };
        let total = self.grants.iter().filter(matches).count() as u16;
        let grant = self
            .grants
            .iter()
            .filter(matches)
            .nth(index as usize)
            .map(|grant| {
                let source = &grant.source_host_id == host_id && &grant.source_boot_id == boot_id;
                BrowserWebRtcGrant {
                    negotiation_id: grant.negotiation_id.clone(),
                    role: if source {
                        BrowserWebRtcRole::Source
                    } else {
                        BrowserWebRtcRole::Sink
                    },
                    peer_host_id: if source {
                        grant.sink_host_id.clone()
                    } else {
                        grant.source_host_id.clone()
                    },
                    peer_boot_id: if source {
                        grant.sink_boot_id.clone()
                    } else {
                        grant.source_boot_id.clone()
                    },
                    session_hello: grant.session_hello.clone(),
                }
            });
        (total, grant)
    }

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
            source_host_id: binding.source.host_id.clone(),
            source_boot_id: binding.source.boot_id.clone(),
            sink_host_id: binding.sink.host_id.clone(),
            sink_boot_id: binding.sink.boot_id.clone(),
            session_hello: session_hello.clone(),
        });
        Ok(session_hello)
    }
}
