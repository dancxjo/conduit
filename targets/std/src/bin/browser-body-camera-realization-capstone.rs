//! Two admitted browser Hosts realizing one semantic camera-frame Cord.
//!
//! Realizability depends on exact acquired camera-resource truth. Losing the Host
//! that owns that truth invalidates the dependent realization.

#[path = "browser_webrtc_rendezvous_probe/admission.rs"]
mod admission;
#[path = "browser_body_camera_realization_capstone/planning.rs"]
mod planning;
#[path = "browser_webrtc_rendezvous_probe/protocol.rs"]
mod protocol;

use admission::admit;
use conduit_body::{
    AdmissionManager, Body, BodyMembership, CandidateInventory, HostPresenceClock,
    HostPresenceClockScale, HostPresenceTable,
};
use conduit_core::{CheckedFormId, SignId, SourceDocumentId};
use conduit_std_host::browser_admission::{
    BrowserAdmissionEgress, BrowserAdmissionIngress, BrowserAdmissionListener,
    BrowserAdmissionSocketError, BrowserWebRtcRendezvous, BROWSER_ADMISSION_PROTOCOL,
};
use conduit_std_host::websocket::{NativeWebSocketError, NativeWebSocketError::Transport};
use planning::realize;
use protocol::{debug, exact_credential, frame_kind};
use std::io::ErrorKind;
use std::time::Duration;

const LEASE_MILLIS: u64 = 60_000;
const RENEW_AFTER_MILLIS: u64 = 30_000;

fn main() -> Result<(), String> {
    let body = Body::born(
        SourceDocumentId::from("source/examples/camera-summary.conduit"),
        CheckedFormId::from("checked/camera-summary"),
        1,
        SignId::from("sign/browser-body-camera-realization/body-born"),
    )
    .map_err(debug("Body birth"))?;
    let mut membership = BodyMembership::new(body.body_id.clone()).map_err(debug("membership"))?;
    let mut candidates =
        CandidateInventory::new(body.body_id.clone()).map_err(debug("inventory"))?;
    let mut admission = AdmissionManager::new(body.body_id.clone()).map_err(debug("admission"))?;
    let listener = BrowserAdmissionListener::bind_loopback().map_err(debug("bind"))?;
    println!("{}", listener.url().map_err(debug("URL"))?);

    let mut peers = Vec::with_capacity(2);
    for index in 0..2 {
        peers.push(admit(
            listener.accept().map_err(debug("accept browser Host"))?,
            index,
            &mut candidates,
            &mut admission,
            &mut membership,
        )?);
    }
    let clock = HostPresenceClock::new(
        "clock/browser-body-camera-realization".into(),
        HostPresenceClockScale::Milliseconds,
        1,
        0,
    )
    .map_err(debug("presence clock"))?;
    let mut presence = HostPresenceTable::new(body.body_id.clone(), clock, LEASE_MILLIS)
        .map_err(debug("presence table"))?;
    for (index, peer) in peers.iter_mut().enumerate() {
        presence
            .start(
                &membership,
                &peer.credential.part_id,
                peer.session_id.clone(),
                1,
                1,
                LEASE_MILLIS,
                SignId::from(format!(
                    "sign/browser-body-camera-realization/present-{index}"
                )),
            )
            .map_err(debug("start presence"))?;
        peer.socket
            .send(&BrowserAdmissionEgress::PresenceAccepted {
                protocol: BROWSER_ADMISSION_PROTOCOL,
                sequence: 1,
                renew_after_millis: RENEW_AFTER_MILLIS,
                expires_at_millis: LEASE_MILLIS + 1,
            })
            .map_err(debug("send presence"))?;
        peer.socket
            .set_read_timeout(Some(Duration::from_millis(25)))
            .map_err(debug("set timeout"))?;
    }
    println!(
        "ready source_host={} source_boot={} sink_host={} sink_boot={}",
        peers[0].credential.host_id.as_str(),
        peers[0].credential.boot_id.as_str(),
        peers[1].credential.host_id.as_str(),
        peers[1].credential.boot_id.as_str()
    );

    let mut rendezvous = BrowserWebRtcRendezvous::default();
    let mut planned = false;
    let mut active = [true, true];
    let mut observed_at = 1_u64;
    loop {
        for index in 0..2 {
            if !active[index] {
                continue;
            }
            let frame = match peers[index].socket.receive() {
                Ok(frame) => frame,
                Err(BrowserAdmissionSocketError::Transport(Transport(
                    ErrorKind::TimedOut | ErrorKind::WouldBlock,
                ))) => continue,
                Err(BrowserAdmissionSocketError::Transport(
                    NativeWebSocketError::Disconnected | NativeWebSocketError::Transport(_),
                )) => {
                    let invalidated = rendezvous.invalidate(
                        &peers[index].credential.host_id,
                        &peers[index].credential.boot_id,
                    );
                    observed_at = observed_at.checked_add(1).ok_or("clock exhausted")?;
                    presence
                        .lose_session(
                            &mut membership,
                            &peers[index].credential.part_id,
                            &peers[index].session_id,
                            observed_at,
                            SignId::from(format!(
                                "sign/browser-body-camera-realization/lost-{index}"
                            )),
                        )
                        .map_err(debug("lose presence"))?;
                    active[index] = false;
                    println!(
                        "host_loss={}",
                        serde_json::json!({
                            "index": index,
                            "host_id": peers[index].credential.host_id,
                            "boot_id": peers[index].credential.boot_id,
                            "invalidated_binding_ids": invalidated,
                            "form": "camera-summary",
                            "replacement": "unrealizable-without-new-camera-resource-truth",
                        })
                    );
                    if planned {
                        return Ok(());
                    }
                    continue;
                }
                Err(error) => return Err(format!("receive Host {index}: {error:?}")),
            };
            println!("received index={index} kind={}", frame_kind(&frame));
            match frame {
                BrowserAdmissionIngress::MediaResourceTruth {
                    credential_id,
                    body_id,
                    part_id,
                    host_id,
                    boot_id,
                    resource,
                    ..
                } => {
                    if index != 0 || planned {
                        return Err(
                            "camera resource truth arrived outside source planning stage".into(),
                        );
                    }
                    exact_credential(
                        &peers[index].credential,
                        &credential_id,
                        &body_id,
                        &part_id,
                        &host_id,
                        &boot_id,
                    )?;
                    let realization = realize(
                        &peers[0].credential,
                        &peers[0].advertisement,
                        &peers[1].advertisement,
                        &resource,
                    )?;
                    rendezvous
                        .replace_grants([&realization.binding])
                        .map_err(debug("install planned grant"))?;
                    for peer in &mut peers {
                        peer.socket
                            .send(&BrowserAdmissionEgress::WebRtcPlanReady {
                                protocol: BROWSER_ADMISSION_PROTOCOL,
                                generation: 1,
                                plan_id: realization.plan.plan_id.clone(),
                            })
                            .map_err(debug("send WebRTC Plan-ready transition"))?;
                    }
                    peers[0]
                        .socket
                        .send(&BrowserAdmissionEgress::MediaUsePlan {
                            protocol: BROWSER_ADMISSION_PROTOCOL,
                            plan_id: realization.plan.plan_id.clone(),
                            resource_handle: resource.handle_id.clone(),
                            output_port: realization.output_port,
                        })
                        .map_err(debug("send media use Plan"))?;
                    println!(
                        "planned={}",
                        serde_json::json!({
                            "plan_id": realization.plan.plan_id,
                            "resource_handle": resource.handle_id,
                            "authority_grant": resource.use_authority_grant,
                            "connection_id": realization.binding.connection_id,
                            "line_id": realization.binding.attachment.line_id,
                            "value_kind": realization.binding.value_kind,
                            "limits": {
                                "maximum_in_flight_items": realization.binding.limits.maximum_in_flight_items,
                                "maximum_payload_bytes": realization.binding.limits.maximum_payload_bytes,
                                "maximum_buffered_bytes": realization.binding.limits.maximum_buffered_bytes,
                            },
                            "output_port": "frame",
                        })
                    );
                    planned = true;
                }
                BrowserAdmissionIngress::WebRtcGrantRequest {
                    credential_id,
                    body_id,
                    part_id,
                    host_id,
                    boot_id,
                    generation,
                    index: grant_index,
                    ..
                } => {
                    exact_credential(
                        &peers[index].credential,
                        &credential_id,
                        &body_id,
                        &part_id,
                        &host_id,
                        &boot_id,
                    )?;
                    if !planned {
                        peers[index]
                            .socket
                            .send(&BrowserAdmissionEgress::WebRtcGrant {
                                protocol: BROWSER_ADMISSION_PROTOCOL,
                                generation,
                                index: grant_index,
                                total: 0,
                                grant: None,
                            })
                            .map_err(debug("send empty pre-Plan grant"))?;
                        continue;
                    }
                    if generation != 1 {
                        return Err("stale WebRTC generation after the exact camera Plan".into());
                    }
                    let (total, grant) = rendezvous.grant_for_endpoint(
                        &peers[index].credential.host_id,
                        &peers[index].credential.boot_id,
                        grant_index,
                    );
                    peers[index]
                        .socket
                        .send(&BrowserAdmissionEgress::WebRtcGrant {
                            protocol: BROWSER_ADMISSION_PROTOCOL,
                            generation,
                            index: grant_index,
                            total,
                            grant,
                        })
                        .map_err(debug("send WebRTC grant"))?;
                }
                BrowserAdmissionIngress::WebRtcSignal {
                    credential_id,
                    body_id,
                    part_id,
                    host_id,
                    boot_id,
                    target_host_id,
                    target_boot_id,
                    signal,
                    ..
                } => {
                    exact_credential(
                        &peers[index].credential,
                        &credential_id,
                        &body_id,
                        &part_id,
                        &host_id,
                        &boot_id,
                    )?;
                    let routed = rendezvous
                        .prepare(
                            &presence,
                            &peers[index].credential,
                            target_host_id,
                            target_boot_id,
                            signal,
                        )
                        .map_err(debug("prepare WebRTC signal"))?;
                    let target = peers
                        .iter_mut()
                        .find(|peer| {
                            peer.credential.host_id == routed.target_host_id
                                && peer.credential.boot_id == routed.target_boot_id
                        })
                        .ok_or("rendezvous target absent")?;
                    target
                        .socket
                        .send(&BrowserAdmissionEgress::WebRtcSignal {
                            protocol: BROWSER_ADMISSION_PROTOCOL,
                            source_host_id: routed.source_host_id.clone(),
                            source_boot_id: routed.source_boot_id.clone(),
                            signal: routed.signal.clone(),
                        })
                        .map_err(debug("relay WebRTC signal"))?;
                    rendezvous.commit(&routed).map_err(debug("commit signal"))?;
                }
                BrowserAdmissionIngress::PresenceRenewal {
                    credential_id,
                    body_id,
                    part_id,
                    host_id,
                    boot_id,
                    sequence,
                    ..
                } => {
                    exact_credential(
                        &peers[index].credential,
                        &credential_id,
                        &body_id,
                        &part_id,
                        &host_id,
                        &boot_id,
                    )?;
                    observed_at = observed_at.checked_add(1).ok_or("clock exhausted")?;
                    presence
                        .renew(
                            &membership,
                            &peers[index].credential.part_id,
                            &peers[index].session_id,
                            sequence,
                            observed_at,
                            LEASE_MILLIS,
                            SignId::from(format!(
                                "sign/browser-body-camera-realization/renewed-{index}-{sequence}"
                            )),
                        )
                        .map_err(debug("renew presence"))?;
                    peers[index]
                        .socket
                        .send(&BrowserAdmissionEgress::PresenceAccepted {
                            protocol: BROWSER_ADMISSION_PROTOCOL,
                            sequence,
                            renew_after_millis: RENEW_AFTER_MILLIS,
                            expires_at_millis: LEASE_MILLIS + observed_at,
                        })
                        .map_err(debug("renew presence"))?;
                }
                _ => return Err(format!("unexpected Host {index} frame")),
            }
        }
    }
}
