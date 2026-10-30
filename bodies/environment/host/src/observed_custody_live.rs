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
}
impl Continuing {
    pub(crate) fn new() -> Self {
        Self {
            slot: 0,
            pending: None,
            accepted: [None; 6],
            next: Instant::now(),
            disabled: false,
        }
    }
    pub(crate) fn due(&self) -> Option<Instant> {
        (!self.disabled).then_some(self.next)
    }
    /// Called only after canonical return verifies. An idle refused lane may
    /// resume, but a possibly accepted signed request keeps its original owner
    /// and bytes; a new credential is not authority to replace that request.
    pub(crate) fn membership_returned(&mut self) {
        if self.pending.is_none() {
            self.disabled = false;
            self.next = Instant::now();
        }
    }
    pub(crate) fn tick(&mut self, binding: &Path, continuity: &Path, peer: &str, history: &Path) {
        if self.disabled || Instant::now() < self.next {
            return;
        }
        self.next = Instant::now() + Duration::from_secs(30);
        if let Err(error) = self.advance(binding, continuity, peer, history) {
            // Invalid authenticated content or terminal protocol failures are
            // not authority to sign again. A slot's explicit 409 is reconciled
            // separately without claiming acceptance. Recording continues.
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
        if status == 202 {
            // Root retains one asynchronous completion. Switching GET slots
            // before consuming it can evict that completion indefinitely.
            // Retry this exact GET, just as a possibly accepted POST must keep
            // its exact signed bytes. Collection still runs before each tick.
            return Ok(());
        }
        if status == 503 {
            if self.pending.is_none() {
                // An unavailable slot must not hold every other closed epoch
                // behind it. Revisit this same hint on the next
                // finite round; a slot is never identity or erase authority.
                self.slot = (slot + 1) % 6;
            }
            // A possibly accepted POST retains its exact immutable request.
            return Ok(());
        }
        if status == 409 {
            // A producer restart may retire an acceptance cache or move its
            // service slots. Explicit refusal releases only our pending request;
            // it proves neither acceptance nor erasure. Durable files and the
            // accepted identity cache remain untouched. Any later offer must
            // pass Root authentication and exact durable verification anew.
            self.pending = None;
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
        .zip(receipt.accepted.as_bytes().as_chunks::<2>().0)
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
#[path = "../../../../targets/esp32/proof/tm1637/src/native_observed_request_queue.rs"]
mod producer_queue;
#[cfg(test)]
mod tests {
    #[test]
    fn verified_return_resumes_idle_lane_but_preserves_possibly_accepted_request() {
        let mut lane = super::Continuing::new();
        lane.disabled = true;
        lane.membership_returned();
        assert!(lane.due().is_some());
        lane.disabled = true;
        lane.pending = Some((vec![1, 2], vec![3, 4]));
        lane.membership_returned();
        assert!(lane.due().is_none());
        assert_eq!(lane.pending, Some((vec![1, 2], vec![3, 4])));
    }
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
    fn accepted_get_consumes_single_cache_completion_before_another_slot() {
        use std::{
            io::{Read, Write},
            net::TcpListener,
        };
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let peer = listener.local_addr().unwrap().to_string();
        let server = std::thread::spawn(move || {
            use super::producer_queue::{Queue, Request, Response};
            let mut queue = Queue::<u8>::new();
            let mut job = None;
            let mut completed_deliveries = 0;
            for attempt in 0..3 {
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut request = Vec::new();
                while !request.ends_with(b"\r\n\r\n") {
                    let mut byte = [0];
                    socket.read_exact(&mut byte).unwrap();
                    request.push(byte[0]);
                }
                let request = String::from_utf8(request).unwrap();
                let slot = request
                    .split("slot=")
                    .nth(1)
                    .unwrap()
                    .split(' ')
                    .next()
                    .unwrap()
                    .parse()
                    .unwrap();
                // Use the production Root queue, including its single completed
                // cache and different-request eviction, not a mirrored model.
                let reply = match queue.submit(Request::offer(slot)) {
                    Response::Pending => {
                        b"HTTP/1.1 202 Accepted\r\nContent-Length: 0\r\n\r\n".as_slice()
                    }
                    Response::Busy => {
                        b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\n\r\n".as_slice()
                    }
                    Response::Completed(Ok(_)) => {
                        completed_deliveries += 1;
                        b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}".as_slice()
                    }
                    Response::Completed(Err(())) => unreachable!(),
                };
                socket.write_all(reply).unwrap();
                if job.is_none() {
                    job = queue.take();
                }
                if attempt == 1 {
                    queue.complete(job.take().unwrap().key, Ok(1));
                }
            }
            completed_deliveries
        });
        let mut continuing = Continuing::new();
        let absent = Path::new("/nonexistent/private-key");
        for _ in 0..2 {
            continuing.advance(absent, absent, &peer, absent).unwrap();
            assert_eq!(continuing.slot, 0);
        }
        // The completed response reaches ordinary binding/authentication
        // validation; absent credentials cannot fabricate an accepted offer.
        assert!(continuing.advance(absent, absent, &peer, absent).is_err());
        assert_eq!(server.join().unwrap(), 1);
        assert!(continuing.pending.is_none());
        assert_eq!(continuing.accepted, [None; 6]);
    }
    #[test]
    fn accepted_or_unavailable_post_retries_keep_exact_offer_slot_and_signed_bytes() {
        use std::{
            io::{Read, Write},
            net::TcpListener,
        };
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let peer = listener.local_addr().unwrap().to_string();
        let server = std::thread::spawn(move || {
            for status in [202, 503, 0, 202] {
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut header = Vec::new();
                while !header.ends_with(b"\r\n\r\n") {
                    let mut byte = [0];
                    socket.read_exact(&mut byte).unwrap();
                    header.push(byte[0]);
                }
                assert!(header.starts_with(b"POST /history/custody/ack?slot=4 "));
                let mut body = [0; 9];
                socket.read_exact(&mut body).unwrap();
                assert_eq!(&body, b"exact-ack");
                if status != 0 {
                    write!(
                        socket,
                        "HTTP/1.1 {status} Pending\r\nContent-Length: 0\r\n\r\n"
                    )
                    .unwrap();
                }
            }
        });
        let mut continuing = Continuing::new();
        continuing.slot = 4;
        continuing.pending = Some((b"original-offer".to_vec(), b"exact-ack".to_vec()));
        let absent = Path::new("/nonexistent/private-key");
        for _ in 0..4 {
            continuing.advance(absent, absent, &peer, absent).unwrap();
            assert_eq!(continuing.slot, 4);
            assert_eq!(continuing.pending.as_ref().unwrap().0, b"original-offer");
            assert_eq!(continuing.pending.as_ref().unwrap().1, b"exact-ack");
            assert_eq!(continuing.accepted, [None; 6]);
        }
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
                assert!(request
                    .starts_with(&format!("GET /history/custody/offer?slot={} ", attempt % 6)));
                socket
                    .write_all(if attempt < 6 {
                        b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\n\r\n"
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
        assert_eq!(continuing.slot, 1);
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
        continuing.advance(absent, absent, &peer, absent).unwrap();
        // Changing a transport endpoint preserves the same pending offer and
        // signed bytes; no key is reloaded after a response disappears.
        let alternate = TcpListener::bind("127.0.0.1:0").unwrap();
        let alternate_peer = alternate.local_addr().unwrap().to_string();
        let alternate_server = std::thread::spawn(move || {
            let (mut socket, _) = alternate.accept().unwrap();
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
        });
        continuing
            .advance(absent, absent, &alternate_peer, absent)
            .unwrap();
        alternate_server.join().unwrap();
        continuing.advance(absent, absent, &peer, absent).unwrap();
        assert_eq!(continuing.pending.as_ref().unwrap().0, b"original-offer");
        assert_eq!(continuing.pending.unwrap().1, b"exact-ack");
        assert_eq!(continuing.slot, 0);
        server.join().unwrap();
    }
    #[test]
    fn refused_pending_ack_rescans_without_claiming_custody_or_touching_files() {
        use std::{
            io::{Read, Write},
            net::TcpListener,
        };
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let peer = listener.local_addr().unwrap().to_string();
        let server = std::thread::spawn(move || {
            for expected in [
                "POST /history/custody/ack?slot=0 ",
                "GET /history/custody/offer?slot=1 ",
            ] {
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut request = Vec::new();
                while !request.ends_with(b"\r\n\r\n") {
                    let mut byte = [0];
                    socket.read_exact(&mut byte).unwrap();
                    request.push(byte[0]);
                }
                assert!(std::str::from_utf8(&request).unwrap().starts_with(expected));
                if expected.starts_with("POST") {
                    let mut body = [0; 9];
                    socket.read_exact(&mut body).unwrap();
                    assert_eq!(&body, b"exact-ack");
                }
                socket
                    .write_all(
                        b"HTTP/1.1 409 Conflict\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    )
                    .unwrap();
            }
        });
        let history =
            std::env::temp_dir().join(format!("custody-refusal-rescan-{}", std::process::id()));
        std::fs::create_dir(&history).unwrap();
        let retained = history.join("retained-observation");
        std::fs::write(&retained, b"original durable evidence").unwrap();
        let mut continuing = Continuing::new();
        continuing.pending = Some((b"original-offer".to_vec(), b"exact-ack".to_vec()));
        let absent = Path::new("/nonexistent/private-key");
        continuing.advance(absent, absent, &peer, &history).unwrap();
        assert!(continuing.pending.is_none());
        assert_eq!(continuing.slot, 1);
        assert_eq!(continuing.accepted, [None; 6]);
        continuing.advance(absent, absent, &peer, &history).unwrap();
        assert_eq!(continuing.slot, 2);
        assert!(!continuing.disabled);
        assert_eq!(continuing.accepted, [None; 6]);
        assert_eq!(
            std::fs::read(&retained).unwrap(),
            b"original durable evidence"
        );
        assert_eq!(std::fs::read_dir(&history).unwrap().count(), 1);
        server.join().unwrap();
        std::fs::remove_dir_all(history).unwrap();
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
