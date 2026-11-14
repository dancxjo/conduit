//! Retained operator-selected reachability, never an invitation or execution grant.
//! Each resolution returns new finite evidence; existing member authentication
//! and native Mask admission still run on their own bounded connection.
use super::member_return::{self, MemberEncounter, MemberReturnFrame, MEMBER_ROUTE_SCHEMA};
use super::*;
use conduit_body::{FederationPeer, MembershipCredential};

const SCHEMA: &str = "conduit.body/member-contact@1";
const MAXIMUM_CONTACT_BYTES: u64 = 16 * 1024;
const CONTACT_PATH: &str = "body/member-contact.json";

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MemberContact {
    schema: String,
    body_id: conduit_body::BodyId,
    part_id: conduit_body::PartId,
    host_id: conduit_core::HostId,
    owner: FederationPeer,
    reachability: String,
    authentication: RendezvousAuthentication,
    reconnect_after_millis: u64,
    attempt_timeout_millis: u32,
    maximum_attempts: u8,
}

impl MemberContact {
    fn address(&self) -> Result<SocketAddr, String> {
        let address: SocketAddr = self
            .reachability
            .strip_prefix("wss://")
            .and_then(|authority| authority.parse().ok())
            .ok_or("standing member contact requires a numeric wss socket address")?;
        if address.ip().is_unspecified() || address.ip().is_loopback() || address.port() == 0 {
            return Err("standing member contact requires a non-loopback numeric endpoint and positive port".into());
        }
        Ok(address)
    }

    fn validate(&self, credential: &MembershipCredential) -> Result<(), String> {
        self.address()?;
        if self.reachability.len() > 256
            || self.schema != SCHEMA
            || self.body_id != credential.body_id
            || self.part_id != credential.part_id
            || self.host_id != credential.host_id
            || self.owner.host_id == self.host_id
            || self.owner.host_id.as_str().is_empty()
            || self.owner.host_id.as_str().len() > 256
            || self.owner.boot_id.as_str().is_empty()
            || self.owner.boot_id.as_str().len() > 256
            || self.owner.offer_generation.0 == 0
            || self.owner.verifying_key == [0; 32]
            || self.authentication.transport_binding_sha256 == [0; 32]
            || self.authentication.server_identity != route_server_identity(&self.reachability)?
            || !(1_000..=60_000).contains(&self.reconnect_after_millis)
            || !(1..=3_000).contains(&self.attempt_timeout_millis)
            || !(1..=2).contains(&self.maximum_attempts)
        {
            return Err(
                "member contact differs from retained scope or bounded pinned policy".into(),
            );
        }
        Ok(())
    }

    fn encounter(&self, ttl_seconds: u64) -> Result<MemberEncounter, String> {
        if !(1..=600).contains(&ttl_seconds) {
            return Err("member contact requires a 1..600 second evidence lifetime".into());
        }
        Ok(MemberEncounter {
            schema: MEMBER_ROUTE_SCHEMA.into(),
            body_id: self.body_id.clone(),
            part_id: self.part_id.clone(),
            host_id: self.host_id.clone(),
            owner: self.owner.clone(),
            candidate: RendezvousCandidate {
                candidate_id: "candidate/member-contact".into(),
                line_family: RendezvousLineFamily::AuthenticatedTlsStream,
                reachability: self.reachability.clone(),
                authentication: self.authentication.clone(),
                expires_at_millis: current_time_millis()?
                    .checked_add(ttl_seconds * 1_000)
                    .ok_or("member contact evidence expiry overflow")?,
                maximum_attempts: self.maximum_attempts,
                attempt_timeout_millis: self.attempt_timeout_millis,
            },
        })
    }

    #[cfg(any(test, all(target_os = "linux", feature = "native-display")))]
    fn validate_evidence(&self, encounter: &MemberEncounter, now: u64) -> Result<(), String> {
        let candidate = &encounter.candidate;
        if encounter.schema != MEMBER_ROUTE_SCHEMA
            || encounter.body_id != self.body_id
            || encounter.part_id != self.part_id
            || encounter.host_id != self.host_id
            || encounter.owner != self.owner
            || candidate.candidate_id != "candidate/member-contact"
            || candidate.line_family != RendezvousLineFamily::AuthenticatedTlsStream
            || candidate.reachability != self.reachability
            || candidate.authentication != self.authentication
            || candidate.expires_at_millis <= now
            || candidate.expires_at_millis.saturating_sub(now) > 600_000
            || candidate.maximum_attempts != self.maximum_attempts
            || candidate.attempt_timeout_millis != self.attempt_timeout_millis
        {
            return Err("member contact returned changed, expired or unbounded evidence".into());
        }
        Ok(())
    }
}

#[cfg(all(target_os = "linux", feature = "native-display"))]
fn selected_contact(state: &Path) -> Result<Option<MemberContact>, String> {
    let path = state.join(CONTACT_PATH);
    match std::fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("inspect member contact: {error}")),
        Ok(metadata) if !metadata.file_type().is_file() => {
            return Err("member contact must be a private regular file".into());
        }
        Ok(_) => (),
    }
    let contact: MemberContact =
        serde_json::from_slice(&bounded_read(&path, MAXIMUM_CONTACT_BYTES)?)
            .map_err(|_| "invalid retained member contact")?;
    let installation = super::super::read_installation(&state.join("installation.json"))?;
    let credential = super::super::membership::load_joined_credential(state, &installation)?;
    contact.validate(&credential)?;
    Ok(Some(contact))
}

pub(crate) fn install_member_contact(
    state: &Path,
    descriptor: &Path,
    authorize: bool,
) -> Result<(), String> {
    if !authorize {
        return Err("selecting a member contact requires --authorize-route".into());
    }
    let _host = super::super::owner_lock::acquire(state)?;
    let installation = super::super::read_installation(&state.join("installation.json"))?;
    let credential = super::super::membership::load_joined_credential(state, &installation)?;
    let contact: MemberContact =
        serde_json::from_slice(&bounded_read(descriptor, MAXIMUM_CONTACT_BYTES)?)
            .map_err(|_| "invalid member contact")?;
    contact.validate(&credential)?;
    super::super::write_json_atomic(&state.join(CONTACT_PATH), &contact)
}

#[cfg(all(target_os = "linux", feature = "native-display"))]
pub(crate) fn reconnect_delay(state: &Path) -> Result<Option<Duration>, String> {
    Ok(selected_contact(state)?
        .map(|contact| Duration::from_millis(contact.reconnect_after_millis)))
}

#[cfg(all(target_os = "linux", feature = "native-display"))]
pub(crate) fn load_and_resolve_member_contact(
    state: &Path,
) -> Result<Option<MemberEncounter>, String> {
    let Some(contact) = selected_contact(state)? else {
        return Ok(None);
    };
    // Discovery uses its own fixed attempt budget. A failure does not authorize
    // joining, returning or issuing another Show from an old grant.
    let mut last = "member contact had no discovery attempt".to_string();
    for _ in 0..contact.maximum_attempts {
        match resolve(&contact) {
            Ok(encounter) => return Ok(Some(encounter)),
            Err(error) => last = error,
        }
    }
    Err(last)
}

#[cfg(all(target_os = "linux", feature = "native-display"))]
fn resolve(contact: &MemberContact) -> Result<MemberEncounter, String> {
    resolve_at(contact, contact.address()?)
}

#[cfg(any(test, all(target_os = "linux", feature = "native-display")))]
fn resolve_at(contact: &MemberContact, address: SocketAddr) -> Result<MemberEncounter, String> {
    let timeout = Duration::from_millis(u64::from(contact.attempt_timeout_millis));
    let deadline = std::time::Instant::now() + timeout;
    let mut line = SecureWebSocketClientLine::connect_pinned_for_exchange(
        address,
        &contact.reachability,
        &contact.authentication.server_identity,
        contact.authentication.transport_binding_sha256,
        timeout,
        MAXIMUM_ROUTE_FRAME_BYTES as u32,
    )
    .map_err(|e| format!("member contact TLS: {e:?}"))?;
    line.set_read_timeout(Some(
        deadline
            .checked_duration_since(std::time::Instant::now())
            .ok_or("member contact deadline expired")?,
    ))
    .map_err(|e| format!("member contact timeout: {e:?}"))?;
    let mut nonce = [0; 32];
    getrandom::fill(&mut nonce).map_err(|e| e.to_string())?;
    send(
        &mut line,
        &MemberReturnFrame::DescribeContact {
            schema: SCHEMA.into(),
            nonce,
        },
    )?;
    line.set_read_timeout(Some(
        deadline
            .checked_duration_since(std::time::Instant::now())
            .ok_or("member contact deadline expired")?,
    ))
    .map_err(|e| format!("member contact timeout: {e:?}"))?;
    let frame = receive(&mut line)?;
    if std::time::Instant::now() >= deadline {
        return Err("member contact deadline expired".into());
    }
    let _ = line.close();
    let MemberReturnFrame::ContactDescribed {
        schema,
        nonce: returned,
        encounter,
    } = frame
    else {
        return Err("member contact did not return finite evidence".into());
    };
    if schema != SCHEMA || returned != nonce {
        return Err("member contact discovery schema or nonce differs".into());
    }
    contact.validate_evidence(&encounter, current_time_millis()?)?;
    Ok(*encounter)
}

/// Explicit administrative listener selection. It holds one TLS connection at
/// a time; no invitation, Part attachment, prepared Mask or Line is minted here.
#[allow(clippy::too_many_arguments)]
pub(crate) fn serve_body_member_contact(
    state: &Path,
    credential_path: &Path,
    ttl_seconds: u64,
    bind: SocketAddr,
    public_url: &str,
    identity: (&Path, &Path),
    authorize: bool,
) -> Result<(), String> {
    if !authorize || !(1..=600).contains(&ttl_seconds) {
        return Err(
            "standing contact requires explicit authorization and finite evidence lifetime".into(),
        );
    }
    let selected: MembershipCredential =
        serde_json::from_slice(&bounded_read(credential_path, 64 * 1024)?)
            .map_err(|_| "invalid selected member credential")?;
    let truth = crate::durable_host_control::inspect_owned_body(state)?;
    let evidence: conduit_body::BodyBiographyEvidence = serde_json::from_value(
        truth
            .get("biography")
            .cloned()
            .ok_or("member contact owner biography absent")?,
    )
    .map_err(|_| "member contact owner biography invalid")?;
    if evidence.body_id != selected.body_id
        || !evidence.membership.parts.iter().any(|part| {
            part.part_id == selected.part_id
                && part.state == conduit_body::MembershipState::Admitted
                && part
                    .current
                    .as_ref()
                    .is_none_or(|current| current.host_id == selected.host_id)
        })
    {
        return Err("member contact selected Part is not admitted in this owner's Body".into());
    }
    drop(evidence);
    drop(truth);
    let listener = SecureWebSocketListener::bind(
        bind,
        identity.0,
        identity.1,
        MAXIMUM_ROUTE_FRAME_BYTES as u32,
        true,
    )
    .map_err(|e| format!("member contact bind: {e:?}"))?;
    if listener
        .local_addr()
        .map_err(|e| format!("member contact address: {e:?}"))?
        != bind
    {
        return Err("member contact listener differs from authorized bind".into());
    }
    let contact = MemberContact {
        schema: SCHEMA.into(),
        body_id: selected.body_id.clone(),
        part_id: selected.part_id.clone(),
        host_id: selected.host_id.clone(),
        owner: crate::durable_host_control::carrier_receiver(state)?,
        reachability: public_url.into(),
        authentication: RendezvousAuthentication {
            server_identity: route_server_identity(public_url)?,
            transport_binding_sha256: listener.certificate_binding_sha256(),
        },
        reconnect_after_millis: 1_000,
        attempt_timeout_millis: 3_000,
        maximum_attempts: 2,
    };
    contact.validate(&selected)?;
    println!(
        "{}",
        serde_json::to_string(&contact).map_err(|e| e.to_string())?
    );
    std::io::stdout().flush().map_err(|e| e.to_string())?;
    // One finite evidence slot, shared by its following return and Resume. An
    // accepted connection must not manufacture a later challenge deadline.
    let mut issued: Option<MemberEncounter> = None;
    loop {
        // Anonymous connection/handshake/input gets only the attempt budget.
        // Existing authenticated presence installs its own separately funded
        // finite I/O lease after proof; raw bytes cannot hold this slot for60s.
        let mut line = match listener.accept_with_timeout(Duration::from_secs(ttl_seconds.min(3))) {
            Ok(line) => line,
            Err(SecureWebSocketError::AcceptDeadline) => continue,
            // A malformed peer cannot permanently kill an operator-selected service.
            Err(SecureWebSocketError::Tls | SecureWebSocketError::Handshake) => continue,
            Err(error) => return Err(format!("member contact accept: {error:?}")),
        };
        line.set_read_timeout(Some(Duration::from_secs(3)))
            .map_err(|e| format!("member contact input deadline: {e:?}"))?;
        if crate::durable_host_control::carrier_receiver(state)? != contact.owner {
            return Err("member contact owner changed; explicit reselection required".into());
        }
        let frame: MemberReturnFrame = match receive(&mut line) {
            Ok(frame) => frame,
            Err(error) => {
                member_return::refuse(&mut line, &error);
                continue;
            }
        };
        if let MemberReturnFrame::DescribeContact { schema, nonce } = &frame {
            if schema != SCHEMA || *nonce == [0; 32] {
                member_return::refuse(
                    &mut line,
                    "member contact discovery schema or nonce differs",
                );
            } else {
                let encounter = contact.encounter(ttl_seconds)?;
                issued = Some(encounter.clone());
                if let Err(error) = send(
                    &mut line,
                    &MemberReturnFrame::ContactDescribed {
                        schema: SCHEMA.into(),
                        nonce: *nonce,
                        encounter: Box::new(encounter),
                    },
                ) {
                    eprintln!("member contact discovery ended: {error}");
                }
            }
            let _ = line.close();
            continue;
        }
        let Some(encounter) = issued.as_ref().filter(|encounter| {
            current_time_millis().is_ok_and(|now| encounter.candidate.expires_at_millis > now)
        }) else {
            member_return::refuse(&mut line, "fresh member contact evidence required");
            continue;
        };
        if let Err(error) =
            member_return::serve_member_frame(state, encounter, &listener, line, frame)
        {
            eprintln!("member contact encounter ended: {error}");
        }
    }
}

#[cfg(test)]
#[path = "member_contact/tests.rs"]
mod tests;
