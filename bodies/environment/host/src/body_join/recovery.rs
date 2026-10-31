//! Read-only continuity recovery after a producer loses its volatile result.
use super::*;
use crate::local_recorder::recovery;
#[derive(Serialize)]
struct RecoveredBinding<'a> {
    schema: &'static str,
    invitation: &'a conduit_body::SpawnInvitationClaim,
    root_key: [u8; 32],
    local_host: &'a conduit_core::HostId,
    local_boot: &'a conduit_core::BootId,
    recovery_request: &'a recovery::Request,
    receipt: &'a recovery::Receipt,
}
pub(super) fn recover(
    bootstrap: &Bootstrap,
    state: &Path,
    peer: &str,
    host: &conduit_core::HostId,
    boot: &conduit_core::BootId,
) -> Result<()> {
    let path = state.join("recovery-request.json");
    let query: recovery::Request = if path.exists() {
        load(&path)?
    } else {
        let mut nonce = [0; 32];
        std::fs::File::open("/dev/urandom")?.read_exact(&mut nonce)?;
        if nonce == [0; 32] {
            return Err("recovery entropy unavailable".into());
        }
        let mut query = recovery::Request {
            schema: recovery::REQUEST.into(),
            body_id: bootstrap.invitation.claim.body_id.clone(),
            host_id: host.clone(),
            boot_id: boot.clone(),
            nonce,
            signature: Vec::new(),
        };
        query.signature = ed25519_dalek::SigningKey::from_bytes(&bootstrap.invitation.secret)
            .sign(&query.transcript())
            .to_bytes()
            .to_vec();
        save(&path, &query)?;
        query
    };
    if query.schema != recovery::REQUEST
        || query.body_id != bootstrap.invitation.claim.body_id
        || &query.host_id != host
        || &query.boot_id != boot
        || query.nonce == [0; 32]
    {
        return Err("retained recovery scope".into());
    }
    let signature: [u8; 64] = query
        .signature
        .as_slice()
        .try_into()
        .map_err(|_| "retained recovery signature extent")?;
    checked(
        ed25519_dalek::SigningKey::from_bytes(&bootstrap.invitation.secret)
            .verifying_key()
            .verify_strict(
                &query.transcript(),
                &ed25519_dalek::Signature::from_bytes(&signature),
            ),
    )?;
    let payload = serde_json::to_vec(&query)?;
    if payload.len() > 1024 {
        return Err("recovery request extent".into());
    }
    let (status, bytes) = http(peer, "POST", "/body/join", &payload)?;
    if status != 202 {
        return Err(format!("recovery submission HTTP {status}").into());
    }
    let ticket: Ticket = serde_json::from_slice(&bytes)?;
    if ticket.schema != "conduit.environment/body-join-queued@1" {
        return Err("recovery queue profile".into());
    }
    let deadline = Instant::now() + Duration::from_secs(60);
    let receipt = loop {
        let response = http(
            peer,
            "GET",
            &format!("/body/join/result?id={}", ticket.ticket),
            &[],
        );
        let (status, bytes) = match response {
            Ok(reply) => reply,
            Err(_) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_secs(1));
                continue;
            }
            Err(error) => return Err(error),
        };
        if status == 200 {
            break serde_json::from_slice::<recovery::Receipt>(&bytes)?;
        }
        if status != 202 || Instant::now() >= deadline {
            return Err(format!("recovery result HTTP {status}").into());
        }
        let pending: Ticket = serde_json::from_slice(&bytes)?;
        if pending.schema != ticket.schema || pending.ticket != ticket.ticket {
            return Err("recovery ticket changed".into());
        }
        std::thread::sleep(Duration::from_millis(250));
    };
    checked(receipt.verify(&bootstrap.root_key, &query))?;
    if receipt.producer_host != bootstrap.producer_host {
        return Err("recovery producer host changed".into());
    }
    save(
        &state.join("recorder-binding.json"),
        &RecoveredBinding {
            schema: "conduit.environment/joined-recorder@2",
            invitation: &bootstrap.invitation.claim,
            root_key: bootstrap.root_key,
            local_host: host,
            local_boot: boot,
            recovery_request: &query,
            receipt: &receipt,
        },
    )
}
