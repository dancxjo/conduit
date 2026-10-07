//! One explicitly authorized browser admission window while the Body is lulled.
//! The window bounds admission authorization, not carrier handshake/close latency.
//! The owner may issue a presentation-only browser Mask route on the admitted
//! carrier; the Body workload remains on its authoritative Host.
mod admission;
mod service;
#[cfg(unix)]
mod service_worker;
#[cfg(unix)]
mod speech;
mod transport;
mod wardrobe;
use super::Owner;
use conduit_body::{BodyState, HostPresenceClock, HostPresenceClockScale, HostPresenceTable};
use conduit_core::{HostId, LinkBindingId, SignId};
use conduit_presentation::{
    OwnerFaceSnapshotResponse, MAX_OWNER_FACE_RESPONSE_BYTES, OWNER_FACE_RESPONSE_SCHEMA,
};
use conduit_std_host::browser_admission::{
    BrowserAdmissionEgress as Out, BrowserAdmissionIngress as In,
    BROWSER_ADMISSION_PROTOCOL as PROTOCOL,
};
pub(crate) use service::{
    BrowserAdmittedSnapshot, BrowserCarrierLineEvidence, BrowserWindow, BrowserWindowAuthorization,
};
#[cfg(unix)]
pub(crate) use service_worker::run_service_window;
use std::{
    path::Path,
    time::{Duration, Instant},
};
const LEASE_MS: u64 = 2000;
const RENEW_MS: u64 = 500;
const MAX_CONNECTIONS: usize = 8;
/// A signed admission opens a finite carrier presence independent of the
/// short window in which the proof was authorized. A continuing browser must
/// still renew its separate short lease; the longer session bound admits
/// complete nonvisual readings without expiring an otherwise live carrier.
const MAX_BROWSER_PRESENCE_MILLIS: u64 = 3_600_000;
// Permit a renewal every RENEW_MS plus three bounded interactions per renewal.
const MAX_PRESENCE_FRAMES: usize = (MAX_BROWSER_PRESENCE_MILLIS / RENEW_MS) as usize * 4;

impl Owner {
    pub(crate) fn admit_browser(
        &mut self,
        root: &Path,
        expected_host: &str,
        new_host_key: Option<[u8; 32]>,
        maximum_millis: u64,
    ) -> Result<(), String> {
        if self.session.evidence().body.state != BodyState::Lulled
            || self.session.realization().is_some()
        {
            return Err(
                "browser admission requires a lulled Body without a pending realization".into(),
            );
        }
        if !(1000..=60_000).contains(&maximum_millis)
            || expected_host.is_empty()
            || expected_host.len() > 256
        {
            return Err(
                "browser admission requires an exact Host and a window of 1000..60000 ms".into(),
            );
        }
        let expected = HostId::from(expected_host);
        if expected == self.host.advertisement().host_id {
            return Err("browser participant cannot replace the owner Host".into());
        }
        let known = self.admissions.as_ref().is_some_and(|manager| {
            manager
                .receipts
                .iter()
                .any(|receipt| receipt.credential.host_id == expected)
        });
        if !known && new_host_key.is_none() {
            return Err("first browser admission requires its exact public verifying key".into());
        }
        let listener = transport::Listener::bind()?;
        let clock = Instant::now();
        let deadline = clock + Duration::from_millis(maximum_millis);
        super::super::emit(&serde_json::json!({
            "schema":"conduit.body/browser-admission-window@1", "body_id":self.session.evidence().body_id,
            "expected_host_id":expected, "url":listener.url()?, "maximum_millis":maximum_millis,
            "maximum_connections":MAX_CONNECTIONS, "remote_execution":false,
            "window_scope":"admission-authorization", "strict_elapsed_termination":false,
        }))?;
        for _ in 0..MAX_CONNECTIONS {
            let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                break;
            };
            let Some(mut socket) = listener.accept(remaining, "line/owner-browser")? else {
                break;
            };
            let binding = socket.binding().clone();
            let result = (|| {
                let (credential, observation) = admission::admit(
                    self,
                    &mut socket,
                    &expected,
                    new_host_key,
                    &binding,
                    clock,
                    deadline,
                )?;
                admission::remaining(deadline)?;
                self.persist(root)?; // A credential is never acknowledged ahead of durable membership.
                let snapshot =
                    BrowserAdmittedSnapshot::from_foreground(self, credential, observation)?;
                let presence_clock = Instant::now();
                serve_presence(
                    &snapshot,
                    &mut socket,
                    &binding,
                    presence_clock,
                    presence_clock + Duration::from_millis(MAX_BROWSER_PRESENCE_MILLIS),
                    None,
                    None,
                )
            })();
            // Every exit fences the current browser incarnation, including failed acknowledgement.
            let current = self
                .session
                .evidence()
                .membership
                .parts
                .iter()
                .filter_map(|part| part.current.as_ref())
                .find(|h| h.host_id == expected)
                .cloned();
            if let Some(current) = current {
                let authority = self.host.advertisement();
                self.session
                    .observe_host_lost(
                        &current.host_id,
                        &current.boot_id,
                        &authority.host_id,
                        &authority.boot_id,
                    )
                    .map_err(debug)?;
                self.persist(root)?;
                let _ = socket.send(&Out::BiographyEvidence {
                    protocol: PROTOCOL,
                    evidence: Box::new(self.session.evidence().clone()),
                });
            }
            socket.close();
            super::super::emit(&serde_json::json!({
                "schema":"conduit.body/browser-session-ended@1", "result":result,
                "truth":self.truth(), "remote_execution":false,
            }))?;
        }
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
fn serve_presence(
    snapshot: &BrowserAdmittedSnapshot,
    socket: &mut transport::Socket,
    binding: &LinkBindingId,
    clock: Instant,
    deadline: Instant,
    state_dir: Option<&Path>,
    window_id: Option<&str>,
) -> Result<&'static str, String> {
    let credential = &snapshot.credential;
    let presence_clock = HostPresenceClock::new(
        binding.as_str().into(),
        HostPresenceClockScale::Milliseconds,
        1,
        1,
    )
    .map_err(debug)?;
    let mut presence = HostPresenceTable::new(credential.body_id.clone(), presence_clock, LEASE_MS)
        .map_err(debug)?;
    let membership = &snapshot.biography.membership;
    presence
        .start(
            membership,
            &credential.part_id,
            binding.clone(),
            1,
            now(clock),
            LEASE_MS,
            signal(binding, "started"),
        )
        .map_err(debug)?;
    admission::remaining(deadline)?;
    socket.send(&Out::Admitted {
        protocol: PROTOCOL,
        credential: credential.clone(),
    })?;
    socket.send(&Out::BiographyEvidence {
        protocol: PROTOCOL,
        evidence: snapshot.biography.clone(),
    })?;
    socket.send(&Out::OfferEvidence {
        protocol: PROTOCOL,
        evidence: snapshot.offer.clone(),
    })?;
    acknowledge_presence(socket, &presence)?;
    #[cfg(unix)]
    let mut selected_speech = speech::CarrierSpeech::default();
    for _ in 0..MAX_PRESENCE_FRAMES {
        let lease = &presence.leases[0];
        let Some(window_left) = deadline.checked_duration_since(Instant::now()) else {
            return Ok("window-closed");
        };
        let remaining = lease.expires_at_millis.saturating_sub(now(clock));
        if remaining == 0 {
            return Err("browser presence lease expired".into());
        }
        let received = socket.receive(window_left.min(Duration::from_millis(remaining)));
        if Instant::now() >= deadline {
            return Ok("window-closed");
        }
        if now(clock) >= lease.expires_at_millis {
            return Err("browser presence lease expired".into());
        }
        let (frame, _) = received?;
        match frame {
            In::PresenceRenewal {
                protocol: PROTOCOL,
                credential_id,
                body_id,
                part_id,
                host_id,
                boot_id,
                sequence,
            } if credential_id == credential.credential_id
                && body_id == credential.body_id
                && part_id == credential.part_id
                && host_id == credential.host_id
                && boot_id == credential.boot_id =>
            {
                presence
                    .renew(
                        membership,
                        &part_id,
                        binding,
                        sequence,
                        now(clock),
                        LEASE_MS,
                        signal(binding, &format!("renew-{sequence}")),
                    )
                    .map_err(debug)?;
                acknowledge_presence(socket, &presence)?;
            }
            In::FaceSnapshotRequest {
                protocol: PROTOCOL,
                request,
            } if request.credential_id == credential.credential_id.as_str()
                && request.body_id == credential.body_id
                && request.part_id == credential.part_id
                && request.host_id == credential.host_id
                && request.boot_id == credential.boot_id =>
            {
                // The carrier and lease prove reachability. The serialized
                // owner checks this credential and its current Part again
                // against authoritative admission before projecting a Face.
                let mut fresh_request = request;
                fresh_request.last_seen_revision = None;
                fresh_request.last_seen_identity = None;
                let mut response = state_dir
                    .ok_or_else(|| "owner Face requires the installed service actor".to_string())
                    .and_then(|dir| crate::durable_host_control::face_snapshot(dir, fresh_request))
                    .unwrap_or_else(|error| OwnerFaceSnapshotResponse::Refused {
                        schema: OWNER_FACE_RESPONSE_SCHEMA.into(),
                        code: match error.as_str() {
                            "face-frame-pressure" => "face-frame-pressure",
                            "owner-face-credential-not-admitted" => "face-credential-not-admitted",
                            "owner-face-current-part-unavailable" => {
                                "face-current-part-unavailable"
                            }
                            _ => "face-unavailable",
                        }
                        .into(),
                    });
                if let OwnerFaceSnapshotResponse::Snapshot {
                    presentation,
                    interactions_admitted,
                    route,
                    ..
                } = &mut response
                {
                    #[cfg(unix)]
                    let selected = state_dir
                        .zip(window_id)
                        .ok_or_else(|| {
                            "owner route requires the installed service actor".to_string()
                        })
                        .and_then(|(dir, window)| {
                            let evidence = socket.line_evidence(snapshot, window)?;
                            crate::durable_host_control::browser::mask_route(
                                dir,
                                window,
                                credential.clone(),
                                binding.clone(),
                                evidence,
                            )
                        });
                    #[cfg(not(unix))]
                    let selected = Err::<conduit_presentation::RemoteOwnerMaskRouteSeal, String>(
                        "owner route requires an installed Unix service actor".into(),
                    );
                    match selected {
                        Ok(selected)
                            if selected.face_id == presentation.identity
                                && selected.face_revision == presentation.revision =>
                        {
                            *interactions_admitted = presentation.actions.iter().any(|action| {
                                action.intent == crate::durable_host::owner::clock_interval_action()
                            });
                            *route = Some(Box::new(selected));
                        }
                        _ => {
                            response = OwnerFaceSnapshotResponse::Refused {
                                schema: OWNER_FACE_RESPONSE_SCHEMA.into(),
                                code: "owner-mask-route-unavailable".into(),
                            };
                        }
                    }
                }
                if serde_json::to_vec(&response)
                    .map_err(|error| format!("encode owner Mask route: {error}"))?
                    .len()
                    > MAX_OWNER_FACE_RESPONSE_BYTES
                {
                    response = OwnerFaceSnapshotResponse::Refused {
                        schema: OWNER_FACE_RESPONSE_SCHEMA.into(),
                        code: "face-route-frame-pressure".into(),
                    };
                }
                socket.send(&Out::FaceSnapshotResponse {
                    protocol: PROTOCOL,
                    response,
                })?;
            }
            In::FaceSnapshotRequest {
                protocol: PROTOCOL, ..
            } => {
                socket.send(&Out::FaceSnapshotResponse {
                    protocol: PROTOCOL,
                    response: OwnerFaceSnapshotResponse::Refused {
                        schema: OWNER_FACE_RESPONSE_SCHEMA.into(),
                        code: "face-credential-mismatch".into(),
                    },
                })?;
                return Err("owner Face request differs from this admitted browser carrier".into());
            }
            In::FaceShowAcknowledgement {
                protocol: PROTOCOL,
                request,
                show,
            } if request.credential_id == credential.credential_id.as_str()
                && request.body_id == credential.body_id
                && request.part_id == credential.part_id
                && request.host_id == credential.host_id
                && request.boot_id == credential.boot_id =>
            {
                #[cfg(unix)]
                let result = state_dir
                    .zip(window_id)
                    .ok_or_else(|| "owner Show requires the installed service actor".to_string())
                    .and_then(|(dir, window)| {
                        crate::durable_host_control::browser::acknowledge_show(
                            dir,
                            window,
                            binding.clone(),
                            request,
                            *show,
                        )
                    });
                #[cfg(not(unix))]
                let result =
                    Err::<(), String>("owner Show requires an installed Unix service actor".into());
                socket.send(&Out::FaceShowResponse {
                    protocol: PROTOCOL,
                    accepted: result.is_ok(),
                    code: result.err().map_or_else(String::new, |error| {
                        if error.contains("Stale") || error.contains("stale") {
                            "stale-face-or-show".into()
                        } else {
                            "mask-show-unavailable".into()
                        }
                    }),
                })?;
            }
            In::FaceWardrobeRequest {
                protocol: PROTOCOL,
                request_id,
                request,
                owner_plan_id,
                basis_revision,
                action,
            } => {
                wardrobe::respond(
                    socket,
                    state_dir,
                    window_id,
                    binding,
                    request_id,
                    request,
                    owner_plan_id,
                    basis_revision,
                    action,
                )?;
            }
            In::FaceShowAcknowledgement {
                protocol: PROTOCOL, ..
            } => {
                socket.send(&Out::FaceShowResponse {
                    protocol: PROTOCOL,
                    accepted: false,
                    code: "credential-mismatch".into(),
                })?;
                return Err("owner Show differs from admitted browser carrier".into());
            }
            In::FaceInteractionRequest {
                protocol: PROTOCOL,
                request,
                show,
                interaction,
            } if request.credential_id == credential.credential_id.as_str()
                && request.body_id == credential.body_id
                && request.part_id == credential.part_id
                && request.host_id == credential.host_id
                && request.boot_id == credential.boot_id =>
            {
                let result = state_dir
                    .ok_or_else(|| {
                        "owner interaction requires the installed service actor".to_string()
                    })
                    .and_then(|dir| {
                        crate::durable_host_control::submit_browser_face_interaction(
                            dir,
                            window_id.ok_or("browser-route-window-unavailable")?,
                            binding.clone(),
                            request,
                            *show,
                            interaction,
                        )
                    });
                let (accepted, code) = match result {
                    Ok(_) => (true, String::new()),
                    Err(error) => (
                        false,
                        if error == "control-outcome-unknown" {
                            "control-outcome-unknown"
                        } else if error == "body-play-active"
                            || error.contains("retired Play")
                            || error == "clock-play-must-lull"
                        {
                            "clock-play-must-lull"
                        } else if error.contains("StaleFace")
                            || error.contains("StaleShow")
                            || error.contains("stale owner Mask Show")
                        {
                            "stale-face-or-show"
                        } else if error.contains("browser-mask-show-refused")
                            || error.contains("browser-mask-route-not-selected")
                            || error.contains("browser-route-carrier-mismatch")
                        {
                            "mask-route-unavailable"
                        } else if error.contains("UnavailableAction") {
                            "action-unavailable"
                        } else if error.contains("RefusedAction") {
                            "action-refused"
                        } else if error == "owner-face-credential-not-admitted" {
                            "credential-not-admitted"
                        } else if error == "owner-face-current-part-unavailable" {
                            "current-part-unavailable"
                        } else {
                            "interaction-refused"
                        }
                        .into(),
                    ),
                };
                socket.send(&Out::FaceInteractionResponse {
                    protocol: PROTOCOL,
                    accepted,
                    code,
                })?;
            }
            In::FaceInteractionRequest {
                protocol: PROTOCOL, ..
            } => {
                socket.send(&Out::FaceInteractionResponse {
                    protocol: PROTOCOL,
                    accepted: false,
                    code: "credential-mismatch".into(),
                })?;
                return Err("owner interaction differs from admitted browser carrier".into());
            }
            #[cfg(unix)]
            frame @ (In::SelectedSpeechStart {
                protocol: PROTOCOL, ..
            }
            | In::SelectedSpeechStatus {
                protocol: PROTOCOL, ..
            }
            | In::SelectedSpeechStop {
                protocol: PROTOCOL, ..
            }) => {
                selected_speech.handle(frame, snapshot, socket, binding, state_dir, window_id)?;
            }
            In::OfferDisclosureRequest {
                protocol: PROTOCOL,
                credential_id,
                body_id,
                part_id,
                host_id,
                boot_id,
                request,
            } if credential_id == credential.credential_id
                && body_id == credential.body_id
                && part_id == credential.part_id
                && host_id == credential.host_id
                && boot_id == credential.boot_id =>
            {
                socket.send(&service::planning_offer_response(
                    state_dir, window_id, credential, request,
                )?)?;
            }
            In::OfferDisclosureRequest {
                protocol: PROTOCOL, ..
            } => {
                socket.send(&Out::Refused {
                    protocol: PROTOCOL,
                    code: "credential-mismatch".into(),
                })?;
                return Err("planning offer differs from admitted browser carrier".into());
            }
            In::WebRtcGrantRequest {
                protocol: PROTOCOL,
                credential_id,
                body_id,
                part_id,
                host_id,
                boot_id,
                generation,
                index,
            } if credential_id == credential.credential_id
                && body_id == credential.body_id
                && part_id == credential.part_id
                && host_id == credential.host_id
                && boot_id == credential.boot_id =>
            {
                // Membership supplies no planned remote Line or execution authority.
                socket.send(&Out::WebRtcGrant {
                    protocol: PROTOCOL,
                    generation,
                    index,
                    total: 0,
                    grant: None,
                })?;
            }
            In::PresenceLeave {
                protocol: PROTOCOL,
                credential_id,
                body_id,
                part_id,
                host_id,
                boot_id,
                sequence,
            } if sequence > lease.sequence
                && credential_id == credential.credential_id
                && body_id == credential.body_id
                && part_id == credential.part_id
                && host_id == credential.host_id
                && boot_id == credential.boot_id =>
            {
                return Ok("browser-left")
            }
            _ => return Err("browser session refused unexpected or stale credential frame".into()),
        }
    }
    Err("browser presence frame budget exhausted".into())
}
fn acknowledge_presence(
    socket: &mut transport::Socket,
    presence: &HostPresenceTable,
) -> Result<(), String> {
    let lease = &presence.leases[0];
    socket.send(&Out::PresenceAccepted {
        protocol: PROTOCOL,
        sequence: lease.sequence,
        renew_after_millis: RENEW_MS,
        expires_at_millis: lease.expires_at_millis,
    })
}
fn now(clock: Instant) -> u64 {
    clock.elapsed().as_millis() as u64
}
fn nonce() -> Result<[u8; 32], String> {
    let mut bytes = [0; 32];
    getrandom::fill(&mut bytes).map_err(|e| e.to_string())?;
    Ok(bytes)
}
fn signal(binding: &LinkBindingId, stage: &str) -> SignId {
    SignId::from(format!("sign/{}/{stage}", binding.as_str()))
}
fn debug(error: impl std::fmt::Debug) -> String {
    format!("browser participant: {error:?}")
}
