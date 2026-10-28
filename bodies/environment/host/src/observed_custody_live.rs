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
/// Incremental, bounded custody work. Collection remains the caller's first
/// operation each cycle. Only one HTTP operation is attempted per tick; no
/// empty slot can enter the one-shot command's 300-second polling loop.
pub(crate) struct Continuing {
    slot: u8,
    pending: Option<(Vec<u8>, Vec<u8>)>,
    accepted: [Option<[u8; 32]>; 6],
    next: Instant,
    disabled: bool,
    pending_offers: u8,
}
impl Continuing {
    pub(crate) fn new() -> Self {
        Self {
            slot: 0,
            pending: None,
            accepted: [None; 6],
            next: Instant::now(),
            disabled: false,
            pending_offers: 0,
        }
    }
    pub(crate) fn tick(&mut self, binding: &Path, continuity: &Path, peer: &str, history: &Path) {
        if self.disabled || Instant::now() < self.next {
            return;
        }
        self.next = Instant::now() + Duration::from_secs(30);
        if let Err(error) = self.advance(binding, continuity, peer, history) {
            // Explicit protocol/authentication refusal is not an invitation to
            // sign again or try a different scope. Recording itself continues.
            self.disabled = true;
            eprintln!("automatic custody disabled; recording continues: {error}");
        }
    }
    fn advance(
        &mut self,
        binding_path: &Path,
        continuity: &Path,
        peer: &str,
        history: &Path,
    ) -> Result<()> {
        let slot = self.slot;
        let (method, path, body) = match &self.pending {
            Some((_, request)) => (
                "POST",
                format!("/history/custody/ack?slot={slot}"),
                request.as_slice(),
            ),
            None => (
                "GET",
                format!("/history/custody/offer?slot={slot}"),
                [].as_slice(),
            ),
        };
        let (status, response) = match crate::body_join::http(peer, method, &path, body) {
            Ok(response) => response,
            // Keep the exact pending bytes after a possibly accepted request.
            // Connection loss never changes the epoch or signs a new request.
            Err(_) => return Ok(()),
        };
        if status == 202 || status == 503 {
            if self.pending.is_none() {
                self.pending_offers += 1;
                if self.pending_offers >= 6 {
                    self.pending_offers = 0;
                    self.slot = (slot + 1) % 6;
                }
            }
            return Ok(());
        }
        self.pending_offers = 0;
        if self.pending.is_none() && status == 409 {
            self.slot = (slot + 1) % 6;
            return Ok(());
        }
        if status != 200 {
            return Err(format!("custody HTTP refusal {status} at slot {slot}").into());
        }
        let binding = crate::local_recorder::validated_binding(binding_path)?;
        if let Some((offer_bytes, request)) = &self.pending {
            let offer: wire::Offer<'_> = serde_json::from_slice(offer_bytes)?;
            verify_receipt(
                &response,
                &offer,
                request,
                &binding.credential().body_id,
                binding.root(),
            )?;
            let key = checked(VerifyingKey::from_bytes(binding.root()))?;
            self.accepted[usize::from(slot)] = Some(
                checked(wire::verify_offer(
                    &offer,
                    &binding.credential().body_id,
                    &key,
                ))?
                .id(),
            );
            self.pending = None;
            self.slot = (slot + 1) % 6;
            eprintln!("observed epoch slot {slot}: durable custody accepted; files retained, erase not confirmed");
            return Ok(());
        }
        let offer: wire::Offer<'_> = serde_json::from_slice(&response)?;
        let key = checked(VerifyingKey::from_bytes(binding.root()))?;
        let authenticated = checked(wire::verify_offer(
            &offer,
            &binding.credential().body_id,
            &key,
        ))?;
        if self.accepted[usize::from(slot)] == Some(authenticated.id()) {
            self.slot = (slot + 1) % 6;
            return Ok(());
        }
        let verified = match VerifiedDurableEpoch::verify(
            &offer,
            &binding.credential().body_id,
            binding.root(),
            &history.join(format!("epoch-{}", hex(&offer.scope.0))),
        ) {
            Ok(verified) => verified,
            Err(error) => {
                // Collection may not yet include the final closed-epoch batch.
                // No signature or acknowledgment exists before durable proof.
                eprintln!("custody slot {slot} awaits exact durable files: {error}");
                self.slot = (slot + 1) % 6;
                return Ok(());
            }
        };
        let request = crate::body_join::sign_durable_custody(binding_path, continuity, &verified)?;
        if request.len() > 768 {
            return Err("live observed custody request exceeds HTTP admission".into());
        }
        self.pending = Some((response, request));
        Ok(())
    }
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
    #[test]
    fn continuing_unavailable_offers_do_not_block_other_slots_or_require_signing_keys() {
        use std::{
            io::{Read, Write},
            net::TcpListener,
        };
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let peer = listener.local_addr().unwrap().to_string();
        let server = std::thread::spawn(move || {
            for attempt in 0..7 {
                let (mut socket, _) = listener.accept().unwrap();
                let mut bytes = [0; 1024];
                let count = socket.read(&mut bytes).unwrap();
                let request = std::str::from_utf8(&bytes[..count]).unwrap();
                assert!(request.starts_with(if attempt < 6 {
                    "GET /history/custody/offer?slot=0 "
                } else {
                    "GET /history/custody/offer?slot=1 "
                }));
                socket
                    .write_all(if attempt < 6 {
                        b"HTTP/1.1 202 Accepted\r\nContent-Length: 0\r\n\r\n"
                    } else {
                        b"HTTP/1.1 409 Conflict\r\nContent-Length: 0\r\n\r\n"
                    })
                    .unwrap();
            }
        });
        let mut continuing = Continuing::new();
        let absent = Path::new("/nonexistent/private-key");
        for _ in 0..7 {
            continuing.advance(absent, absent, &peer, absent).unwrap();
        }
        assert_eq!(continuing.slot, 2);
        assert!(continuing.pending.is_none());
        server.join().unwrap();
    }
    #[test]
    fn incremental_lost_response_retains_exact_ack_without_loading_a_new_key() {
        use std::{io::Read, net::TcpListener};
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let peer = listener.local_addr().unwrap().to_string();
        let server = std::thread::spawn(move || {
            for _ in 0..2 {
                let (mut socket, _) = listener.accept().unwrap();
                let mut bytes = Vec::new();
                let mut block = [0; 1024];
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
                }
                // A missing receipt is never interpreted as erased evidence.
            }
        });
        let mut continuing = Continuing::new();
        continuing.pending = Some((b"original-offer".to_vec(), b"exact-ack".to_vec()));
        let absent = Path::new("/nonexistent/private-key");
        for _ in 0..2 {
            continuing.advance(absent, absent, &peer, absent).unwrap();
        }
        assert_eq!(continuing.pending.unwrap().1, b"exact-ack");
        assert_eq!(continuing.slot, 0);
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
