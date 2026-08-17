//! Bounded two-browser admission and WebRTC rendezvous proof fixture.

#[path = "browser_webrtc_rendezvous_probe/planning.rs"]
mod planning;

use conduit_body::{
    AdmissionManager, AdmissionSigns, AmbientAdmissionProof, Body, BodyMembership,
    CandidateInventory, CandidateObservation, DiscoveryProofId, HostPresenceClock,
    HostPresenceClockScale, HostPresenceTable, MembershipCredential,
};
use conduit_core::{CheckedFormId, LinkBindingId, SignId, SourceDocumentId};
use conduit_std_host::browser_admission::{
    BrowserAdmissionEgress, BrowserAdmissionIngress, BrowserAdmissionListener,
    BrowserAdmissionSocket, BrowserAdmissionSocketError, BrowserWebRtcRendezvous,
    BROWSER_ADMISSION_PROTOCOL,
};
use conduit_std_host::websocket::{NativeWebSocketError, NativeWebSocketError::Transport};
use planning::{binding, session_basis};
use std::io::ErrorKind;
use std::time::Duration;

const LEASE_MILLIS: u64 = 60_000;
const RENEW_AFTER_MILLIS: u64 = 30_000;

struct Peer {
    socket: BrowserAdmissionSocket,
    credential: MembershipCredential,
    session_id: LinkBindingId,
}

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
    let mut active = [true, true];
    let mut grant_generation = 0_u16;
    let mut stale_rendezvous = None;
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
                    presence
                        .lose_session(
                            &mut membership,
                            &peers[index].credential.part_id,
                            &peers[index].session_id,
                            2,
                            SignId::from(format!(
                                "sign/browser-webrtc-rendezvous-probe/lost-{index}"
                            )),
                        )
                        .map_err(debug("lose peer presence"))?;
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
                    active[index] = false;
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
        if !active[0] && !active[1] {
            return Ok(());
        }
    }
}

fn admit(
    mut socket: BrowserAdmissionSocket,
    index: usize,
    candidates: &mut CandidateInventory,
    admission: &mut AdmissionManager,
    membership: &mut BodyMembership,
) -> Result<Peer, String> {
    let (frame, encoded_bytes) = socket.receive_with_size().map_err(debug("advertise"))?;
    let BrowserAdmissionIngress::Advertise {
        advertisement,
        friendly_label,
        verifying_key,
        freshness_sequence,
        ..
    } = frame
    else {
        return Err("peer did not advertise".into());
    };
    let verifying_key = verifying_key
        .try_into()
        .map_err(|_| "browser verifying key was not 32 bytes")?;
    let proof_id = format!("proof/probe/{index}");
    let candidate = candidates
        .observe(CandidateObservation {
            advertisement,
            friendly_label,
            observed_binding_id: LinkBindingId::from(format!("probe/admission/{index}")),
            observation_sign_id: SignId::from(format!("sign/probe/observed/{index}")),
            proof_id: DiscoveryProofId::bind(&proof_id).map_err(debug("proof id"))?,
            freshness_sequence,
            encoded_bytes,
        })
        .map_err(debug("observe"))?;
    let challenge = admission
        .begin_ambient(
            candidates,
            &candidate,
            verifying_key,
            [index as u8 + 1; 32],
            1_000,
            2_000,
            SignId::from(format!("sign/probe/requested/{index}")),
        )
        .map_err(debug("challenge"))?;
    socket
        .send(&BrowserAdmissionEgress::Challenge {
            protocol: BROWSER_ADMISSION_PROTOCOL,
            challenge,
        })
        .map_err(debug("send challenge"))?;
    let BrowserAdmissionIngress::AmbientProof {
        admission_id,
        body_id,
        host_id,
        boot_id,
        nonce,
        signature,
        ..
    } = socket.receive().map_err(debug("proof"))?
    else {
        return Err("peer did not prove admission".into());
    };
    let proof = AmbientAdmissionProof {
        admission_id,
        body_id,
        host_id,
        boot_id,
        nonce: nonce.try_into().map_err(|_| "invalid proof nonce")?,
        signature: signature
            .try_into()
            .map_err(|_| "invalid proof signature")?,
    };
    let credential = admission
        .complete_ambient(
            candidates,
            membership,
            &proof,
            1_100,
            AdmissionSigns {
                part_admitted: SignId::from(format!("sign/probe/admitted/{index}")),
                host_attached: SignId::from(format!("sign/probe/attached/{index}")),
                candidate_admitted: SignId::from(format!("sign/probe/candidate/{index}")),
            },
        )
        .map_err(debug("complete"))?;
    socket
        .send(&BrowserAdmissionEgress::Admitted {
            protocol: BROWSER_ADMISSION_PROTOCOL,
            credential: credential.clone(),
        })
        .map_err(debug("send admitted"))?;
    Ok(Peer {
        socket,
        session_id: LinkBindingId::from(format!("probe/presence/{index}")),
        credential,
    })
}

fn exact_credential(
    expected: &MembershipCredential,
    credential_id: &conduit_body::MembershipCredentialId,
    body_id: &conduit_body::BodyId,
    part_id: &conduit_body::PartId,
    host_id: &conduit_core::HostId,
    boot_id: &conduit_core::BootId,
) -> Result<(), String> {
    if credential_id == &expected.credential_id
        && body_id == &expected.body_id
        && part_id == &expected.part_id
        && host_id == &expected.host_id
        && boot_id == &expected.boot_id
    {
        Ok(())
    } else {
        Err("stale membership credential".into())
    }
}

fn frame_kind(frame: &BrowserAdmissionIngress) -> &'static str {
    match frame {
        BrowserAdmissionIngress::PresenceRenewal { .. } => "presence-renewal",
        BrowserAdmissionIngress::WebRtcGrantRequest { .. } => "web-rtc-grant-request",
        BrowserAdmissionIngress::WebRtcSignal { .. } => "web-rtc-signal",
        _ => "unexpected",
    }
}

fn debug<T: core::fmt::Debug>(label: &'static str) -> impl FnOnce(T) -> String {
    move |error| format!("{label}: {error:?}")
}
