use conduit_body::{HostPresenceState, HostPresenceTable, MembershipCredential};
#[cfg(test)]
use conduit_core::BaseImplementationId;
use conduit_core::{BootId, HostId, LinkBindingId};
use conduit_wire::{decode_session_frame, SessionMessage};

pub fn browser_webrtc_line_contract() -> conduit_core::LineContract {
    conduit_core::LineContract {
        scope: conduit_core::LineScope::PointToPoint,
        traffic_shape: conduit_core::LineTrafficShape::Message,
        duplex: conduit_core::LineDuplex::FullDuplex,
        ordering: conduit_core::LineOrdering::Ordered,
        reliability: conduit_core::LineReliability::Reliable,
        continuation: conduit_core::LineContinuation::None,
        security: conduit_core::LineSecurity::AuthenticatedEncrypted,
    }
}
#[path = "webrtc_signaling/grants.rs"]
mod grants;
#[path = "webrtc_signaling/types.rs"]
mod types;
pub use types::*;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Negotiation {
    negotiation_id: LinkBindingId,
    source_host_id: HostId,
    source_boot_id: BootId,
    sink_host_id: HostId,
    sink_boot_id: BootId,
    session_hello: Vec<u8>,
    answered: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserWebRtcRendezvous {
    grants: Vec<GrantedSession>,
    negotiations: Vec<Negotiation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GrantedSession {
    negotiation_id: LinkBindingId,
    source_host_id: HostId,
    source_boot_id: BootId,
    sink_host_id: HostId,
    sink_boot_id: BootId,
    session_hello: Vec<u8>,
}

impl Default for BrowserWebRtcRendezvous {
    fn default() -> Self {
        Self {
            grants: Vec::with_capacity(MAX_WEBRTC_NEGOTIATIONS),
            negotiations: Vec::with_capacity(MAX_WEBRTC_NEGOTIATIONS),
        }
    }
}

impl BrowserWebRtcRendezvous {
    pub fn prepare(
        &self,
        presence: &HostPresenceTable,
        credential: &MembershipCredential,
        target_host_id: HostId,
        target_boot_id: BootId,
        signal: BrowserWebRtcSignal,
    ) -> Result<RoutedBrowserWebRtcSignal, BrowserWebRtcRendezvousRefusal> {
        signal
            .validate()
            .map_err(|_| BrowserWebRtcRendezvousRefusal::InvalidSignal)?;
        let source = presence
            .leases
            .iter()
            .find(|lease| lease.part_id == credential.part_id)
            .filter(|lease| lease.state == HostPresenceState::Available)
            .ok_or(BrowserWebRtcRendezvousRefusal::SourceUnavailable)?;
        if credential.body_id != presence.body_id
            || source.host_id != credential.host_id
            || source.boot_id != credential.boot_id
        {
            return Err(BrowserWebRtcRendezvousRefusal::SourceCredentialMismatch);
        }
        presence
            .leases
            .iter()
            .find(|lease| {
                lease.state == HostPresenceState::Available
                    && lease.host_id == target_host_id
                    && lease.boot_id == target_boot_id
            })
            .ok_or(BrowserWebRtcRendezvousRefusal::TargetUnavailable)?;

        let frame = decode_session_frame(
            &signal.session_hello,
            MAX_WEBRTC_SESSION_HELLO_BYTES as u32,
            MAX_WEBRTC_SESSION_HELLO_BYTES as u32,
        )
        .map_err(|_| BrowserWebRtcRendezvousRefusal::SessionMismatch)?;
        let SessionMessage::Hello(hello) = frame.message else {
            return Err(BrowserWebRtcRendezvousRefusal::SessionMismatch);
        };
        if hello.base != "conduit.base/webrtc-data-channel@1"
            || hello.link_binding_id != signal.negotiation_id.as_str()
        {
            return Err(BrowserWebRtcRendezvousRefusal::SessionMismatch);
        }
        if !self.grants.iter().any(|grant| {
            grant.negotiation_id == signal.negotiation_id
                && grant.session_hello == signal.session_hello
        }) {
            return Err(BrowserWebRtcRendezvousRefusal::UngrantedSession);
        }

        let (
            expected_source_host,
            expected_source_boot,
            expected_target_host,
            expected_target_boot,
        ) = match signal.description {
            BrowserWebRtcDescription::Offer => (
                frame.identity.source_host_id,
                frame.identity.source_boot_id,
                frame.identity.sink_host_id,
                frame.identity.sink_boot_id,
            ),
            BrowserWebRtcDescription::Answer => (
                frame.identity.sink_host_id,
                frame.identity.sink_boot_id,
                frame.identity.source_host_id,
                frame.identity.source_boot_id,
            ),
        };
        if credential.host_id.as_str() != expected_source_host
            || credential.boot_id.as_str() != expected_source_boot
            || target_host_id.as_str() != expected_target_host
            || target_boot_id.as_str() != expected_target_boot
        {
            return Err(BrowserWebRtcRendezvousRefusal::WrongDirection);
        }

        match signal.description {
            BrowserWebRtcDescription::Offer => {
                if self
                    .negotiations
                    .iter()
                    .any(|entry| entry.negotiation_id == signal.negotiation_id)
                {
                    return Err(BrowserWebRtcRendezvousRefusal::DuplicateNegotiation);
                }
                if self.negotiations.len() == MAX_WEBRTC_NEGOTIATIONS {
                    return Err(BrowserWebRtcRendezvousRefusal::CapacityExhausted);
                }
            }
            BrowserWebRtcDescription::Answer => {
                let entry = self
                    .negotiations
                    .iter()
                    .find(|entry| entry.negotiation_id == signal.negotiation_id)
                    .ok_or(BrowserWebRtcRendezvousRefusal::UnknownNegotiation)?;
                if entry.answered {
                    return Err(BrowserWebRtcRendezvousRefusal::InvalidStage);
                }
                if entry.source_host_id != target_host_id
                    || entry.source_boot_id != target_boot_id
                    || entry.sink_host_id != credential.host_id
                    || entry.sink_boot_id != credential.boot_id
                    || entry.session_hello != signal.session_hello
                {
                    return Err(BrowserWebRtcRendezvousRefusal::SessionMismatch);
                }
            }
        }
        Ok(RoutedBrowserWebRtcSignal {
            source_host_id: credential.host_id.clone(),
            source_boot_id: credential.boot_id.clone(),
            target_host_id,
            target_boot_id,
            signal,
        })
    }

    pub fn commit(
        &mut self,
        routed: &RoutedBrowserWebRtcSignal,
    ) -> Result<(), BrowserWebRtcRendezvousRefusal> {
        match routed.signal.description {
            BrowserWebRtcDescription::Offer => {
                if self
                    .negotiations
                    .iter()
                    .any(|entry| entry.negotiation_id == routed.signal.negotiation_id)
                {
                    return Err(BrowserWebRtcRendezvousRefusal::DuplicateNegotiation);
                }
                if self.negotiations.len() == MAX_WEBRTC_NEGOTIATIONS {
                    return Err(BrowserWebRtcRendezvousRefusal::CapacityExhausted);
                }
                self.negotiations.push(Negotiation {
                    negotiation_id: routed.signal.negotiation_id.clone(),
                    source_host_id: routed.source_host_id.clone(),
                    source_boot_id: routed.source_boot_id.clone(),
                    sink_host_id: routed.target_host_id.clone(),
                    sink_boot_id: routed.target_boot_id.clone(),
                    session_hello: routed.signal.session_hello.clone(),
                    answered: false,
                });
            }
            BrowserWebRtcDescription::Answer => {
                let entry = self
                    .negotiations
                    .iter_mut()
                    .find(|entry| entry.negotiation_id == routed.signal.negotiation_id)
                    .ok_or(BrowserWebRtcRendezvousRefusal::UnknownNegotiation)?;
                if entry.answered {
                    return Err(BrowserWebRtcRendezvousRefusal::InvalidStage);
                }
                if entry.source_host_id != routed.target_host_id
                    || entry.source_boot_id != routed.target_boot_id
                    || entry.sink_host_id != routed.source_host_id
                    || entry.sink_boot_id != routed.source_boot_id
                    || entry.session_hello != routed.signal.session_hello
                {
                    return Err(BrowserWebRtcRendezvousRefusal::SessionMismatch);
                }
                entry.answered = true;
            }
        }
        Ok(())
    }

    pub fn invalidate(&mut self, host_id: &HostId, boot_id: &BootId) -> Vec<LinkBindingId> {
        let mut invalidated = Vec::with_capacity(MAX_WEBRTC_NEGOTIATIONS);
        self.grants.retain(|entry| {
            let matches = (&entry.source_host_id == host_id && &entry.source_boot_id == boot_id)
                || (&entry.sink_host_id == host_id && &entry.sink_boot_id == boot_id);
            if matches {
                invalidated.push(entry.negotiation_id.clone());
            }
            !matches
        });
        self.negotiations.retain(|entry| {
            let matches = (&entry.source_host_id == host_id && &entry.source_boot_id == boot_id)
                || (&entry.sink_host_id == host_id && &entry.sink_boot_id == boot_id);
            if matches && !invalidated.contains(&entry.negotiation_id) {
                invalidated.push(entry.negotiation_id.clone());
            }
            !matches
        });
        invalidated
    }
}

#[cfg(test)]
mod tests;
