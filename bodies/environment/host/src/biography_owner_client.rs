//! Explicit physical-owner biography retention. Durable custody is not membership.
use crate::{
    admission::Authority,
    biography_custody_remote::{wire, VerifiedDurableBiography},
    body_join::owner,
    checked,
    transport::Line,
    Result,
};
use ed25519_dalek::Signer;
use std::{
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
    time::{Duration, Instant},
};
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Pin {
    body: conduit_body::BodyId,
    root_key: [u8; 32],
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Offered {
    event: String,
    offer: wire::Offer,
    challenge: owner::BiographyChallenge,
}
fn private(path: &Path, directory: bool) -> Result<()> {
    let m = std::fs::symlink_metadata(path)?;
    // SAFETY: observes this process's effective identity only.
    if m.uid() != unsafe { libc::geteuid() }
        || m.mode() & 0o077 != 0
        || if directory { !m.is_dir() } else { !m.is_file() }
    {
        return Err("biography owner requires private current-owner files/directories".into());
    }
    Ok(())
}
fn load_pin(path: &Path) -> Result<Pin> {
    private(path.parent().ok_or("biography pin parent")?, true)?;
    private(path, false)?;
    let mut bytes = Vec::new();
    std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)?
        .take(4097)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 4096 {
        return Err("biography pin extent".into());
    }
    let pin: Pin = serde_json::from_slice(&bytes)?;
    checked(ed25519_dalek::VerifyingKey::from_bytes(&pin.root_key))?;
    Ok(pin)
}
trait Carrier {
    fn text(&mut self, text: &str) -> Result<()>;
    fn event(&mut self, name: &str) -> Result<serde_json::Value>;
    fn exact(&mut self, size: usize) -> Result<Vec<u8>>;
}
impl Carrier for Line {
    fn text(&mut self, text: &str) -> Result<()> {
        Line::text(self, text)
    }
    fn event(&mut self, name: &str) -> Result<serde_json::Value> {
        Line::event(self, name)
    }
    fn exact(&mut self, size: usize) -> Result<Vec<u8>> {
        Line::exact(self, size)
    }
}
fn offer(line: &mut impl Carrier, pin: &Pin) -> Result<Offered> {
    let offered: Offered = serde_json::from_value(line.event("biography-offer")?)?;
    if offered.event != "biography-offer" {
        return Err("biography offer event".into());
    }
    let root = checked(ed25519_dalek::VerifyingKey::from_bytes(&pin.root_key))?;
    let id = checked(offered.offer.verify(&pin.body, &root))?;
    let challenge = &offered.challenge;
    if challenge.body != pin.body.as_str()
        || challenge.root_key != pin.root_key
        || challenge.offer != id
    {
        return Err("biography challenge pinned scope".into());
    }
    checked(
        owner::BiographyRequest {
            schema: owner::BIOGRAPHY_PROFILE.into(),
            challenge: challenge.clone(),
            signature: vec![],
        }
        .transcript(),
    )?;
    Ok(offered)
}
fn stream(line: &mut impl Carrier) -> Result<Vec<u8>> {
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut bytes = Vec::new();
    for _ in 0..=131072 {
        if Instant::now() >= deadline {
            return Err("biography stream deadline".into());
        }
        let length = u32::from_le_bytes(
            line.exact(4)?
                .try_into()
                .map_err(|_| "biography frame header")?,
        ) as usize;
        if length == 0 {
            if line.exact(1)? != b"\n" {
                return Err("biography frame terminator".into());
            }
            if Instant::now() >= deadline {
                return Err("biography stream deadline".into());
            }
            return Ok(bytes);
        }
        if length > 128
            || bytes
                .len()
                .checked_add(length)
                .ok_or("biography frame extent")?
                > 131072
        {
            return Err("biography frame extent".into());
        }
        let chunk = line.exact(length)?;
        if chunk.len() != length {
            return Err("biography short frame".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    Err("biography frame count".into())
}
fn exchange(
    line: &mut impl Carrier,
    pin: &Pin,
    key: &ed25519_dalek::SigningKey,
    directory: &Path,
) -> Result<u64> {
    // One bounded wait observes the producer's explicit pre-resume window.
    // Opening a carrier is not permission to replay an earlier BIO exchange.
    line.event("biography-window")?;
    line.text("BIO?")?;
    let initial = offer(line, pin)?;
    line.text("BIO DATA")?;
    line.event("biography-data")?;
    let bytes = stream(line)?;
    let durable = VerifiedDurableBiography::retain(
        &initial.offer,
        &pin.body,
        &pin.root_key,
        &bytes,
        directory,
    )?;
    // The durable token has reopened/fsynced exact canonical custody before a
    // fresh nonce is requested. Retrying never replays an ambiguous owner ACK.
    line.text("BIO CHALLENGE")?;
    let fresh = offer(line, pin)?;
    if serde_json::to_vec(&fresh.offer)? != serde_json::to_vec(&initial.offer)?
        || fresh.challenge.host != initial.challenge.host
        || fresh.challenge.boot != initial.challenge.boot
        || fresh.challenge.sequence != initial.challenge.sequence
        || fresh.challenge.nonce == initial.challenge.nonce
    {
        return Err("biography offer or producer changed during retention".into());
    }
    let draft = durable.owner_draft(fresh.challenge)?;
    let signature = key.sign(&draft.signing_transcript()?).to_bytes();
    let request = draft.encode(signature)?;
    line.text(&format!("BIO ACK {}", std::str::from_utf8(&request)?))?;
    let retained = line.event("biography-retained")?;
    retained["generation"]
        .as_u64()
        .filter(|g| *g > initial.offer.generation)
        .ok_or_else(|| "biography retained generation".into())
}
pub(crate) fn run(state: &Path, serial: &str, directory: &Path, pin_path: &Path) -> Result<()> {
    // This physical-owner entrance never follows a network carrier or ambient peer.
    if serial.starts_with("tcp://") {
        return Err("biography owner requires physical serial custody".into());
    }
    // Validate local custody before opening a serial port, which may reset some MCUs.
    let pin = load_pin(pin_path)?;
    private(state, true)?;
    private(&state.join("owner-secret.bin"), false)?;
    let authority = Authority::new(state)?;
    let mut line = Line::open(serial)?;
    let generation = exchange(&mut line, &pin, &authority.key, directory)?;
    println!("Biography retained durably; producer checkpoint generation {generation}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::VecDeque, os::unix::fs::PermissionsExt};
    struct Fake {
        events: VecDeque<serde_json::Value>,
        bytes: Vec<u8>,
        at: usize,
        commands: Vec<String>,
    }
    impl Carrier for Fake {
        fn text(&mut self, text: &str) -> Result<()> {
            self.commands.push(text.into());
            Ok(())
        }
        fn event(&mut self, name: &str) -> Result<serde_json::Value> {
            let value = self.events.pop_front().ok_or("missing event")?;
            if value["event"] != name {
                return Err("refused or wrong event".into());
            }
            Ok(value)
        }
        fn exact(&mut self, size: usize) -> Result<Vec<u8>> {
            let end = self.at.checked_add(size).ok_or("extent")?;
            let result = self.bytes.get(self.at..end).ok_or("short frame")?.to_vec();
            self.at = end;
            Ok(result)
        }
    }
    fn frames(bytes: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        for chunk in bytes.chunks(97) {
            out.extend_from_slice(&(chunk.len() as u32).to_le_bytes());
            out.extend_from_slice(chunk);
        }
        out.extend_from_slice(&[0; 4]);
        out.push(b'\n');
        out
    }
    fn fixture() -> (Pin, wire::Offer, Vec<u8>) {
        use conduit_body as b;
        let plots = b::BodyWorkset::from_plots([b::ResidentPlot::new(
            "source/test".into(),
            "checked/test".into(),
        )])
        .unwrap();
        let body = b::Body::born_with_plots(plots, 1, "sign/birth".into()).unwrap();
        let membership = b::BodyMembership::new(body.body_id.clone()).unwrap();
        let evidence =
            b::BodyBiographyEvidence::born(body, membership, "Owner custody fixture".into())
                .unwrap();
        let host = conduit_core::HostAdvertisement {
            declared_name: None,
            protocol_version: conduit_core::PROTOCOL_VERSION,
            host_id: "host/custody".into(),
            boot_id: "boot/0".into(),
            offer_generation: conduit_core::OfferGeneration(1),
            profile: "fixture".into(),
            bases: vec![],
            resources: vec![],
            capabilities: vec![],
            planner_capabilities: vec![],
        };
        let mut body =
            conduit_environment_runtime::body_image::open_installed_image(evidence, &host).unwrap();
        let mut segments = Vec::new();
        for i in 1..120 {
            body = b::BodyLifecycleSession::resume_here(
                body.evidence().clone(),
                &host.host_id,
                &format!("boot/{i}").into(),
            )
            .unwrap();
            if !body.pending_archives().is_empty() {
                segments.extend_from_slice(body.pending_archives());
                let head = body.pending_archives().last().unwrap().digest;
                body.acknowledge_archives(head).unwrap();
                if !segments.is_empty() {
                    break;
                }
            }
        }
        assert!(!segments.is_empty());
        let root = ed25519_dalek::SigningKey::from_bytes(&[17; 32]);
        let pin = Pin {
            body: segments[0].body_id.clone(),
            root_key: root.verifying_key().to_bytes(),
        };
        let bytes = serde_json::to_vec(&segments).unwrap();
        let mut end = wire::Summary::default();
        for segment in &segments {
            end.append(&pin.body, segment).unwrap();
        }
        use sha2::{Digest, Sha256};
        let mut offered = wire::Offer {
            schema: wire::OFFER.into(),
            body: pin.body.clone(),
            generation: 7,
            source: wire::Hex([19; 32]),
            base: wire::Summary::default(),
            end,
            stream: wire::Hex(Sha256::digest(&bytes).into()),
            signature: wire::Hex([0; 64]),
        };
        offered.signature = wire::Hex(root.sign(&offered.transcript(&pin.root_key)).to_bytes());
        (pin, offered, bytes)
    }
    fn offered(pin: &Pin, offer: &wire::Offer, nonce: u8) -> serde_json::Value {
        serde_json::json!({"event":"biography-offer","offer":offer,"challenge":{"host":"host/actual","boot":"boot/actual","body":pin.body,"root_key":pin.root_key,"nonce":vec![nonce;32],"sequence":1,"offer":offer.transcript(&pin.root_key)}})
    }
    fn directory() -> std::path::PathBuf {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        std::env::temp_dir().join(format!(
            "conduit-owner-biography-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        ))
    }
    #[test]
    fn exact_durable_capture_precedes_fresh_typed_owner_signature() {
        let (pin, offer, bytes) = fixture();
        let dir = directory();
        let mut line = Fake {
            events: [
                serde_json::json!({"event":"biography-window"}),
                offered(&pin, &offer, 1),
                serde_json::json!({"event":"biography-data"}),
                offered(&pin, &offer, 2),
                serde_json::json!({"event":"biography-retained","generation":8}),
            ]
            .into(),
            bytes: frames(&bytes),
            at: 0,
            commands: vec![],
        };
        let key = ed25519_dalek::SigningKey::from_bytes(&[22; 32]);
        assert_eq!(exchange(&mut line, &pin, &key, &dir).unwrap(), 8);
        assert_eq!(&line.commands[..3], &["BIO?", "BIO DATA", "BIO CHALLENGE"]);
        let ack = line.commands[3].strip_prefix("BIO ACK ").unwrap();
        let request: owner::BiographyRequest = serde_json::from_str(ack).unwrap();
        assert_eq!(request.challenge.nonce, [2; 32]);
        let signature: [u8; 64] = request.signature.as_slice().try_into().unwrap();
        key.verifying_key()
            .verify_strict(
                &request.transcript().unwrap(),
                &ed25519_dalek::Signature::from_bytes(&signature),
            )
            .unwrap();
        assert!(std::fs::read_dir(&dir).unwrap().count() >= 2);
        assert_eq!(
            std::fs::metadata(&dir).unwrap().permissions().mode() & 0o077,
            0
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn changed_producer_offer_nonce_or_stream_never_sends_an_ack() {
        let (pin, offer, bytes) = fixture();
        for case in 0..6 {
            let dir = directory();
            let initial = offered(&pin, &offer, 1);
            let mut fresh = offered(&pin, &offer, 2);
            match case {
                0 => fresh["challenge"]["boot"] = serde_json::json!("boot/other"),
                1 => fresh["challenge"]["host"] = serde_json::json!("host/other"),
                2 => fresh["challenge"]["nonce"] = serde_json::json!(vec![1; 32]),
                3 => fresh["offer"]["generation"] = serde_json::json!(8),
                _ => (),
            }
            let mut raw = bytes.clone();
            if case == 4 {
                raw[0] = b'!';
            }
            let initial = if case == 5 {
                let mut wrong = initial;
                wrong["challenge"]["root_key"] = serde_json::json!(vec![9; 32]);
                wrong
            } else {
                initial
            };
            let mut line = Fake {
                events: [
                    serde_json::json!({"event":"biography-window"}),
                    initial,
                    serde_json::json!({"event":"biography-data"}),
                    fresh,
                ]
                .into(),
                bytes: frames(&raw),
                at: 0,
                commands: vec![],
            };
            assert!(exchange(
                &mut line,
                &pin,
                &ed25519_dalek::SigningKey::from_bytes(&[22; 32]),
                &dir
            )
            .is_err());
            assert!(line.commands.iter().all(|s| !s.starts_with("BIO ACK")));
            if dir.exists() {
                std::fs::remove_dir_all(dir).unwrap();
            }
        }
    }
    #[test]
    fn physical_owner_entrance_refuses_tcp_before_loading_state_or_opening_any_carrier() {
        let missing = Path::new("/nonexistent/biography-owner-fixture");
        let error = run(missing, "tcp://127.0.0.1:1", missing, missing).unwrap_err();
        assert!(error.to_string().contains("physical serial custody"));
        // Local invalid custody also fails before trying the deliberately absent UART.
        assert!(run(missing, "/nonexistent/uart-fixture", missing, missing).is_err());
    }
    #[test]
    fn framing_and_private_pin_refuse_wrong_extents_and_types() {
        for bytes in [vec![129, 0, 0, 0], vec![1, 0, 0], vec![0, 0, 0, 0, b'x']] {
            let mut line = Fake {
                events: VecDeque::new(),
                bytes,
                at: 0,
                commands: vec![],
            };
            assert!(stream(&mut line).is_err());
        }
        let mut oversized = Vec::new();
        for _ in 0..1025 {
            oversized.extend_from_slice(&128u32.to_le_bytes());
            oversized.extend_from_slice(&[1; 128]);
        }
        assert!(stream(&mut Fake {
            events: VecDeque::new(),
            bytes: oversized,
            at: 0,
            commands: vec![]
        })
        .is_err());
        let (pin, _, _) = fixture();
        let dir = directory();
        std::fs::create_dir(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        let path = dir.join("pin.json");
        std::fs::write(
            &path,
            serde_json::to_vec(&serde_json::json!({"body":pin.body,"root_key":pin.root_key}))
                .unwrap(),
        )
        .unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        assert!(load_pin(&path).is_ok());
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(load_pin(&path).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
