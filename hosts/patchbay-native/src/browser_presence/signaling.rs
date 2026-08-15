//! Atomic bounded WebRTC rendezvous relay between admitted browser workers.

use conduit_std_host::browser_admission::{
    BrowserAdmissionEgress, BrowserAdmissionIngress, BROWSER_ADMISSION_PROTOCOL,
};
use std::sync::mpsc::SyncSender;

use super::{BrowserPresenceCoordinator, WorkerResponse};

impl BrowserPresenceCoordinator {
    pub(super) fn relay_webrtc(
        &mut self,
        index: usize,
        frame: BrowserAdmissionIngress,
        response: SyncSender<WorkerResponse>,
    ) -> Result<Option<String>, String> {
        let BrowserAdmissionIngress::WebRtcSignal {
            credential_id,
            body_id,
            part_id,
            host_id,
            boot_id,
            target_host_id,
            target_boot_id,
            signal,
            ..
        } = frame
        else {
            return Err("non-WebRTC frame reached WebRTC relay".into());
        };
        let credential = self.workers[index].credential.clone();
        if credential_id != credential.credential_id
            || body_id != credential.body_id
            || part_id != credential.part_id
            || host_id != credential.host_id
            || boot_id != credential.boot_id
        {
            response
                .send(WorkerResponse::Refused(
                    "stale-membership-credential".into(),
                ))
                .map_err(|_| "browser WebRTC response worker disconnected".to_string())?;
            return Ok(Some("Browser WebRTC signaling credential refused".into()));
        }
        let routed = match self.rendezvous.prepare(
            &self.table,
            &credential,
            target_host_id,
            target_boot_id,
            signal,
        ) {
            Ok(routed) => routed,
            Err(refusal) => {
                let code = format!("webrtc-rendezvous-{refusal:?}");
                response
                    .send(WorkerResponse::Refused(code.clone()))
                    .map_err(|_| "browser WebRTC response worker disconnected".to_string())?;
                return Ok(Some(format!("Browser WebRTC signaling refused: {code}")));
            }
        };
        let Some(target) = self.workers.iter().find(|worker| {
            worker.credential.host_id == routed.target_host_id
                && worker.credential.boot_id == routed.target_boot_id
        }) else {
            response
                .send(WorkerResponse::Refused("webrtc-target-absent".into()))
                .map_err(|_| "browser WebRTC response worker disconnected".to_string())?;
            return Ok(Some("Browser WebRTC target worker is absent".into()));
        };
        if target
            .outbound
            .try_send(BrowserAdmissionEgress::WebRtcSignal {
                protocol: BROWSER_ADMISSION_PROTOCOL,
                source_host_id: routed.source_host_id.clone(),
                source_boot_id: routed.source_boot_id.clone(),
                signal: routed.signal.clone(),
            })
            .is_err()
        {
            response
                .send(WorkerResponse::Refused("webrtc-target-pressure".into()))
                .map_err(|_| "browser WebRTC response worker disconnected".to_string())?;
            return Ok(Some(
                "Browser WebRTC target outbound capacity exhausted".into(),
            ));
        }
        self.rendezvous
            .commit(&routed)
            .map_err(|refusal| format!("WebRTC rendezvous commit refused: {refusal:?}"))?;
        response
            .send(WorkerResponse::Relayed)
            .map_err(|_| "browser WebRTC response worker disconnected".to_string())?;
        Ok(Some(
            "Browser WebRTC signaling relayed through current Body".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser_presence::{PresenceWorker, WorkerEvent};
    use conduit_body::{HostPresenceLease, HostPresenceState, MembershipCredential};
    use conduit_core::{
        bind_active_play, ConnectionBase, ConnectionBaseInstanceId, ConnectionId, FragmentId,
        KindId, LineId, LinkBindingId, LinkEndpointId, LinkLimits, PlanId, PROTOCOL_VERSION,
    };
    use conduit_std_host::browser_admission::{
        BrowserWebRtcDescription, BrowserWebRtcSignal, MAX_WEBRTC_SESSION_HELLO_BYTES,
    };
    use conduit_wire::{LineAttachment, SessionBinding, SessionEndpointIdentity, SessionLimits};
    use std::sync::mpsc;

    fn credential(label: &str) -> MembershipCredential {
        serde_json::from_value(serde_json::json!({
            "credential_id": format!("credential/{label}"),
            "body_id": "body/native-rendezvous",
            "part_id": format!("part/{label}"),
            "host_id": format!("host/{label}"),
            "boot_id": format!("boot/{label}"),
            "issued_at_millis": 1,
        }))
        .unwrap()
    }

    fn binding(source: &MembershipCredential, sink: &MembershipCredential) -> SessionBinding {
        let plan_id = PlanId::from("plan/native-rendezvous");
        SessionBinding {
            protocol_version: PROTOCOL_VERSION,
            plan_id: plan_id.clone(),
            source_fragment_id: FragmentId::from("fragment/source"),
            sink_fragment_id: FragmentId::from("fragment/sink"),
            source_active_play_id: bind_active_play(&plan_id, &source.host_id, &source.boot_id, 0)
                .active_play_id,
            sink_active_play_id: bind_active_play(&plan_id, &sink.host_id, &sink.boot_id, 0)
                .active_play_id,
            connection_id: ConnectionId::from("connection/native-rendezvous"),
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
                line_id: LineId::from("line/native-rendezvous"),
                link_binding_id: LinkBindingId::from("binding/native-rendezvous"),
                base: ConnectionBase::WebRtcDataChannel,
                base_instance_id: ConnectionBaseInstanceId::from("base/native-rendezvous"),
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

    #[test]
    fn target_pressure_refuses_without_committing_then_exact_retry_relays() {
        let source = credential("source");
        let sink = credential("sink");
        let mut coordinator = BrowserPresenceCoordinator::new(source.body_id.clone());
        coordinator.table.leases = [&source, &sink]
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
            .collect();
        let mut target_receiver = None;
        for credential in [&source, &sink] {
            let (_event_sender, receiver) = mpsc::channel::<WorkerEvent>();
            let (outbound, outbound_receiver) = mpsc::sync_channel(1);
            if credential.host_id == sink.host_id {
                target_receiver = Some(outbound_receiver);
            }
            coordinator.workers.push(PresenceWorker {
                credential: credential.clone(),
                session_id: LinkBindingId::from(format!("worker/{}", credential.host_id.as_str())),
                receiver,
                outbound,
            });
        }
        let target_receiver = target_receiver.unwrap();
        let session_hello = coordinator
            .rendezvous
            .grant(&binding(&source, &sink))
            .unwrap();
        coordinator.workers[1]
            .outbound
            .try_send(BrowserAdmissionEgress::Refused {
                protocol: BROWSER_ADMISSION_PROTOCOL,
                code: "occupy-capacity".into(),
            })
            .unwrap();
        let offer = BrowserAdmissionIngress::WebRtcSignal {
            protocol: BROWSER_ADMISSION_PROTOCOL,
            credential_id: source.credential_id.clone(),
            body_id: source.body_id.clone(),
            part_id: source.part_id.clone(),
            host_id: source.host_id.clone(),
            boot_id: source.boot_id.clone(),
            target_host_id: sink.host_id.clone(),
            target_boot_id: sink.boot_id.clone(),
            signal: BrowserWebRtcSignal {
                negotiation_id: LinkBindingId::from("binding/native-rendezvous"),
                description: BrowserWebRtcDescription::Offer,
                session_hello,
                sdp: "v=0\na=setup:actpass".into(),
            },
        };
        let (response, pressure_response) = mpsc::sync_channel(1);
        coordinator
            .relay_webrtc(0, offer.clone(), response)
            .unwrap();
        assert!(matches!(
            pressure_response.recv().unwrap(),
            WorkerResponse::Refused(code) if code == "webrtc-target-pressure"
        ));
        target_receiver.recv().unwrap();

        let (response, accepted_response) = mpsc::sync_channel(1);
        coordinator.relay_webrtc(0, offer, response).unwrap();
        assert!(matches!(
            accepted_response.recv().unwrap(),
            WorkerResponse::Relayed
        ));
        let relayed = target_receiver.recv().unwrap();
        assert!(matches!(
            relayed,
            BrowserAdmissionEgress::WebRtcSignal { source_host_id, source_boot_id, .. }
                if source_host_id == source.host_id && source_boot_id == source.boot_id
        ));
    }
}
