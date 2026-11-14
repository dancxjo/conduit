//! A finite current-member encounter on one pinned TLS carrier.
use super::*;
use conduit_body::MembershipCredential;
use conduit_std_host::browser_admission::{
    BrowserAdmissionEgress as Out, BrowserAdmissionIngress as In,
    BROWSER_ADMISSION_PROTOCOL as PROTOCOL,
};
use std::{cell::RefCell, path::Path};

#[path = "member_carrier_client/presence_deadline.rs"]
mod presence_deadline;
use presence_deadline::PresenceDeadline;

struct Carrier {
    line: SecureWebSocketClientLine,
    presence: PresenceDeadline,
    frames: usize,
    closed: bool,
}

pub(in crate::durable_host::presentation) struct MemberRoute {
    carrier: RefCell<Carrier>,
    request: OwnerFaceSnapshotRequest,
    credential: MembershipCredential,
    deadline: Instant,
    descriptor_expiry: u64,
    admitted: AdmittedSnapshot,
}

impl MemberRoute {
    /// Retire this finite carrier before exhausting its admitted frame/deadline
    /// budget. A replacement must authenticate independently; this extends none
    /// of the old presence, route or Show authority.
    #[cfg(all(target_os = "linux", feature = "native-display"))]
    pub(in crate::durable_host::presentation) fn renewal_due(&self) -> bool {
        renewal_due(self.deadline, Instant::now(), self.carrier.borrow().frames)
            || crate::durable_host::current_time_millis().map_or(true, |now| {
                self.descriptor_expiry.saturating_sub(now) <= 3_000
            })
    }
    #[cfg(all(target_os = "linux", feature = "native-display"))]
    pub(in crate::durable_host::presentation) fn connect_initialized(
        runtime: &mut crate::durable_host_control::DurableHostRuntime,
        status: &mut super::super::RuntimeStatus,
        state: &Path,
        encounter: crate::durable_host::routed_join::MemberEncounter,
    ) -> Result<(Self, Snapshot), String> {
        use crate::durable_host::routed_join::MEMBER_ROUTE_SCHEMA;
        let started = Instant::now();
        let installation =
            crate::durable_host::read_installation(&state.join("installation.json"))?;
        let prior = crate::durable_host::membership::load_joined_credential(state, &installation)?;
        let actual = runtime.truth().advertisement;
        let candidate = &encounter.candidate;
        let now = crate::durable_host::current_time_millis()?;
        let checks = [
            (encounter.schema == MEMBER_ROUTE_SCHEMA, "schema"),
            (encounter.body_id == prior.body_id, "body"),
            (encounter.part_id == prior.part_id, "part"),
            (encounter.host_id == prior.host_id, "member-host"),
            (actual.host_id == prior.host_id, "runtime-host"),
            (actual.host_id.as_str() == status.host_id, "published-host"),
            (actual.boot_id.as_str() == status.boot_id, "published-boot"),
            (
                candidate.line_family == conduit_body::RendezvousLineFamily::AuthenticatedTlsStream,
                "line-family",
            ),
            (candidate.expires_at_millis > now, "expired"),
            (
                (1..=30_000).contains(&candidate.attempt_timeout_millis),
                "timeout",
            ),
            ((1..=2).contains(&candidate.maximum_attempts), "attempts"),
        ];
        if let Some((_, boundary)) = checks.iter().find(|(valid, _)| !valid) {
            return Err(format!("member-encounter-refused:{boundary}"));
        }
        let credential = if prior.boot_id != actual.boot_id {
            let (mut old_line, credential) =
                bounded_signed_return(state, &prior, &actual, candidate)?;
            // PartReturn and current-member Resume are distinct protocol entrances.
            // The old return carrier cannot accidentally receive Resume frames.
            crate::durable_host::membership::retain_returned_membership(
                state,
                &prior,
                &credential,
                &actual,
            )?;
            if let Err(error) = old_line.close() {
                eprintln!("PartReturn carrier close unacknowledged: {error:?}");
            }
            drop(old_line);
            credential
        } else {
            prior
        };
        status.body_id = Some(credential.body_id.as_str().into());
        status.offer_generation = actual.offer_generation.0;
        crate::durable_host::write_json_atomic(&state.join("runtime.json"), status)?;
        let (mut route, snapshot) = Self::connect_current(state, &credential, &actual, &encounter)?;
        route.deadline = route.deadline.min(started + Duration::from_secs(60));
        route.check()?;
        Ok((route, snapshot))
    }

    pub(in crate::durable_host::presentation) fn connect_current(
        state: &Path,
        credential: &MembershipCredential,
        actual: &conduit_core::HostAdvertisement,
        encounter: &crate::durable_host::routed_join::MemberEncounter,
    ) -> Result<(Self, Snapshot), String> {
        use crate::durable_host::routed_join::{current_member, MEMBER_ROUTE_SCHEMA};
        let started = Instant::now();
        let candidate = &encounter.candidate;
        if encounter.schema != MEMBER_ROUTE_SCHEMA
            || encounter.body_id != credential.body_id
            || encounter.part_id != credential.part_id
            || encounter.host_id != credential.host_id
            || actual.host_id != credential.host_id
            || actual.boot_id != credential.boot_id
            || candidate.line_family != conduit_body::RendezvousLineFamily::AuthenticatedTlsStream
            || candidate.expires_at_millis <= crate::durable_host::current_time_millis()?
            || !(1..=30_000).contains(&candidate.attempt_timeout_millis)
            || !(1..=2).contains(&candidate.maximum_attempts)
        {
            return Err(
                "current-member encounter differs from credential and actual Host/Boot".into(),
            );
        }
        let deadline = started + Duration::from_secs(60);
        let authentication_deadline =
            started + Duration::from_millis(u64::from(candidate.attempt_timeout_millis.min(3_000)));
        let mut line = connect(
            candidate,
            Duration::from_millis(u64::from(candidate.attempt_timeout_millis.min(3_000))),
        )?;
        line.replace_io_deadline(authentication_deadline.min(deadline))
            .map_err(|e| format!("member authentication deadline:{e:?}"))?;
        line.set_read_timeout(Some(
            deadline
                .saturating_duration_since(Instant::now())
                .min(Duration::from_secs(2)),
        ))
        .map_err(|e| format!("member carrier timeout:{e:?}"))?;
        let anchor = Instant::now();
        current_member::resume(state, credential, actual, encounter, &mut line)?;
        let mut carrier = Carrier {
            line,
            presence: PresenceDeadline::initial(anchor),
            frames: 0,
            closed: false,
        };
        carrier.accept_presence(1, anchor, deadline)?;
        let request = request(credential);
        let snapshot = carrier.face(&request, deadline)?;
        let admitted = AdmittedSnapshot::bind(&snapshot, &request)?;
        Ok((
            Self {
                carrier: RefCell::new(carrier),
                request,
                credential: credential.clone(),
                deadline,
                descriptor_expiry: candidate.expires_at_millis,
                admitted,
            },
            snapshot,
        ))
    }

    fn check(&self) -> Result<(), String> {
        let carrier = self.carrier.borrow();
        if carrier.closed
            || Instant::now() >= self.deadline
            || Instant::now() >= carrier.presence.deadline
            || crate::durable_host::current_time_millis()? >= self.descriptor_expiry
        {
            return Err("current-member presentation encounter expired".into());
        }
        Ok(())
    }

    pub(in crate::durable_host::presentation) fn poll_presence(&self) -> Result<(), String> {
        self.check()?;
        let mut carrier = self.carrier.borrow_mut();
        if Instant::now() < carrier.presence.renew_at {
            return Ok(());
        }
        let sequence = carrier
            .presence
            .sequence
            .checked_add(1)
            .ok_or("member sequence exhausted")?;
        let request = &self.request;
        let frame = In::PresenceRenewal {
            protocol: PROTOCOL,
            credential_id: self.credential.credential_id.clone(),
            body_id: request.body_id.clone(),
            part_id: request.part_id.clone(),
            host_id: request.host_id.clone(),
            boot_id: request.boot_id.clone(),
            sequence,
        };
        let anchor = Instant::now();
        carrier.send(&frame, self.deadline)?;
        carrier.accept_presence(sequence, anchor, self.deadline)
    }

    pub(in crate::durable_host::presentation) fn refresh(&mut self) -> Result<Snapshot, String> {
        self.poll_presence()?;
        let next = self
            .carrier
            .borrow_mut()
            .face(&self.request, self.deadline)?;
        let admitted = AdmittedSnapshot::bind(&next, &self.request)?;
        self.admitted = admitted;
        self.check()?;
        Ok(next)
    }

    pub(in crate::durable_host::presentation) fn acknowledge(
        &self,
        show: &MaskShow,
    ) -> Result<(), String> {
        self.admitted.validate_show(show)?;
        self.poll_presence()?;
        let mut carrier = self.carrier.borrow_mut();
        carrier.send(
            &In::FaceShowAcknowledgement {
                protocol: PROTOCOL,
                request: self.request.clone(),
                show: Box::new(show.clone()),
            },
            self.deadline,
        )?;
        match carrier.receive(self.deadline)? {
            Out::FaceShowResponse {
                protocol: PROTOCOL,
                accepted: true,
                ..
            } => Ok(()),
            Out::FaceShowResponse { code, .. } => Err(format!("member Show refused:{code}")),
            _ => Err("member Show response differs from selected carrier".into()),
        }
    }

    #[cfg(test)]
    pub(in crate::durable_host::presentation) fn leave(&self) -> Result<(), String> {
        self.check()?;
        let mut carrier = self.carrier.borrow_mut();
        let request = &self.request;
        let sequence = carrier
            .presence
            .sequence
            .checked_add(1)
            .ok_or("member sequence exhausted")?;
        carrier.send(
            &In::PresenceLeave {
                protocol: PROTOCOL,
                credential_id: self.credential.credential_id.clone(),
                body_id: request.body_id.clone(),
                part_id: request.part_id.clone(),
                host_id: request.host_id.clone(),
                boot_id: request.boot_id.clone(),
                sequence,
            },
            self.deadline,
        )?;
        carrier.closed = true;
        carrier
            .line
            .close()
            .map_err(|e| format!("member carrier close:{e:?}"))
    }
}

#[cfg(any(test, all(target_os = "linux", feature = "native-display")))]
fn renewal_due(deadline: Instant, now: Instant, frames: usize) -> bool {
    frames >= 96 || deadline.saturating_duration_since(now) <= Duration::from_secs(3)
}

#[cfg(all(target_os = "linux", feature = "native-display"))]
impl Drop for MemberRoute {
    fn drop(&mut self) {
        let carrier = self.carrier.get_mut();
        carrier.closed = true;
        // Closing a transport does not withdraw the member's attachment.
        let _ = carrier.line.close();
    }
}

#[cfg(test)]
#[test]
fn finite_member_carrier_renews_before_exhaustion_without_extending_deadline() {
    let now = Instant::now();
    let deadline = now + Duration::from_secs(60);
    assert!(!renewal_due(deadline, now, 95));
    assert!(renewal_due(deadline, now, 96));
    assert!(renewal_due(deadline, now + Duration::from_secs(57), 0));
    assert!(renewal_due(deadline, now + Duration::from_secs(61), 0));
}

impl Carrier {
    fn bound(&self, overall: Instant) -> Result<(), String> {
        let left = overall
            .min(self.presence.deadline)
            .checked_duration_since(Instant::now())
            .ok_or("member carrier deadline expired")?;
        self.line
            .set_read_timeout(Some(left.min(Duration::from_secs(2))))
            .map_err(|e| format!("member carrier timeout:{e:?}"))
    }
    fn send(&mut self, frame: &In, overall: Instant) -> Result<(), String> {
        self.bound(overall)?;
        if self.frames >= 128 {
            return Err("member carrier frame budget exhausted".into());
        }
        self.frames += 1;
        send(&mut self.line, frame, MESSAGE)
    }
    fn receive(&mut self, overall: Instant) -> Result<Out, String> {
        self.bound(overall)?;
        let out = receive(&mut self.line, ADMISSION)?;
        self.bound(overall)?;
        Ok(out)
    }
    fn accept_presence(
        &mut self,
        sequence: u64,
        anchor: Instant,
        overall: Instant,
    ) -> Result<(), String> {
        let frame = self.receive(overall)?;
        self.presence
            .accept(&frame, sequence, anchor, overall, Instant::now())
            .map_err(String::from)?;
        // Only a validated owner receipt can replace the socket watchdog.
        // Partial frames or raw traffic never renew this admitted lease.
        self.line
            .replace_io_deadline(self.presence.deadline.min(overall))
            .map_err(|e| format!("member presence I/O deadline:{e:?}"))
    }

    fn face(
        &mut self,
        request: &OwnerFaceSnapshotRequest,
        overall: Instant,
    ) -> Result<Snapshot, String> {
        self.send(
            &In::FaceSnapshotRequest {
                protocol: PROTOCOL,
                request: request.clone(),
            },
            overall,
        )?;
        match self.receive(overall)? {
            Out::FaceSnapshotResponse {
                protocol: PROTOCOL,
                response,
            } => snapshot(serde_json::to_value(response).map_err(|e| e.to_string())?),
            _ => Err("member Face response differs from selected carrier".into()),
        }
    }
}
fn request(credential: &MembershipCredential) -> OwnerFaceSnapshotRequest {
    OwnerFaceSnapshotRequest {
        schema: OWNER_FACE_REQUEST_SCHEMA.into(),
        credential_id: credential.credential_id.as_str().into(),
        body_id: credential.body_id.clone(),
        part_id: credential.part_id.clone(),
        host_id: credential.host_id.clone(),
        boot_id: credential.boot_id.clone(),
        last_seen_revision: None,
        last_seen_identity: None,
    }
}

#[cfg(all(target_os = "linux", feature = "native-display"))]
impl conduit_std_host::NativeMaskEncounter for MemberRoute {
    fn validate_current(
        &self,
        face: &Face,
        route: &RemoteOwnerMaskRouteSeal,
        local_offer: &conduit_core::HostAdvertisement,
    ) -> Result<(), String> {
        self.check()?;
        if route.body_id != self.request.body_id
            || face.basis.body_id.as_ref() != Some(&self.request.body_id)
            || local_offer.host_id != self.request.host_id
            || local_offer.boot_id != self.request.boot_id
        {
            return Err("native member encounter differs from its authenticated Host/Boot".into());
        }
        self.admitted.validate_native(face, route)?;
        route
            .validate_mask_host_offer(local_offer)
            .map_err(|_| "native member route targets a different current offer".into())
    }
}
