//! Bounded two-browser admission and WebRTC rendezvous proof fixture.

#[path = "browser_webrtc_rendezvous_probe/admission.rs"]
mod admission;
#[path = "browser_webrtc_rendezvous_probe/planning.rs"]
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
use planning::{binding, session_basis};
use protocol::{debug, exact_credential, frame_kind};
use std::io::ErrorKind;
use std::time::Duration;

const LEASE_MILLIS: u64 = 60_000;
const RENEW_AFTER_MILLIS: u64 = 30_000;

fn main() -> Result<(), String> {
    let body = Body::born(
        SourceDocumentId::from("source/browser-webrtc-rendezvous-probe"),
        CheckedFormId::from("checked/browser-webrtc-rendezvous-probe"),
        1,
        SignId::from("sign/browser-webrtc-rendezvous-probe/body-born"),
    )
    .map_err(debug("Body birth"))?;
    let body_id = body.body_id.clone();
    let mut membership = BodyMembership::new(body.body_id.clone()).map_err(debug("membership"))?;
    let mut candidates =
        CandidateInventory::new(body.body_id.clone()).map_err(debug("inventory"))?;
    let mut admission = AdmissionManager::new(body.body_id.clone()).map_err(debug("admission"))?;
    let listener = BrowserAdmissionListener::bind_loopback().map_err(debug("bind"))?;
    println!("{}", listener.url().map_err(debug("URL"))?);

    let mut peers = Vec::with_capacity(2);
    for index in 0..2 {
        let socket = listener.accept().map_err(debug("accept"))?;
        peers.push(admit(
            socket,
            index,
            &mut candidates,
            &mut admission,
            &mut membership,
        )?);
    }

    let clock = HostPresenceClock::new(
        "clock/browser-webrtc-rendezvous-probe".into(),
        HostPresenceClockScale::Milliseconds,
        1,
        0,
    )
    .map_err(debug("presence clock"))?;
    let mut presence = HostPresenceTable::new(body.body_id, clock, LEASE_MILLIS)
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
                    "sign/browser-webrtc-rendezvous-probe/present-{index}"
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

    let initial_binding = binding(&peers[0].credential, &peers[1].credential, 0);
    let replacement_binding = binding(&peers[0].credential, &peers[1].credential, 1);
    let mut rendezvous = BrowserWebRtcRendezvous::default();
    rendezvous
        .replace_grants([&initial_binding])
        .map_err(debug("install grant"))?;
    println!(
        "ready source_host={} source_boot={} sink_host={} sink_boot={}",
        peers[0].credential.host_id.as_str(),
        peers[0].credential.boot_id.as_str(),
        peers[1].credential.host_id.as_str(),
        peers[1].credential.boot_id.as_str()
    );
    println!(
        "session_basis={}",
        session_basis(
            &body_id,
            &peers[0].credential,
            &peers[1].credential,
            &initial_binding,
            0,
        )
    );

    let mut relayed = 0_u8;
    let mut active = vec![true, true];
    let mut grant_generation = 0_u16;
    let mut stale_rendezvous = None;
    let mut restart_admitted = false;
    let mut observed_at_millis = 1_u64;
    loop {
        for index in 0..peers.len() {
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
                    println!("peer-lost index={index} relayed={relayed}");
                    let invalidated = rendezvous.invalidate(
                        &peers[index].credential.host_id,
                        &peers[index].credential.boot_id,
                    );
                    observed_at_millis = observed_at_millis
                        .checked_add(1)
                        .ok_or("presence observation clock exhausted")?;
                    presence
                        .lose_session(
                            &mut membership,
                            &peers[index].credential.part_id,
                            &peers[index].session_id,
                            observed_at_millis,
                            SignId::from(format!(
                                "sign/browser-webrtc-rendezvous-probe/lost-{index}"
                            )),
                        )
                        .map_err(debug("lose peer presence"))?;
                    active[index] = false;
                    if restart_admitted {
                        continue;
                    }
                    let survivor_index = usize::from(index == 0);
                    let lost_part = membership
                        .parts
                        .iter()
                        .find(|part| part.part_id == peers[index].credential.part_id)
                        .ok_or("lost durable Part absent")?;
                    let survivor_part = membership
                        .parts
                        .iter()
                        .find(|part| part.part_id == peers[survivor_index].credential.part_id)
                        .ok_or("surviving Part absent")?;
                    let lost_presence = presence
                        .leases
                        .iter()
                        .find(|lease| lease.part_id == peers[index].credential.part_id)
                        .ok_or("lost presence lease absent")?;
                    let survivor_presence = presence
                        .leases
                        .iter()
                        .find(|lease| lease.part_id == peers[survivor_index].credential.part_id)
                        .ok_or("surviving presence lease absent")?;
                    let (lost_grant_total, lost_grant) = rendezvous.grant_for_endpoint(
                        &peers[index].credential.host_id,
                        &peers[index].credential.boot_id,
                        0,
                    );
                    let (survivor_grant_total, survivor_grant) = rendezvous.grant_for_endpoint(
                        &peers[survivor_index].credential.host_id,
                        &peers[survivor_index].credential.boot_id,
                        0,
                    );
                    println!(
                        "host_loss={}",
                        serde_json::json!({
                            "lost_index": index,
                            "lost_part": lost_part,
                            "lost_presence": lost_presence,
                            "survivor_index": survivor_index,
                            "survivor_part": survivor_part,
                            "survivor_presence": survivor_presence,
                            "invalidated_binding_ids": invalidated,
                            "lost_grant_total": lost_grant_total,
                            "lost_grant_present": lost_grant.is_some(),
                            "survivor_grant_total": survivor_grant_total,
                            "survivor_grant_present": survivor_grant.is_some(),
                        })
                    );
                    let old_part_id = peers[index].credential.part_id.clone();
                    let old_host_id = peers[index].credential.host_id.clone();
                    let old_boot_id = peers[index].credential.boot_id.clone();
                    let mut fresh = admit(
                        listener
                            .accept()
                            .map_err(debug("accept restarted browser"))?,
                        2,
                        &mut candidates,
                        &mut admission,
                        &mut membership,
                    )?;
                    observed_at_millis = observed_at_millis
                        .checked_add(1)
                        .ok_or("presence observation clock exhausted")?;
                    presence
                        .start(
                            &membership,
                            &fresh.credential.part_id,
                            fresh.session_id.clone(),
                            1,
                            observed_at_millis,
                            LEASE_MILLIS,
                            SignId::from("sign/browser-webrtc-rendezvous-probe/present-restart"),
                        )
                        .map_err(debug("start restarted presence"))?;
                    fresh
                        .socket
                        .send(&BrowserAdmissionEgress::PresenceAccepted {
                            protocol: BROWSER_ADMISSION_PROTOCOL,
                            sequence: 1,
                            renew_after_millis: RENEW_AFTER_MILLIS,
                            expires_at_millis: LEASE_MILLIS + observed_at_millis,
                        })
                        .map_err(debug("send restarted presence"))?;
                    fresh
                        .socket
                        .set_read_timeout(Some(Duration::from_millis(25)))
                        .map_err(debug("set restarted timeout"))?;
                    let old_part = membership
                        .parts
                        .iter()
                        .find(|part| part.part_id == old_part_id)
                        .ok_or("offline Part disappeared during browser restart")?;
                    let fresh_part = membership
                        .parts
                        .iter()
                        .find(|part| part.part_id == fresh.credential.part_id)
                        .ok_or("fresh browser Part was not admitted")?;
                    let old_presence = presence
                        .leases
                        .iter()
                        .find(|lease| lease.part_id == old_part_id)
                        .ok_or("offline Part presence disappeared during browser restart")?;
                    let fresh_presence = presence
                        .leases
                        .iter()
                        .find(|lease| lease.part_id == fresh.credential.part_id)
                        .ok_or("fresh browser presence was not established")?;
                    let (old_grant_total, old_grant) =
                        rendezvous.grant_for_endpoint(&old_host_id, &old_boot_id, 0);
                    let (fresh_grant_total, fresh_grant) = rendezvous.grant_for_endpoint(
                        &fresh.credential.host_id,
                        &fresh.credential.boot_id,
                        0,
                    );
                    println!(
                        "restart={}",
                        serde_json::json!({
                            "old_part": old_part,
                            "old_presence": old_presence,
                            "old_host_id": old_host_id,
                            "old_boot_id": old_boot_id,
                            "fresh_part": fresh_part,
                            "fresh_presence": fresh_presence,
                            "fresh_credential": fresh.credential,
                            "old_grant_total": old_grant_total,
                            "old_grant_present": old_grant.is_some(),
                            "fresh_grant_total": fresh_grant_total,
                            "fresh_grant_present": fresh_grant.is_some(),
                            "membership_part_count": membership.parts.len(),
                            "presence_lease_count": presence.leases.len(),
                        })
                    );
                    peers.push(fresh);
                    active.push(true);
                    restart_admitted = true;
                    continue;
                }
                Err(error) => return Err(format!("receive peer {index}: {error:?}")),
            };
            println!("received index={index} kind={}", frame_kind(&frame));
            match frame {
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
                    if index >= 2 {
                        if generation != 0 {
                            return Err(
                                "fresh browser requested a noninitial grant generation".into()
                            );
                        }
                        peers[index]
                            .socket
                            .send(&BrowserAdmissionEgress::WebRtcGrant {
                                protocol: BROWSER_ADMISSION_PROTOCOL,
                                generation,
                                index: grant_index,
                                total: 0,
                                grant: None,
                            })
                            .map_err(debug("send empty restarted grant"))?;
                        println!(
                            "restart-grant-empty index={index} generation={generation} grant_index={grant_index}"
                        );
                        continue;
                    }
                    if generation == 1 && grant_generation == 0 {
                        if grant_index != 0 || relayed != 2 {
                            return Err(
                                "replacement grant requested outside the exact replan stage".into(),
                            );
                        }
                        stale_rendezvous = Some(rendezvous.clone());
                        rendezvous.deactivate_grants();
                        rendezvous
                            .replace_grants([&replacement_binding])
                            .map_err(debug("install replacement grant"))?;
                        grant_generation = 1;
                        println!(
                            "replacement_basis={}",
                            session_basis(
                                &body_id,
                                &peers[0].credential,
                                &peers[1].credential,
                                &replacement_binding,
                                1,
                            )
                        );
                    }
                    if generation != grant_generation {
                        return Err(format!(
                            "stale grant generation requested={generation} current={grant_generation}"
                        ));
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
                        .map_err(debug("send grant"))?;
                    println!("grant index={index} grant_index={grant_index} total={total}");
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
                        .map_err(debug("prepare signal"))?;
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
                        .map_err(debug("relay signal"))?;
                    rendezvous.commit(&routed).map_err(debug("commit signal"))?;
                    relayed = relayed.checked_add(1).ok_or("relay count exhausted")?;
                    println!("relayed stage={relayed}");
                    if relayed == 4 {
                        let stale = stale_rendezvous
                            .as_ref()
                            .ok_or("replacement signaling completed without retired grants")?;
                        let (total, grant) = stale.grant_for_endpoint(
                            &peers[0].credential.host_id,
                            &peers[0].credential.boot_id,
                            0,
                        );
                        peers[0]
                            .socket
                            .send(&BrowserAdmissionEgress::WebRtcGrant {
                                protocol: BROWSER_ADMISSION_PROTOCOL,
                                generation: 0,
                                index: 0,
                                total,
                                grant,
                            })
                            .map_err(debug("send stale grant callback"))?;
                        println!("stale-grant-callback generation=0 current=1");
                    }
                }
                BrowserAdmissionIngress::PresenceRenewal { sequence, .. } => {
                    peers[index]
                        .socket
                        .send(&BrowserAdmissionEgress::PresenceAccepted {
                            protocol: BROWSER_ADMISSION_PROTOCOL,
                            sequence,
                            renew_after_millis: RENEW_AFTER_MILLIS,
                            expires_at_millis: LEASE_MILLIS + 1,
                        })
                        .map_err(debug("renew presence"))?;
                }
                _ => return Err(format!("unexpected peer {index} frame")),
            }
        }
        if restart_admitted && active.iter().all(|is_active| !is_active) {
            return Ok(());
        }
    }
}
