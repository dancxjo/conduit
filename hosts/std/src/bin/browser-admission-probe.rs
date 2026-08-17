//! One bounded live browser admission proof server for conformance tests.

#[path = "browser-admission-probe/return_admission.rs"]
mod return_admission;
#[path = "browser-admission-probe/return_session.rs"]
mod return_session;

use conduit_body::{
    AdmissionManager, AdmissionSigns, AmbientAdmissionProof, Body, BodyMembership,
    CandidateInventory, CandidateObservation, DiscoveryProofId, HostPresenceClock,
    HostPresenceClockScale, HostPresenceRefusal, HostPresenceTable,
};
use conduit_core::{CheckedFormId, LinkBindingId, SignId, SourceDocumentId};
use conduit_std_host::browser_admission::{
    BrowserAdmissionEgress, BrowserAdmissionIngress, BrowserAdmissionListener,
    BrowserAdmissionSocketError, BROWSER_ADMISSION_PROTOCOL,
};
use conduit_std_host::websocket::{NativeWebSocketError, NativeWebSocketError::Transport};
use std::io::ErrorKind;
use std::time::{Duration, Instant};

use return_admission::accept_return;

const PRESENCE_LEASE_MILLIS: u64 = 2_000;
const PRESENCE_RENEW_AFTER_MILLIS: u64 = 500;

fn main() -> Result<(), String> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let live_presence = arguments.iter().any(|argument| argument == "--presence");
    let reconnect = arguments.iter().any(|argument| argument == "--reconnect");
    let clock = Instant::now();
    let body = Body::born(
        SourceDocumentId::from("source/browser-admission-probe"),
        CheckedFormId::from("checked/browser-admission-probe"),
        1,
        SignId::from("sign/browser-admission-probe/body-born"),
    )
    .map_err(|error| format!("Body birth: {error:?}"))?;
    let mut membership = BodyMembership::new(body.body_id.clone())
        .map_err(|error| format!("membership: {error:?}"))?;
    let mut candidates = CandidateInventory::new(body.body_id.clone())
        .map_err(|error| format!("candidate inventory: {error:?}"))?;
    let mut admission = AdmissionManager::new(body.body_id.clone())
        .map_err(|error| format!("admission manager: {error:?}"))?;
    let listener = BrowserAdmissionListener::bind_loopback()
        .map_err(|error| format!("bind admission Line: {error:?}"))?;
    println!(
        "{}",
        listener.url().map_err(|error| format!("URL: {error:?}"))?
    );
    let mut socket = listener
        .accept()
        .map_err(|error| format!("accept browser: {error:?}"))?;
    let (frame, encoded_bytes) = socket
        .receive_with_size()
        .map_err(|error| format!("receive advertisement: {error:?}"))?;
    let BrowserAdmissionIngress::Advertise {
        advertisement,
        friendly_label,
        verifying_key,
        freshness_sequence,
        ..
    } = frame
    else {
        return Err("first browser admission frame was not an advertisement".into());
    };
    let verifying_key: [u8; 32] = verifying_key
        .try_into()
        .map_err(|_| "browser verifying key was not 32 bytes")?;
    let candidate = candidates
        .observe(CandidateObservation {
            advertisement,
            friendly_label,
            observed_binding_id: LinkBindingId::from("line/browser-admission-probe"),
            observation_sign_id: SignId::from("sign/browser-admission-probe/observed"),
            proof_id: DiscoveryProofId::bind("proof/browser-admission-probe/loopback")
                .map_err(|error| format!("discovery proof: {error:?}"))?,
            freshness_sequence,
            encoded_bytes,
        })
        .map_err(|error| format!("observe candidate: {error:?}"))?;
    // Running this conformance probe is the explicit admission decision. Mere
    // reachability above created only an inert candidate.
    let challenge = admission
        .begin_ambient(
            &mut candidates,
            &candidate,
            verifying_key,
            [9; 32],
            1_000,
            2_000,
            SignId::from("sign/browser-admission-probe/requested"),
        )
        .map_err(|error| format!("begin admission: {error:?}"))?;
    socket
        .send(&BrowserAdmissionEgress::Challenge {
            protocol: BROWSER_ADMISSION_PROTOCOL,
            challenge,
        })
        .map_err(|error| format!("send challenge: {error:?}"))?;
    let proof = match socket
        .receive()
        .map_err(|error| format!("receive proof: {error:?}"))?
    {
        BrowserAdmissionIngress::AmbientProof {
            admission_id,
            body_id,
            host_id,
            boot_id,
            nonce,
            signature,
            ..
        } => AmbientAdmissionProof {
            admission_id,
            body_id,
            host_id,
            boot_id,
            nonce: nonce.try_into().map_err(|_| "invalid proof nonce")?,
            signature: signature
                .try_into()
                .map_err(|_| "invalid proof signature")?,
        },
        _ => return Err("second browser admission frame was not an ambient proof".into()),
    };
    let credential = admission
        .complete_ambient(
            &mut candidates,
            &mut membership,
            &proof,
            1_100,
            AdmissionSigns {
                part_admitted: SignId::from("sign/browser-admission-probe/part-admitted"),
                host_attached: SignId::from("sign/browser-admission-probe/host-attached"),
                candidate_admitted: SignId::from("sign/browser-admission-probe/candidate-admitted"),
            },
        )
        .map_err(|error| format!("complete admission: {error:?}"))?;
    socket
        .send(&BrowserAdmissionEgress::Admitted {
            protocol: BROWSER_ADMISSION_PROTOCOL,
            credential: credential.clone(),
        })
        .map_err(|error| format!("send credential: {error:?}"))?;
    println!(
        "admitted body={} part={} host={} boot={} candidates={} members={}",
        credential.body_id.as_str(),
        credential.part_id.as_str(),
        credential.host_id.as_str(),
        credential.boot_id.as_str(),
        candidates.candidates.len(),
        membership.parts.len()
    );
    if !live_presence {
        return Ok(());
    }
    let session_binding = LinkBindingId::from("line/browser-admission-probe/session-1");
    let presence_clock = HostPresenceClock::new(
        format!(
            "clock/browser-admission-probe/{}",
            credential.boot_id.as_str()
        ),
        HostPresenceClockScale::Milliseconds,
        1,
        1,
    )
    .map_err(|error| format!("presence clock: {error:?}"))?;
    let mut presence =
        HostPresenceTable::new(body.body_id.clone(), presence_clock, PRESENCE_LEASE_MILLIS)
            .map_err(|error| format!("presence table: {error:?}"))?;
    let observed_at_millis = monotonic_millis(clock)?;
    presence
        .start(
            &membership,
            &credential.part_id,
            session_binding.clone(),
            1,
            observed_at_millis,
            PRESENCE_LEASE_MILLIS,
            SignId::from("sign/browser-admission-probe/presence-started"),
        )
        .map_err(|error| format!("start presence: {error:?}"))?;
    send_presence_accepted(&mut socket, 1, presence.leases[0].expires_at_millis)?;
    'presence: loop {
        let now = monotonic_millis(clock)?;
        let remaining = presence.leases[0].expires_at_millis.saturating_sub(now);
        socket
            .set_read_timeout(Some(Duration::from_millis(remaining.max(1))))
            .map_err(|error| format!("set presence deadline: {error:?}"))?;
        match socket.receive() {
            Ok(BrowserAdmissionIngress::PresenceRenewal {
                credential_id,
                body_id,
                part_id,
                host_id,
                boot_id,
                sequence,
                ..
            }) => {
                if credential_id != credential.credential_id
                    || body_id != credential.body_id
                    || part_id != credential.part_id
                    || host_id != credential.host_id
                    || boot_id != credential.boot_id
                {
                    socket
                        .send(&BrowserAdmissionEgress::Refused {
                            protocol: BROWSER_ADMISSION_PROTOCOL,
                            code: "stale-membership-credential".into(),
                        })
                        .map_err(|error| format!("send renewal refusal: {error:?}"))?;
                    presence
                        .lose_session(
                            &mut membership,
                            &credential.part_id,
                            &session_binding,
                            monotonic_millis(clock)?,
                            SignId::from("sign/browser-admission-probe/refused-session"),
                        )
                        .map_err(|error| format!("lose refused session: {error:?}"))?;
                    println!("unavailable reason=refused sequence={sequence}");
                    return Ok(());
                }
                let observed_at_millis = monotonic_millis(clock)?;
                if let Err(refusal) = presence.renew(
                    &membership,
                    &credential.part_id,
                    &session_binding,
                    sequence,
                    observed_at_millis,
                    PRESENCE_LEASE_MILLIS,
                    SignId::from(format!("sign/browser-admission-probe/presence-{sequence}")),
                ) {
                    socket
                        .send(&BrowserAdmissionEgress::Refused {
                            protocol: BROWSER_ADMISSION_PROTOCOL,
                            code: presence_refusal_code(refusal).into(),
                        })
                        .map_err(|error| format!("send presence refusal: {error:?}"))?;
                    presence
                        .lose_session(
                            &mut membership,
                            &credential.part_id,
                            &session_binding,
                            monotonic_millis(clock)?,
                            SignId::from("sign/browser-admission-probe/renewal-refused"),
                        )
                        .map_err(|error| format!("lose refused renewal session: {error:?}"))?;
                    println!(
                        "unavailable reason={} sequence={sequence}",
                        presence_refusal_code(refusal)
                    );
                    return Ok(());
                }
                send_presence_accepted(
                    &mut socket,
                    sequence,
                    presence.leases[0].expires_at_millis,
                )?;
                println!("renewed sequence={sequence}");
                if reconnect {
                    socket
                        .close()
                        .map_err(|error| format!("end first presence session: {error:?}"))?;
                    presence
                        .lose_session(
                            &mut membership,
                            &credential.part_id,
                            &session_binding,
                            monotonic_millis(clock)?,
                            SignId::from("sign/browser-admission-probe/reconnect-session-lost"),
                        )
                        .map_err(|error| format!("record reconnect session loss: {error:?}"))?;
                    println!("unavailable reason=session-lost-for-return sequence={sequence}");
                    break 'presence;
                }
            }
            Ok(BrowserAdmissionIngress::WebRtcGrantRequest {
                credential_id,
                body_id,
                part_id,
                host_id,
                boot_id,
                generation,
                index,
                ..
            }) => {
                if credential_id != credential.credential_id
                    || body_id != credential.body_id
                    || part_id != credential.part_id
                    || host_id != credential.host_id
                    || boot_id != credential.boot_id
                {
                    socket
                        .send(&BrowserAdmissionEgress::Refused {
                            protocol: BROWSER_ADMISSION_PROTOCOL,
                            code: "stale-membership-credential".into(),
                        })
                        .map_err(|error| format!("send grant refusal: {error:?}"))?;
                    return Ok(());
                }
                socket
                    .send(&BrowserAdmissionEgress::WebRtcGrant {
                        protocol: BROWSER_ADMISSION_PROTOCOL,
                        generation,
                        index,
                        total: 0,
                        grant: None,
                    })
                    .map_err(|error| format!("send empty grant result: {error:?}"))?;
                println!("webrtc-grant generation={generation} index={index} total=0");
            }
            Ok(_) => return Err("post-admission frame was not a presence renewal".into()),
            Err(BrowserAdmissionSocketError::Transport(Transport(
                ErrorKind::TimedOut | ErrorKind::WouldBlock,
            ))) => {
                let observed_at_millis = monotonic_millis(clock)?;
                presence
                    .expire(
                        &mut membership,
                        &credential.part_id,
                        observed_at_millis,
                        SignId::from("sign/browser-admission-probe/presence-expired"),
                    )
                    .map_err(|error| format!("expire presence: {error:?}"))?;
                println!(
                    "unavailable reason=expired sequence={}",
                    presence.leases[0].sequence
                );
                return Ok(());
            }
            Err(BrowserAdmissionSocketError::Transport(
                NativeWebSocketError::Disconnected | NativeWebSocketError::Transport(_),
            )) => {
                presence
                    .lose_session(
                        &mut membership,
                        &credential.part_id,
                        &session_binding,
                        monotonic_millis(clock)?,
                        SignId::from("sign/browser-admission-probe/session-lost"),
                    )
                    .map_err(|error| format!("lose browser session: {error:?}"))?;
                println!(
                    "unavailable reason=session-lost sequence={}",
                    presence.leases[0].sequence
                );
                if reconnect {
                    break 'presence;
                }
                return Ok(());
            }
            Err(error) => return Err(format!("receive presence renewal: {error:?}")),
        }
    }
    accept_return(
        &listener,
        &mut admission,
        &mut presence,
        &mut membership,
        &credential,
        clock,
        PRESENCE_LEASE_MILLIS,
        PRESENCE_RENEW_AFTER_MILLIS,
    )
}

fn monotonic_millis(clock: Instant) -> Result<u64, String> {
    u64::try_from(clock.elapsed().as_millis())
        .map_err(|_| "monotonic presence clock overflowed".into())
}

fn send_presence_accepted(
    socket: &mut conduit_std_host::browser_admission::BrowserAdmissionSocket,
    sequence: u64,
    expires_at_millis: u64,
) -> Result<(), String> {
    socket
        .send(&BrowserAdmissionEgress::PresenceAccepted {
            protocol: BROWSER_ADMISSION_PROTOCOL,
            sequence,
            renew_after_millis: PRESENCE_RENEW_AFTER_MILLIS,
            expires_at_millis,
        })
        .map_err(|error| format!("send presence acceptance: {error:?}"))
}

fn presence_refusal_code(refusal: HostPresenceRefusal) -> &'static str {
    match refusal {
        HostPresenceRefusal::WrongBody => "wrong-body",
        HostPresenceRefusal::UnknownPart => "unknown-part",
        HostPresenceRefusal::RevokedPart => "revoked-part",
        HostPresenceRefusal::HostUnavailable => "host-unavailable",
        HostPresenceRefusal::WrongHost => "wrong-host",
        HostPresenceRefusal::StaleBoot => "stale-boot",
        HostPresenceRefusal::StaleOfferGeneration => "stale-offer-generation",
        HostPresenceRefusal::StaleMembershipProof => "stale-membership-proof",
        HostPresenceRefusal::WrongSession => "wrong-session",
        HostPresenceRefusal::StaleSequence => "stale-sequence",
        HostPresenceRefusal::ClockRegressed => "clock-regressed",
        HostPresenceRefusal::InvalidClock => "invalid-clock",
        HostPresenceRefusal::LeaseDurationZero => "lease-duration-zero",
        HostPresenceRefusal::LeaseDurationTooLong => "lease-duration-too-long",
        HostPresenceRefusal::LeaseDeadlineOverflow => "lease-deadline-overflow",
        HostPresenceRefusal::LeaseStillCurrent => "lease-still-current",
        HostPresenceRefusal::PresenceCapacityExhausted => "presence-capacity-exhausted",
        HostPresenceRefusal::RevisionOverflow => "revision-overflow",
        HostPresenceRefusal::MalformedState => "malformed-presence-state",
        HostPresenceRefusal::EmptyIdentity => "empty-identity",
        HostPresenceRefusal::IdentityTooLong => "identity-too-long",
        HostPresenceRefusal::Membership(_) => "membership-refused",
    }
}
