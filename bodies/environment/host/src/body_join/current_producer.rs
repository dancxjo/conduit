//! Fresh read-only producer identity through the existing membership recovery wire.
//! This grants neither new membership nor recording custody or clock authority.
use super::*;
use crate::local_recorder::{recovery, ValidatedBinding};
use std::net::ToSocketAddrs;
#[path = "current_producer_retry.rs"]
mod retry;

/// An untrusted transport hint to attempt the canonical signed handshake.
/// This error is never evidence of admission or of an actual Root restart.
#[derive(Debug)]
pub(crate) struct ReturnRequired;
impl std::fmt::Display for ReturnRequired {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("current producer requires observer continuity return")
    }
}
impl std::error::Error for ReturnRequired {}
fn refusal(status: u16, bytes: &[u8]) -> Box<dyn std::error::Error> {
    let reason = super::refusal_reason(bytes);
    if status == 403 && reason == "observer-recovery-continuity" {
        Box::new(ReturnRequired)
    } else {
        format!("current producer HTTP {status} ({reason})").into()
    }
}

pub(crate) fn probe(binding_path: &Path, continuity: &Path, peer: &str) -> Result<String> {
    let binding = crate::local_recorder::validated_binding(binding_path)?;
    let bootstrap: Bootstrap = load(continuity)?;
    if binding.root() != &bootstrap.root_key
        || binding.invitation() != &bootstrap.invitation.claim
        || binding.producer_host() != &bootstrap.producer_host
    {
        return Err("current producer continuity differs".into());
    }
    let mut nonce = [0; 32];
    std::fs::File::open("/dev/urandom")?.read_exact(&mut nonce)?;
    if nonce == [0; 32] {
        return Err("current producer entropy unavailable".into());
    }
    let credential = binding.credential();
    let mut request = recovery::Request {
        schema: recovery::REQUEST.into(),
        body_id: credential.body_id.clone(),
        host_id: credential.host_id.clone(),
        boot_id: credential.boot_id.clone(),
        nonce,
        signature: Vec::new(),
    };
    request.signature = ed25519_dalek::SigningKey::from_bytes(&bootstrap.invitation.secret)
        .sign(&request.transcript())
        .to_bytes()
        .to_vec();
    let payload = serde_json::to_vec(&request)?;
    // DNS is synchronous, as in the existing collector; socket operations share
    // one absolute budget. This read-only probe never retries an admission.
    let address = peer
        .to_socket_addrs()?
        .find(|a| a.is_ipv4())
        .ok_or("current producer has no IPv4 route")?;
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut attempts = 16;
    let mut exchange = |method, path: &str, body: &[u8]| {
        retry::exchange(deadline, &mut attempts, || {
            super::http::http_resolved_until(
                super::http::Request {
                    peer,
                    method,
                    path,
                    body,
                    content_type: "application/json",
                },
                address,
                deadline,
            )
        })
    };
    let (status, bytes) = exchange("POST", "/body/join", &payload)?;
    if status != 202 {
        return Err(refusal(status, &bytes));
    }
    let ticket: Ticket = serde_json::from_slice(&bytes)?;
    if ticket.schema != "conduit.environment/body-join-queued@1" || ticket.ticket == 0 {
        return Err("current producer probe ticket differs".into());
    }
    for _ in 0..16 {
        let (status, bytes) = exchange(
            "GET",
            &format!("/body/join/result?id={}", ticket.ticket),
            &[],
        )?;
        if status == 200 {
            let receipt: recovery::Receipt = serde_json::from_slice(&bytes)?;
            return verified_boot(&binding, &request, &receipt);
        }
        if status != 202 {
            return Err(refusal(status, &bytes));
        }
        let pending: Ticket = serde_json::from_slice(&bytes)?;
        if pending.schema != ticket.schema || pending.ticket != ticket.ticket {
            return Err("current producer probe ticket changed".into());
        }
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or("current producer probe expired")?;
        std::thread::sleep(remaining.min(Duration::from_millis(100)));
    }
    Err("current producer probe poll bound".into())
}

fn verified_boot(
    binding: &ValidatedBinding,
    request: &recovery::Request,
    receipt: &recovery::Receipt,
) -> Result<String> {
    checked(receipt.verify(binding.root(), request))?;
    if receipt.producer_host != *binding.producer_host()
        || receipt.credential != *binding.credential()
        || !receipt.producer_boot.as_str().starts_with("boot/")
        || receipt.producer_boot.as_str().len() > 128
        || receipt
            .producer_boot
            .as_str()
            .bytes()
            .any(|b| b.is_ascii_control())
    {
        return Err("current producer receipt identity differs".into());
    }
    Ok(receipt.producer_boot.as_str().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_exact_continuity_refusal_is_a_return_hint() {
        let bytes = br#"{"schema":"conduit.environment/body-join-refused@1","reason":"observer-recovery-continuity"}"#;
        assert!(refusal(403, bytes).is::<ReturnRequired>());
        assert!(!refusal(500, bytes).is::<ReturnRequired>());
        assert!(!refusal(403, b"untrusted server error").is::<ReturnRequired>());
        assert!(!refusal(
            403,
            br#"{"schema":"other","reason":"observer-recovery-continuity"}"#
        )
        .is::<ReturnRequired>());
    }
}
