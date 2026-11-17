//! Read only the independently selected existing member-contact owner pin.
//! No resolution, transport, installation, key creation or membership mutation.
use super::*;
use conduit_body::{FederationPeer, RendezvousAuthentication};
use std::net::SocketAddr;
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
            || self.authentication.server_identity != server_identity(&self.reachability)?
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
}

pub(in crate::durable_host) fn selected_owner(state: &Path) -> Result<FederationPeer, String> {
    let path = state.join(CONTACT_PATH);
    match std::fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.file_type().is_file() => (),
        Ok(_) => return Err("member contact must be a private regular file".into()),
        Err(_) => return Err("independently retained member owner pin is unavailable".into()),
    }
    let contact: MemberContact =
        serde_json::from_slice(&bounded_read(&path, MAXIMUM_CONTACT_BYTES)?)
            .map_err(|_| "invalid retained member contact")?;
    let installation = read_installation(&state.join("installation.json"))?;
    let credential = super::load_joined_credential(state, &installation)?;
    contact.validate(&credential)?;
    Ok(contact.owner)
}
pub(in crate::durable_host) fn install_member_contact(
    state: &Path,
    descriptor: &Path,
    authorize: bool,
) -> Result<(), String> {
    if !authorize {
        return Err("selecting a member contact requires --authorize-route".into());
    }
    let _host = super::super::owner_lock::acquire(state)?;
    let installation = read_installation(&state.join("installation.json"))?;
    let credential = super::load_joined_credential(state, &installation)?;
    let contact: MemberContact =
        serde_json::from_slice(&bounded_read(descriptor, MAXIMUM_CONTACT_BYTES)?)
            .map_err(|_| "invalid member contact")?;
    contact.validate(&credential)?;
    super::super::write_json_atomic(&state.join(CONTACT_PATH), &contact)
}

fn server_identity(url: &str) -> Result<String, String> {
    let authority = url
        .strip_prefix("wss://")
        .and_then(|remainder| remainder.split('/').next())
        .ok_or("Body admission route is not wss")?;
    let identity = if let Some(bracketed) = authority.strip_prefix('[') {
        bracketed
            .split(']')
            .next()
            .filter(|value| !value.is_empty())
    } else {
        authority
            .split(':')
            .next()
            .filter(|value| !value.is_empty())
    }
    .ok_or("Body admission route omitted its server identity")?;
    Ok(identity.into())
}

/// Explicit finite export: the listener proves its selected address/certificate,
/// then closes. The public descriptor does not promise a continuing service.
#[cfg(unix)]
#[allow(clippy::too_many_arguments)]
pub(in crate::durable_host) fn export_member_contact(
    state: &Path,
    credential_path: &Path,
    bind: SocketAddr,
    public_url: &str,
    certificate: &Path,
    private_key: &Path,
    authorize: bool,
) -> Result<(), String> {
    use std::io::Write;
    if !authorize {
        return Err("exporting a member contact requires --authorize-route".into());
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
        return Err("selected Part is not admitted in this original owner's Body".into());
    }
    drop(evidence);
    drop(truth);
    let listener = conduit_std_host::secure_websocket::SecureWebSocketListener::bind(
        bind,
        certificate,
        private_key,
        16 * 1024,
        true,
    )
    .map_err(|_| "member contact export TLS bind failed")?;
    if listener
        .local_addr()
        .map_err(|_| "member contact listener address unavailable")?
        != bind
    {
        return Err("member contact listener differs from authorized bind".into());
    }
    let contact = MemberContact {
        schema: SCHEMA.into(),
        body_id: selected.body_id.clone(),
        part_id: selected.part_id.clone(),
        host_id: selected.host_id.clone(),
        owner: crate::durable_host_control::startup_member::owner_peer(state)?,
        reachability: public_url.into(),
        authentication: RendezvousAuthentication {
            server_identity: server_identity(public_url)?,
            transport_binding_sha256: listener.certificate_binding_sha256(),
        },
        reconnect_after_millis: 1000,
        attempt_timeout_millis: 3000,
        maximum_attempts: 2,
    };
    contact.validate(&selected)?;
    let bytes = serde_json::to_vec(&contact).map_err(|_| "member contact encoding failed")?;
    if bytes.len() as u64 > MAXIMUM_CONTACT_BYTES {
        return Err("member contact exceeds its finite bound".into());
    }
    let mut output = std::io::stdout().lock();
    output
        .write_all(&bytes)
        .and_then(|_| output.write_all(b"\n"))
        .and_then(|_| output.flush())
        .map_err(|_| "member contact export failed")?;
    drop(listener);
    eprintln!("Public contact exported; no continuing contact listener is running.");
    Ok(())
}
#[cfg(not(unix))]
pub(in crate::durable_host) fn export_member_contact(
    _: &Path,
    _: &Path,
    _: SocketAddr,
    _: &str,
    _: &Path,
    _: &Path,
    _: bool,
) -> Result<(), String> {
    Err("member contact export requires local Unix owner control".into())
}
