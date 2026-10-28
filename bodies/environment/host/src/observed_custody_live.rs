//! Live acceptance of exact durable observation custody. Acceptance is not an
//! erase receipt; local retained files and the running recorder are untouched.
use crate::{
    checked,
    observed_custody_client::{wire, VerifiedDurableEpoch},
    Result,
};
use ed25519_dalek::VerifyingKey;
use std::{
    path::Path,
    time::{Duration, Instant},
};
const RECEIPT: &str = "conduit.environment/accepted-observed-custody@1";
const DEADLINE: Duration = Duration::from_secs(300);

pub(crate) fn run(
    binding_path: &Path,
    continuity: &Path,
    peer: &str,
    history: &Path,
    slot: u8,
) -> Result<()> {
    if slot > 5 {
        return Err("observed custody slot must be 0..5".into());
    }
    let binding = crate::local_recorder::validated_binding(binding_path)?;
    let offer_bytes = poll(
        peer,
        "GET",
        &format!("/history/custody/offer?slot={slot}"),
        &[],
    )?;
    let offer: wire::Offer<'_> = serde_json::from_slice(&offer_bytes)?;
    let scope = hex(&offer.scope.0);
    let verified = VerifiedDurableEpoch::verify(
        &offer,
        &binding.credential().body_id,
        binding.root(),
        &history.join(format!("epoch-{scope}")),
    )?;
    let request = crate::body_join::sign_durable_custody(binding_path, continuity, &verified)?;
    if request.len() > 768 {
        return Err("live observed custody request exceeds HTTP admission".into());
    }
    // Reuse these exact bytes for every pending response. Neither reconnection
    // nor a retry silently switches the retained member or observation epoch.
    let response = poll(
        peer,
        "POST",
        &format!("/history/custody/ack?slot={slot}"),
        &request,
    )?;
    verify_receipt(
        &response,
        &offer,
        &request,
        &binding.credential().body_id,
        binding.root(),
    )?;
    println!("observed epoch slot {slot}: exact durable recorder custody accepted; reclamation not confirmed");
    Ok(())
}
fn poll(peer: &str, method: &str, path: &str, request: &[u8]) -> Result<Vec<u8>> {
    let deadline = Instant::now() + DEADLINE;
    loop {
        if Instant::now() >= deadline {
            return Err("live observed custody deadline".into());
        }
        // A lost response may follow a successful acceptance. Retry the same
        // bytes; only an authenticated receipt establishes success. An explicit
        // HTTP refusal remains terminal, while transport loss consumes the same
        // finite deadline as pending inspection.
        let result = crate::body_join::http(peer, method, path, request);
        let (status, response) = match result {
            Ok(response) => response,
            Err(_) => {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return Err("live observed custody transport deadline".into());
                }
                std::thread::sleep(remaining.min(Duration::from_secs(1)));
                continue;
            }
        };
        match status {
            200 => return Ok(response),
            202 => {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return Err("live observed custody deadline".into());
                }
                std::thread::sleep(remaining.min(Duration::from_secs(1)));
            }
            _ => return Err(format!("live observed custody refused: HTTP {status}").into()),
        }
    }
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
fn verify_receipt(
    response: &[u8],
    offer: &wire::Offer<'_>,
    request: &[u8],
    body: &conduit_body::BodyId,
    root: &[u8; 32],
) -> Result<()> {
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Receipt<'a> {
        schema: &'a str,
        accepted: &'a str,
    }
    let receipt: Receipt<'_> = serde_json::from_slice(response)?;
    if receipt.schema != RECEIPT || receipt.accepted.len() != 832 {
        return Err("observed acceptance receipt schema/extent".into());
    }
    let mut bytes = [0; 416];
    for (out, pair) in bytes
        .iter_mut()
        .zip(receipt.accepted.as_bytes().chunks_exact(2))
    {
        let digit = |byte| match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            _ => None,
        };
        *out = digit(pair[0]).ok_or("observed acceptance hexadecimal")? * 16
            + digit(pair[1]).ok_or("observed acceptance hexadecimal")?;
    }
    let key = checked(VerifyingKey::from_bytes(root))?;
    let authenticated = checked(wire::verify_offer(offer, body, &key))?;
    let request: wire::Request<'_> = serde_json::from_slice(request)?;
    checked(request.validate())?;
    if !wire::verify_accepted(&bytes, body, &key)
        || request.offer.0 != authenticated.id()
        || bytes[..32] != offer.region.0
        || bytes[32..128] != offer.scope.0
        || bytes[128..160] != offer.forms.0
        || bytes[160..164] != offer.committed.to_le_bytes()
        || bytes[164..196] != offer.digest.0
        || bytes[196..228] != request.transcript()
        || bytes[228..292] != request.signature.0
    {
        return Err(
            "observed acceptance does not match exact signed offer/request/Root/Body".into(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use sha2::{Digest, Sha256};
    #[test]
    fn lost_response_and_pending_retry_preserve_the_exact_request() {
        use std::{
            io::{Read, Write},
            net::TcpListener,
        };
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let peer = listener.local_addr().unwrap().to_string();
        let server = std::thread::spawn(move || {
            for attempt in 0..3 {
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut bytes = Vec::new();
                let mut block = [0; 128];
                loop {
                    let count = socket.read(&mut block).unwrap();
                    assert_ne!(count, 0);
                    bytes.extend_from_slice(&block[..count]);
                    if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                        if bytes.len() >= end + 4 + 9 {
                            assert_eq!(&bytes[end + 4..], b"exact-ack");
                            break;
                        }
                    }
                    assert!(bytes.len() <= 1024);
                }
                if attempt == 0 {
                    continue;
                } // Accepted response lost in transport.
                let reply = if attempt == 1 {
                    b"HTTP/1.1 202 Accepted\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                        .as_slice()
                } else {
                    b"HTTP/1.1 200 OK\r\nContent-Length: 7\r\nConnection: close\r\n\r\nreceipt"
                        .as_slice()
                };
                socket.write_all(reply).unwrap();
            }
        });
        assert_eq!(
            poll(&peer, "POST", "/history/custody/ack?slot=1", b"exact-ack").unwrap(),
            b"receipt"
        );
        server.join().unwrap();
    }
    fn fixture() -> (
        conduit_body::BodyId,
        SigningKey,
        wire::Offer<'static>,
        Vec<u8>,
        [u8; 416],
    ) {
        let body: conduit_body::BodyId =
            serde_json::from_value(serde_json::json!("02".repeat(32))).unwrap();
        let root = SigningKey::from_bytes(&[41; 32]);
        let mut scope = [9; 96];
        scope[..32].copy_from_slice(&Sha256::digest(body.as_str().as_bytes()));
        let mut offer = wire::Offer {
            schema: wire::OFFER,
            region: wire::Hex([8; 32]),
            scope: wire::Hex(scope),
            forms: wire::Hex([7; 32]),
            committed: 3,
            digest: wire::Hex([6; 32]),
            signature: wire::Hex([0; 64]),
        };
        let id = wire::offer_transcript(&offer, &root.verifying_key().to_bytes());
        offer.signature = wire::Hex(root.sign(&id).to_bytes());
        let request = wire::Request {
            schema: wire::ACK,
            offer: wire::Hex(id),
            part: "part",
            host: "host",
            boot: "boot",
            signature: wire::Hex([5; 64]),
        };
        let mut accepted = [0; 416];
        accepted[..32].copy_from_slice(&offer.region.0);
        accepted[32..128].copy_from_slice(&scope);
        accepted[128..160].copy_from_slice(&offer.forms.0);
        accepted[160..164].copy_from_slice(&offer.committed.to_le_bytes());
        accepted[164..196].copy_from_slice(&offer.digest.0);
        accepted[196..228].copy_from_slice(&request.transcript());
        accepted[228..292].copy_from_slice(&request.signature.0);
        let signature = root.sign(&wire::accepted_transcript(&accepted)).to_bytes();
        accepted[352..].copy_from_slice(&signature);
        (
            body,
            root,
            offer,
            serde_json::to_vec(&request).unwrap(),
            accepted,
        )
    }
    fn receipt(bytes: &[u8; 416]) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({"schema":RECEIPT,"accepted":hex(bytes)})).unwrap()
    }
    #[test]
    fn signed_acceptance_must_match_exact_offer_and_request() {
        let (body, root, offer, request, accepted) = fixture();
        let key = root.verifying_key().to_bytes();
        verify_receipt(&receipt(&accepted), &offer, &request, &body, &key).unwrap();
        // Even an authentic Root signature must not accept another epoch,
        // retained member request, or reserved byte layout.
        for position in [0, 32, 128, 160, 164, 196, 228, 324] {
            let mut wrong = accepted;
            wrong[position] ^= 1;
            let signature = root.sign(&wire::accepted_transcript(&wrong)).to_bytes();
            wrong[352..].copy_from_slice(&signature);
            assert!(verify_receipt(&receipt(&wrong), &offer, &request, &body, &key).is_err());
        }
        let wrong_root = SigningKey::from_bytes(&[42; 32]).verifying_key().to_bytes();
        assert!(verify_receipt(&receipt(&accepted), &offer, &request, &body, &wrong_root).is_err());
        let mut tampered = accepted;
        tampered[352] ^= 1;
        assert!(verify_receipt(&receipt(&tampered), &offer, &request, &body, &key).is_err());
    }
    #[test]
    fn malformed_or_ambiguous_receipts_fail_closed() {
        let (body, root, offer, request, accepted) = fixture();
        let key = root.verifying_key().to_bytes();
        for response in [
            serde_json::json!({"schema":RECEIPT,"accepted":hex(&accepted),"reclaimed":true}),
            serde_json::json!({"schema":"other","accepted":hex(&accepted)}),
            serde_json::json!({"schema":RECEIPT,"accepted":hex(&accepted).to_uppercase()}),
            serde_json::json!({"schema":RECEIPT,"accepted":"00"}),
        ] {
            assert!(verify_receipt(
                &serde_json::to_vec(&response).unwrap(),
                &offer,
                &request,
                &body,
                &key
            )
            .is_err());
        }
    }
}
