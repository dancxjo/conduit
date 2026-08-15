use conduit_body::{HostPresenceState, HostPresenceTable, MembershipCredential};
use conduit_core::{BootId, ConnectionBase, HostId, LinkBindingId};
use conduit_wire::{decode_session_frame, SessionMessage};
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
        if hello.base != ConnectionBase::WebRtcDataChannel
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
        let mut invalidated = Vec::with_capacity(self.negotiations.len());
        self.negotiations.retain(|entry| {
            let matches = (&entry.source_host_id == host_id && &entry.source_boot_id == boot_id)
                || (&entry.sink_host_id == host_id && &entry.sink_boot_id == boot_id);
            if matches {
                invalidated.push(entry.negotiation_id.clone());
            }
            !matches
        });
        invalidated
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_body::HostPresenceLease;
    use conduit_core::{
        bind_active_play, ConnectionBaseInstanceId, ConnectionId, FragmentId, KindId, LineId,
        LinkEndpointId, LinkLimits, PlanId, PROTOCOL_VERSION,
    };
    use conduit_wire::{
        encode_session_frame_into, LineAttachment, SessionBinding, SessionEndpointIdentity,
        SessionLimits,
    };

    fn credential(label: &str) -> MembershipCredential {
        serde_json::from_value(serde_json::json!({
            "credential_id": format!("credential/{label}"),
            "body_id": "body/rendezvous",
            "part_id": format!("part/{label}"),
            "host_id": format!("host/{label}"),
            "boot_id": format!("boot/{label}"),
            "issued_at_millis": 1,
        }))
        .unwrap()
    }

    fn presence(source: &MembershipCredential, sink: &MembershipCredential) -> HostPresenceTable {
        HostPresenceTable {
            body_id: source.body_id.clone(),
            maximum_lease_millis: 1_000,
            revision: 2,
            dropped_event_count: 0,
            leases: [source, sink]
                .into_iter()
                .enumerate()
                .map(|(index, credential)| HostPresenceLease {
                    part_id: credential.part_id.clone(),
                    host_id: credential.host_id.clone(),
                    boot_id: credential.boot_id.clone(),
                    offer_generation: conduit_core::OfferGeneration(1),
                    membership_proof_id: serde_json::from_value(serde_json::json!(format!(
                        "proof/{index}"
                    )))
                    .unwrap(),
                    session_binding_id: LinkBindingId::from(format!("presence/{index}")),
                    sequence: 1,
                    observed_at_millis: 0,
                    expires_at_millis: 1_000,
                    state: HostPresenceState::Available,
                })
                .collect(),
            events: Vec::new(),
        }
    }

    fn binding(source: &MembershipCredential, sink: &MembershipCredential) -> SessionBinding {
        let plan_id = PlanId::from("plan/rendezvous");
        SessionBinding {
            protocol_version: PROTOCOL_VERSION,
            plan_id: plan_id.clone(),
            source_fragment_id: FragmentId::from("fragment/source"),
            sink_fragment_id: FragmentId::from("fragment/sink"),
            source_active_play_id: bind_active_play(&plan_id, &source.host_id, &source.boot_id, 0)
                .active_play_id,
            sink_active_play_id: bind_active_play(&plan_id, &sink.host_id, &sink.boot_id, 0)
                .active_play_id,
            connection_id: ConnectionId::from("connection/rendezvous"),
            source: SessionEndpointIdentity {
                host_id: source.host_id.clone(),
                boot_id: source.boot_id.clone(),
            },
            sink: SessionEndpointIdentity {
                host_id: sink.host_id.clone(),
                boot_id: sink.boot_id.clone(),
            },
            value_kind: KindId::from("value/bounded@1"),
            limits: SessionLimits {
                maximum_in_flight_items: 1,
                maximum_payload_bytes: 16,
                maximum_buffered_bytes: 16,
            },
            attachment: LineAttachment {
                line_id: LineId::from("line/rendezvous"),
                link_binding_id: LinkBindingId::from("binding/rendezvous"),
                base: ConnectionBase::WebRtcDataChannel,
                base_instance_id: ConnectionBaseInstanceId::from("base/rendezvous"),
                source_host_id: source.host_id.clone(),
                source_boot_id: source.boot_id.clone(),
                source_endpoint_id: LinkEndpointId::from("endpoint/source"),
                sink_host_id: sink.host_id.clone(),
                sink_boot_id: sink.boot_id.clone(),
                sink_endpoint_id: LinkEndpointId::from("endpoint/sink"),
                limits: LinkLimits {
                    maximum_in_flight_items: 1,
                    maximum_payload_bytes: 16,
                    maximum_buffered_bytes: 16,
                    maximum_frame_bytes: MAX_WEBRTC_SESSION_HELLO_BYTES as u32,
                },
            },
        }
    }

    fn signal(description: BrowserWebRtcDescription, hello: Vec<u8>) -> BrowserWebRtcSignal {
        BrowserWebRtcSignal {
            negotiation_id: LinkBindingId::from("binding/rendezvous"),
            description,
            session_hello: hello,
            sdp: match description {
                BrowserWebRtcDescription::Offer => "v=0\na=setup:actpass",
                BrowserWebRtcDescription::Answer => "v=0\na=setup:active",
            }
            .into(),
        }
    }

    #[test]
    fn exact_available_pair_routes_offer_then_answer_and_invalidates_on_boot_loss() {
        let source = credential("source");
        let sink = credential("sink");
        let presence = presence(&source, &sink);
        let mut rendezvous = BrowserWebRtcRendezvous::default();
        let granted = binding(&source, &sink);
        let hello = rendezvous.preflight_grants([&granted]).unwrap().remove(0);
        assert_eq!(
            rendezvous.prepare(
                &presence,
                &source,
                sink.host_id.clone(),
                sink.boot_id.clone(),
                signal(BrowserWebRtcDescription::Offer, hello.clone()),
            ),
            Err(BrowserWebRtcRendezvousRefusal::UngrantedSession)
        );
        assert_eq!(rendezvous.replace_grants([&granted]).unwrap()[0], hello);
        assert_eq!(
            rendezvous.grant(&granted),
            Err(BrowserWebRtcRendezvousRefusal::DuplicateGrant)
        );

        let offered = rendezvous
            .prepare(
                &presence,
                &source,
                sink.host_id.clone(),
                sink.boot_id.clone(),
                signal(BrowserWebRtcDescription::Offer, hello.clone()),
            )
            .unwrap();
        assert!(rendezvous.negotiations.is_empty());
        assert!(rendezvous
            .prepare(
                &presence,
                &source,
                sink.host_id.clone(),
                sink.boot_id.clone(),
                signal(BrowserWebRtcDescription::Offer, hello.clone()),
            )
            .is_ok());
        rendezvous.commit(&offered).unwrap();
        assert_eq!(
            rendezvous.replace_grants(core::iter::empty()),
            Err(BrowserWebRtcRendezvousRefusal::InvalidStage)
        );
        assert_eq!(
            rendezvous.deactivate_grants(),
            vec![LinkBindingId::from("binding/rendezvous")]
        );
        assert_eq!(
            rendezvous.prepare(
                &presence,
                &source,
                sink.host_id.clone(),
                sink.boot_id.clone(),
                signal(BrowserWebRtcDescription::Offer, hello.clone()),
            ),
            Err(BrowserWebRtcRendezvousRefusal::UngrantedSession)
        );
        let hello = rendezvous.grant(&granted).unwrap();
        let offered = rendezvous
            .prepare(
                &presence,
                &source,
                sink.host_id.clone(),
                sink.boot_id.clone(),
                signal(BrowserWebRtcDescription::Offer, hello.clone()),
            )
            .unwrap();
        rendezvous.commit(&offered).unwrap();
        assert_eq!(offered.target_host_id, sink.host_id);
        let answered = rendezvous
            .prepare(
                &presence,
                &sink,
                source.host_id.clone(),
                source.boot_id.clone(),
                signal(BrowserWebRtcDescription::Answer, hello),
            )
            .unwrap();
        rendezvous.commit(&answered).unwrap();
        assert_eq!(answered.target_host_id, source.host_id);

        assert_eq!(
            rendezvous.invalidate(&sink.host_id, &sink.boot_id),
            vec![LinkBindingId::from("binding/rendezvous")]
        );
    }

    #[test]
    fn stale_target_wrong_direction_and_duplicate_stage_refuse_without_relay() {
        let source = credential("source");
        let sink = credential("sink");
        let mut presence = presence(&source, &sink);
        let mut rendezvous = BrowserWebRtcRendezvous::default();
        let ungranted = binding(&source, &sink);
        let mut encoded = [0; MAX_WEBRTC_SESSION_HELLO_BYTES];
        let length = encode_session_frame_into(
            ungranted.hello_frame(),
            &mut encoded,
            ungranted.limits.maximum_payload_bytes,
            MAX_WEBRTC_SESSION_HELLO_BYTES as u32,
        )
        .unwrap();
        assert_eq!(
            rendezvous.prepare(
                &presence,
                &source,
                sink.host_id.clone(),
                sink.boot_id.clone(),
                signal(BrowserWebRtcDescription::Offer, encoded[..length].to_vec(),),
            ),
            Err(BrowserWebRtcRendezvousRefusal::UngrantedSession)
        );
        let hello = rendezvous.grant(&binding(&source, &sink)).unwrap();

        assert_eq!(
            rendezvous.prepare(
                &presence,
                &source,
                sink.host_id.clone(),
                BootId::from("boot/stale"),
                signal(BrowserWebRtcDescription::Offer, hello.clone()),
            ),
            Err(BrowserWebRtcRendezvousRefusal::TargetUnavailable)
        );
        assert_eq!(
            rendezvous.prepare(
                &presence,
                &sink,
                source.host_id.clone(),
                source.boot_id.clone(),
                signal(BrowserWebRtcDescription::Offer, hello.clone()),
            ),
            Err(BrowserWebRtcRendezvousRefusal::WrongDirection)
        );
        let offered = rendezvous
            .prepare(
                &presence,
                &source,
                sink.host_id.clone(),
                sink.boot_id.clone(),
                signal(BrowserWebRtcDescription::Offer, hello.clone()),
            )
            .unwrap();
        rendezvous.commit(&offered).unwrap();
        assert_eq!(
            rendezvous.prepare(
                &presence,
                &source,
                sink.host_id.clone(),
                sink.boot_id.clone(),
                signal(BrowserWebRtcDescription::Offer, hello.clone()),
            ),
            Err(BrowserWebRtcRendezvousRefusal::DuplicateNegotiation)
        );
        presence.leases[1].state = HostPresenceState::Unavailable;
        assert_eq!(
            rendezvous.prepare(
                &presence,
                &sink,
                source.host_id.clone(),
                source.boot_id.clone(),
                signal(BrowserWebRtcDescription::Answer, hello),
            ),
            Err(BrowserWebRtcRendezvousRefusal::SourceUnavailable)
        );
    }
}
