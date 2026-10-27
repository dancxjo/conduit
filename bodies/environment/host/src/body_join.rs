//! Private physical bootstrap followed by canonical, signed observer admission.
use crate::local_recorder::joined;
#[path = "body_join/recovery.rs"]
mod recovery_client;
use crate::{checked, Result};
use ed25519_dalek::Signer;
use recovery_client::recover;
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    path::Path,
    time::{Duration, Instant},
};
#[path = "../../../../targets/esp32/proof/tm1637/src/native_body_join_owner.rs"]
#[allow(dead_code)]
pub(crate) mod owner;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Bootstrap {
    invitation: conduit_body::PortableInvitation,
    root_key: [u8; 32],
    producer_host: conduit_core::HostId,
    producer_boot: conduit_core::BootId,
}
impl Drop for Bootstrap {
    fn drop(&mut self) {
        self.invitation.secret.fill(0);
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Challenge {
    schema: String,
    host: String,
    boot: String,
    body: String,
    root_key: [u8; 32],
    nonce: [u8; 32],
    valid_for_ms: u64,
    next_sequence: u64,
}
fn private(path: &Path, directory: bool) -> Result<()> {
    use std::os::unix::fs::MetadataExt;
    let m = std::fs::symlink_metadata(path)?;
    // SAFETY: observation of the process's effective local identity only.
    if m.uid() != unsafe { libc::geteuid() }
        || m.mode() & 0o077 != 0
        || if directory { !m.is_dir() } else { !m.is_file() }
    {
        return Err("join custody requires current-owner private files/directories".into());
    }
    Ok(())
}
fn save(path: &Path, value: &impl Serialize) -> Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    let parent = path.parent().ok_or("private output parent")?;
    private(parent, true)?;
    let mut nonce = 0u64;
    let (temporary, mut f) = loop {
        let temporary = path.with_extension(format!("pending-{}-{nonce}", std::process::id()));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&temporary)
        {
            Ok(file) => break (temporary, file),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => nonce += 1,
            Err(error) => return Err(error.into()),
        }
    };
    let result = (|| -> Result<()> {
        serde_json::to_writer(&mut f, value)?;
        f.write_all(b"\n")?;
        f.sync_all()?;
        // Publish only the complete synced inode, preserving no-overwrite custody.
        std::fs::hard_link(&temporary, path)?;
        std::fs::File::open(parent)?.sync_all()?;
        Ok(())
    })();
    let _ = std::fs::remove_file(temporary);
    result
}
fn load<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    use std::os::unix::fs::OpenOptionsExt;
    private(path.parent().ok_or("private input parent")?, true)?;
    private(path, false)?;
    let mut bytes = Vec::new();
    std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)?
        .take(16385)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 16384 {
        return Err("private bootstrap extent".into());
    }
    Ok(serde_json::from_slice(&bytes)?)
}
/// Sign only an exact epoch already authenticated and reopened from durable files.
/// The secret remains private to its bootstrap owner and never leaves this API.
#[allow(dead_code)] // Dormant until the producer exposes authenticated closed-epoch offers.
pub(crate) fn sign_durable_custody(
    binding: &Path,
    continuity_private: &Path,
    verified: &crate::observed_custody_client::VerifiedDurableEpoch,
) -> Result<Vec<u8>> {
    use sha2::{Digest, Sha256};
    let binding = crate::local_recorder::validated_binding(binding)?;
    let bootstrap: Bootstrap = load(continuity_private)?;
    checked(bootstrap.invitation.validate(0))?;
    if bootstrap.root_key != *binding.root()
        || bootstrap.invitation.claim != *binding.invitation()
        || verified.root() != binding.root()
        || verified.body_digest()
            != &<[u8; 32]>::from(Sha256::digest(
                binding.credential().body_id.as_str().as_bytes(),
            ))
    {
        return Err("durable custody bootstrap/binding/epoch scope differs".into());
    }
    let transcript = verified.signing_transcript(binding.credential())?;
    let signature = ed25519_dalek::SigningKey::from_bytes(&bootstrap.invitation.secret)
        .sign(&transcript)
        .to_bytes();
    verified.encode_ack(binding.credential(), signature)
}

/// Export the private bootstrap only into an explicitly selected local helper.
/// The public AP/LAN page never receives this bearer secret.
pub fn browser(input: &Path, output: &Path) -> Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    let bootstrap: Bootstrap = load(input)?;
    checked(bootstrap.invitation.validate(0))?;
    let json = serde_json::to_string(&bootstrap)?.replace('<', "\\u003c");
    let template = include_str!("body_join_browser.html");
    if template.matches("__PRIVATE_BOOTSTRAP_JSON__").count() != 1 {
        return Err("browser helper template marker".into());
    }
    let parent = output.parent().ok_or("private helper parent")?;
    private(parent, true)?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(output)?;
    file.write_all(
        template
            .replace("__PRIVATE_BOOTSTRAP_JSON__", &json)
            .as_bytes(),
    )?;
    file.sync_all()?;
    std::fs::File::open(parent)?.sync_all()?;
    Ok(())
}
pub fn issue(state: &Path, serial: &str, output: &Path) -> Result<()> {
    if serial.starts_with("tcp://") {
        return Err("invitation capture requires physical USB UART".into());
    }
    private(state, true)?;
    private(&state.join("owner-secret.bin"), false)?;
    let authority = crate::admission::Authority::new(state)?;
    let mut line = crate::transport::Line::open(serial)?;
    line.text("JOIN?")?;
    let challenge: Challenge = serde_json::from_value(
        line.schema_document("conduit.environment/private-invitation-challenge@1")?,
    )?;
    if challenge.schema != "conduit.environment/private-invitation-challenge@1"
        || challenge.nonce == [0; 32]
        || challenge.valid_for_ms != 30000
        || challenge.next_sequence == 0
        || challenge.host.is_empty()
        || challenge.boot.is_empty()
        || challenge.body.is_empty()
    {
        return Err("private invitation challenge".into());
    }
    let mut request = owner::Request {
        schema: owner::PROFILE.into(),
        host: challenge.host,
        boot: challenge.boot,
        body: challenge.body,
        root_key: challenge.root_key,
        nonce: challenge.nonce,
        sequence: challenge.next_sequence,
        ttl_ms: 300_000,
        signature: Vec::new(),
    };
    request.signature = authority
        .key
        .sign(&request.transcript())
        .to_bytes()
        .to_vec();
    line.text(&format!("JOIN {}", serde_json::to_string(&request)?))?;
    let mut bootstrap: Bootstrap =
        serde_json::from_value(line.schema_document(conduit_body::INVITATION_SCHEMA)?)?;
    let result = (|| {
        checked(bootstrap.invitation.validate(0))?;
        if bootstrap.root_key != request.root_key
            || bootstrap.producer_host.as_str() != request.host
            || bootstrap.producer_boot.as_str() != request.boot
            || bootstrap.invitation.claim.body_id.as_str() != request.body
        {
            return Err("private invitation scope changed".into());
        }
        save(output, &bootstrap)
    })();
    bootstrap.invitation.secret.fill(0);
    result
}
fn compact_request(request: &conduit_body::PortableSpawnAdmissionRequest) -> Result<Vec<u8>> {
    let hex = |bytes: &[u8]| {
        bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    };
    let mut value = serde_json::to_value(request)?;
    value["schema"] = serde_json::json!("conduit.environment/observer-admission@1");
    value["nonce"] = serde_json::json!(hex(&request.nonce));
    value["signature"] = serde_json::json!(hex(&request.signature));
    Ok(serde_json::to_vec(&value)?)
}
pub(crate) fn http(peer: &str, method: &str, path: &str, body: &[u8]) -> Result<(u16, Vec<u8>)> {
    use std::net::ToSocketAddrs;
    if peer.len() > 255 || peer.contains(['/', '@', '\r', '\n']) {
        return Err("join peer must be HOST:PORT".into());
    }
    let mut last = None;
    for address in peer.to_socket_addrs()?.filter(|a| a.is_ipv4()).take(8) {
        let response = (|| -> Result<_> {
            let mut s = std::net::TcpStream::connect_timeout(&address, Duration::from_secs(3))?;
            s.set_read_timeout(Some(Duration::from_secs(3)))?;
            s.set_write_timeout(Some(Duration::from_secs(3)))?;
            let request_header = format!("{method} {path} HTTP/1.1\r\nHost: {peer}\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",body.len());
            if request_header.len() + body.len() > 1024 {
                return Err("join request exceeds native1024-byte frame".into());
            }
            s.write_all(request_header.as_bytes())?;
            s.write_all(body)?;
            let mut bytes = Vec::new();
            let deadline = Instant::now() + Duration::from_secs(3);
            let mut block = [0; 1024];
            loop {
                s.set_read_timeout(Some(
                    deadline
                        .checked_duration_since(Instant::now())
                        .ok_or("join HTTP deadline")?,
                ))?;
                let n = s.read(&mut block)?;
                if n == 0 {
                    break;
                }
                if bytes.len() + n > 16384 {
                    return Err("join HTTP extent".into());
                }
                bytes.extend_from_slice(&block[..n]);
            }
            if bytes.len() > 16384 {
                return Err("join HTTP extent".into());
            }
            let at = bytes
                .windows(4)
                .position(|w| w == b"\r\n\r\n")
                .ok_or("join HTTP framing")?;
            let header = std::str::from_utf8(&bytes[..at])?;
            let status = header
                .lines()
                .next()
                .and_then(|v| v.split_whitespace().nth(1))
                .ok_or("join HTTP status")?
                .parse()?;
            Ok((status, bytes[at + 4..].to_vec()))
        })();
        match response {
            Ok(r) => return Ok(r),
            Err(e) => last = Some(e),
        }
    }
    Err(last.unwrap_or_else(|| "join peer has no bounded IPv4 route".into()))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Ticket {
    schema: String,
    ticket: u32,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pending {
    ticket: u32,
    local_host: conduit_core::HostId,
    local_boot: conduit_core::BootId,
}
#[derive(Serialize)]
struct Binding<'a> {
    schema: &'static str,
    invitation: &'a conduit_body::SpawnInvitationClaim,
    root_key: [u8; 32],
    local_host: &'a conduit_core::HostId,
    local_boot: &'a conduit_core::BootId,
    receipt: &'a joined::JoinedHostReceipt,
}
pub fn join(input: &Path, state: &Path, peer: &str) -> Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    let bootstrap: Bootstrap = load(input)?;
    checked(bootstrap.invitation.validate(0))?;
    if state.exists() {
        private(state, true)?;
        if state.join("continuity-private.json").exists() {
            let retained: Bootstrap = load(&state.join("continuity-private.json"))?;
            if serde_json::to_vec(&bootstrap)? != serde_json::to_vec(&retained)? {
                return Err("retained invitation differs".into());
            }
        }
        if state.join("recorder-binding.json").exists() {
            let binding: serde_json::Value = load(&state.join("recorder-binding.json"))?;
            if binding["root_key"] != serde_json::to_value(bootstrap.root_key)?
                || binding["invitation"] != serde_json::to_value(&bootstrap.invitation.claim)?
            {
                return Err("retained recorder binding differs".into());
            }
            crate::local_recorder::admitted_collector(&state.join("recorder-binding.json"))?;
            return Ok(());
        }
        if state.join("pending-ticket.json").exists() {
            let pending: Pending = load(&state.join("pending-ticket.json"))?;
            return finish(&bootstrap, state, peer, pending);
        }
        // A lost POST reply must replay the exact original signed request,
        // never allocate another Boot identity or consume another invitation.
        if state.join("submission-private.json").exists() {
            let request = load(&state.join("submission-private.json"))?;
            return submit(&bootstrap, state, peer, request);
        }
        // Nothing could have been submitted before the durable submission file.
    } else {
        std::fs::DirBuilder::new().mode(0o700).create(state)?;
        std::fs::File::open(
            state
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new(".")),
        )?
        .sync_all()?;
    }
    private(state, true)?;
    let host: conduit_core::HostId = format!(
        "linux/{}",
        std::fs::read_to_string("/etc/machine-id")?.trim()
    )
    .into();
    let boot: conduit_core::BootId = format!(
        "boot/{}",
        std::fs::read_to_string("/proc/sys/kernel/random/uuid")?.trim()
    )
    .into();
    let advertisement = conduit_core::HostAdvertisement {
        protocol_version: conduit_core::PROTOCOL_VERSION,
        host_id: host.clone(),
        boot_id: boot.clone(),
        offer_generation: conduit_core::OfferGeneration(1),
        profile: "conduit/environment/history-observer@1".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    let claim = &bootstrap.invitation.claim;
    let signature = ed25519_dalek::SigningKey::from_bytes(&bootstrap.invitation.secret)
        .sign(&claim.signing_transcript(&host, &boot, advertisement.offer_generation))
        .to_bytes()
        .to_vec();
    let request = conduit_body::PortableSpawnAdmissionRequest {
        schema: conduit_body::SPAWN_ADMISSION_REQUEST_SCHEMA.into(),
        invitation_id: claim.invitation_id.clone(),
        body_id: claim.body_id.clone(),
        host_advertisement: advertisement,
        nonce: claim.nonce,
        signature,
        membership_admitted: false,
        plan_created: false,
        play_created: false,
    };
    // Retain continuity authority before a possibly successful irreversible admission.
    if !state.join("continuity-private.json").exists() {
        save(&state.join("continuity-private.json"), &bootstrap)?;
    }
    save(&state.join("submission-private.json"), &request)?;
    submit(&bootstrap, state, peer, request)
}
fn submit(
    bootstrap: &Bootstrap,
    state: &Path,
    peer: &str,
    request: conduit_body::PortableSpawnAdmissionRequest,
) -> Result<()> {
    let claim = &bootstrap.invitation.claim;
    let advertisement = &request.host_advertisement;
    if request.invitation_id != claim.invitation_id
        || request.body_id != claim.body_id
        || request.nonce != claim.nonce
    {
        return Err("retained submission invitation scope".into());
    }
    let signature: [u8; 64] = request
        .signature
        .as_slice()
        .try_into()
        .map_err(|_| "retained submission signature extent")?;
    checked(
        ed25519_dalek::SigningKey::from_bytes(&bootstrap.invitation.secret)
            .verifying_key()
            .verify_strict(
                &claim.signing_transcript(
                    &advertisement.host_id,
                    &advertisement.boot_id,
                    advertisement.offer_generation,
                ),
                &ed25519_dalek::Signature::from_bytes(&signature),
            ),
    )?;
    let (status, bytes) = http(peer, "POST", "/body/join", &compact_request(&request)?)?;
    if status != 202 {
        return Err(format!("join submission HTTP {status}").into());
    }
    let ticket: Ticket = serde_json::from_slice(&bytes)?;
    if ticket.schema != "conduit.environment/body-join-queued@1" {
        return Err("join queue profile".into());
    }
    let pending = Pending {
        ticket: ticket.ticket,
        local_host: request.host_advertisement.host_id,
        local_boot: request.host_advertisement.boot_id,
    };
    save(&state.join("pending-ticket.json"), &pending)?;
    finish(bootstrap, state, peer, pending)
}
fn finish(bootstrap: &Bootstrap, state: &Path, peer: &str, pending: Pending) -> Result<()> {
    let ticket = Ticket {
        schema: "conduit.environment/body-join-queued@1".into(),
        ticket: pending.ticket,
    };
    let (host, boot) = (pending.local_host, pending.local_boot);
    let claim = &bootstrap.invitation.claim;
    let deadline = Instant::now() + Duration::from_secs(60);
    let receipt = loop {
        let response = http(
            peer,
            "GET",
            &format!("/body/join/result?id={}", ticket.ticket),
            &[],
        );
        let (status, bytes) = match response {
            Ok(response) => response,
            Err(_) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_secs(1));
                continue;
            }
            Err(error) => return Err(error),
        };
        if status == 200 {
            break serde_json::from_slice::<joined::JoinedHostReceipt>(&bytes)?;
        }
        if status == 404 || status == 403 {
            return recover(bootstrap, state, peer, &host, &boot);
        }
        if status != 202 || Instant::now() >= deadline {
            return Err(
                format!("join result HTTP {status}; private pending ticket retained").into(),
            );
        }
        let pending: Ticket = serde_json::from_slice(&bytes)?;
        if pending.schema != ticket.schema || pending.ticket != ticket.ticket {
            return Err("join ticket changed".into());
        }
        std::thread::sleep(Duration::from_millis(250));
    };
    checked(receipt.verify(&bootstrap.root_key))?;
    if receipt.invitation_id != claim.invitation_id
        || receipt.credential.body_id != claim.body_id
        || receipt.credential.host_id != host
        || receipt.credential.boot_id != boot
        || receipt.producer_host != bootstrap.producer_host
        || receipt.producer_boot != bootstrap.producer_boot
        || receipt.offer_generation.0 != 1
        || receipt.credential.issued_at_millis > claim.expires_at_millis
    {
        return Err("joined receipt scope".into());
    }
    save(
        &state.join("recorder-binding.json"),
        &Binding {
            schema: "conduit.environment/joined-recorder@1",
            invitation: claim,
            root_key: bootstrap.root_key,
            local_host: &host,
            local_boot: &boot,
            receipt: &receipt,
        },
    )?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    pub(super) fn directory(label: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("conduit-join-{label}-{}", std::process::id()));
        if p.exists() {
            std::fs::remove_dir_all(&p).unwrap()
        }
        std::fs::DirBuilder::new().mode(0o700).create(&p).unwrap();
        p
    }
    #[test]
    fn private_custody_refuses_overwrite_public_and_symlink() {
        let dir = directory("custody");
        let path = dir.join("secret.json");
        save(&path, &serde_json::json!({"secret":1})).unwrap();
        assert!(save(&path, &serde_json::json!({"secret":2})).is_err());
        assert_eq!(load::<serde_json::Value>(&path).unwrap()["secret"], 1);
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(load::<serde_json::Value>(&path).is_err());
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let link = dir.join("link");
        std::os::unix::fs::symlink(&path, &link).unwrap();
        assert!(load::<serde_json::Value>(&link).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn incomplete_private_state_is_never_published_and_stale_temporary_cannot_block_retry() {
        struct Interrupted;
        impl Serialize for Interrupted {
            fn serialize<S: serde::Serializer>(
                &self,
                serializer: S,
            ) -> std::result::Result<S::Ok, S::Error> {
                use serde::ser::{Error, SerializeSeq};
                let mut seq = serializer.serialize_seq(Some(2))?;
                seq.serialize_element(&"partial")?;
                Err(S::Error::custom("interrupted serializer"))
            }
        }
        let dir = directory("incomplete");
        let path = dir.join("private.json");
        assert!(save(&path, &Interrupted).is_err());
        assert!(!path.exists());
        std::fs::write(
            path.with_extension(format!("pending-{}-0", std::process::id())),
            b"torn",
        )
        .unwrap();
        save(&path, &serde_json::json!({"complete":true})).unwrap();
        assert_eq!(load::<serde_json::Value>(&path).unwrap()["complete"], true);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn actual_http_join_writes_only_verified_binding_and_retains_private_continuity() {
        http_join(false, false, false);
    }
    #[test]
    fn interrupted_join_resumes_exact_retained_ticket_without_resubmission() {
        http_join(true, false, false);
    }
    #[test]
    fn root_signed_receipt_for_other_body_never_creates_binding() {
        http_join(false, true, false);
    }
    #[test]
    fn lost_submission_reply_replays_exact_durable_request_and_completed_join_is_idempotent() {
        http_join(false, false, true);
    }
    fn http_join(resume: bool, wrong_body: bool, lost_reply: bool) {
        let dir = directory(if wrong_body {
            "wrong-body"
        } else if lost_reply {
            "lost-reply"
        } else if resume {
            "resume"
        } else {
            "http"
        });
        let input = dir.join("bootstrap.json");
        let state = dir.join("observer");
        let key = ed25519_dalek::SigningKey::from_bytes(&[41; 32]);
        let bootstrap:Bootstrap=serde_json::from_value(serde_json::json!({
            "invitation":{"schema":conduit_body::INVITATION_SCHEMA,"claim":{"invitation_id":"08".repeat(32),"body_id":"02".repeat(32),"nonce":vec![3;32],"expires_at_millis":999999},"secret":vec![22;32]},
            "root_key":key.verifying_key().to_bytes(),"producer_host":"06".repeat(32),"producer_boot":"07".repeat(32)
        })).unwrap();
        save(&input, &bootstrap).unwrap();
        let malicious_input = dir.join("html-input.json");
        let mut html_bootstrap = serde_json::to_value(&bootstrap).unwrap();
        html_bootstrap["producer_host"] = serde_json::json!("</script><script>untrusted</script>");
        save(&malicious_input, &html_bootstrap).unwrap();
        let helper = dir.join("helper.html");
        browser(&malicious_input, &helper).unwrap();
        let html = std::fs::read_to_string(&helper).unwrap();
        assert!(html.contains("\\u003c/script>"));
        assert!(!html.contains("<script>untrusted"));
        assert!(!html.contains("__PRIVATE_BOOTSTRAP_JSON__"));
        private(&helper, false).unwrap();
        assert!(browser(&malicious_input, &helper).is_err());

        let retained_request = if resume {
            std::fs::DirBuilder::new()
                .mode(0o700)
                .create(&state)
                .unwrap();
            save(&state.join("continuity-private.json"), &bootstrap).unwrap();
            let pending = Pending {
                ticket: 7,
                local_host: "retained-host".into(),
                local_boot: "retained-boot".into(),
            };
            save(&state.join("pending-ticket.json"), &pending).unwrap();
            Some(conduit_body::PortableSpawnAdmissionRequest {
                schema: conduit_body::SPAWN_ADMISSION_REQUEST_SCHEMA.into(),
                invitation_id: bootstrap.invitation.claim.invitation_id.clone(),
                body_id: bootstrap.invitation.claim.body_id.clone(),
                nonce: bootstrap.invitation.claim.nonce,
                signature: vec![],
                membership_admitted: false,
                plan_created: false,
                play_created: false,
                host_advertisement: conduit_core::HostAdvertisement {
                    protocol_version: conduit_core::PROTOCOL_VERSION,
                    host_id: pending.local_host,
                    boot_id: pending.local_boot,
                    offer_generation: conduit_core::OfferGeneration(1),
                    profile: "observer".into(),
                    bases: vec![],
                    resources: vec![],
                    capabilities: vec![],
                    planner_capabilities: vec![],
                },
            })
        } else {
            None
        };
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let peer = listener.local_addr().unwrap().to_string();
        let server = std::thread::spawn(move || {
            let mut request = retained_request;
            let mut original_post = None;
            for step in if resume {
                1..2
            } else {
                0..if lost_reply { 3 } else { 2 }
            } {
                let round = if lost_reply && step == 1 {
                    0
                } else if lost_reply && step == 2 {
                    1
                } else {
                    step
                };
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut bytes = Vec::new();
                let mut byte = [0];
                while !bytes.ends_with(b"\r\n\r\n") {
                    socket.read_exact(&mut byte).unwrap();
                    bytes.push(byte[0]);
                }
                let header = String::from_utf8(bytes).unwrap();
                let length: usize = header
                    .lines()
                    .find_map(|l| l.strip_prefix("Content-Length: "))
                    .unwrap()
                    .parse()
                    .unwrap();
                let mut body = vec![0; length];
                socket.read_exact(&mut body).unwrap();
                let (status, body) = if round == 0 {
                    assert!(header.starts_with("POST /body/join "));
                    if let Some(original) = &original_post {
                        assert_eq!(original, &body);
                    } else {
                        original_post = Some(body.clone());
                    }
                    let mut wire: serde_json::Value = serde_json::from_slice(&body).unwrap();
                    assert_eq!(wire["schema"], "conduit.environment/observer-admission@1");
                    wire["schema"] =
                        serde_json::json!(conduit_body::SPAWN_ADMISSION_REQUEST_SCHEMA);
                    for field in ["nonce", "signature"] {
                        let hex = wire[field].as_str().unwrap();
                        let bytes: Vec<u8> = (0..hex.len())
                            .step_by(2)
                            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
                            .collect();
                        wire[field] = serde_json::json!(bytes);
                    }
                    let r: conduit_body::PortableSpawnAdmissionRequest =
                        serde_json::from_value(wire).unwrap();
                    assert!(r.host_advertisement.capabilities.is_empty());
                    assert!(!r.membership_admitted);
                    let signature: [u8; 64] = r.signature.as_slice().try_into().unwrap();
                    ed25519_dalek::SigningKey::from_bytes(&[22; 32])
                        .verifying_key()
                        .verify_strict(
                            &bootstrap.invitation.claim.signing_transcript(
                                &r.host_advertisement.host_id,
                                &r.host_advertisement.boot_id,
                                r.host_advertisement.offer_generation,
                            ),
                            &ed25519_dalek::Signature::from_bytes(&signature),
                        )
                        .unwrap();
                    request = Some(r);
                    if lost_reply && step == 0 {
                        continue; // accepted request, lost ticket reply
                    }
                    (202,serde_json::to_vec(&serde_json::json!({"schema":"conduit.environment/body-join-queued@1","ticket":7})).unwrap())
                } else {
                    assert!(header.starts_with("GET /body/join/result?id=7 "));
                    let r = request.as_ref().unwrap();
                    let credential=serde_json::from_value(serde_json::json!({"credential_id":"01".repeat(32),"body_id":r.body_id,"part_id":"03".repeat(32),"host_id":r.host_advertisement.host_id,"boot_id":r.host_advertisement.boot_id,"issued_at_millis":123})).unwrap();
                    let mut receipt = joined::JoinedHostReceipt {
                        schema: joined::PROFILE.into(),
                        credential,
                        membership_revision: 2,
                        producer_host: bootstrap.producer_host.clone(),
                        producer_boot: bootstrap.producer_boot.clone(),
                        producer_key: bootstrap.root_key,
                        invitation_id: r.invitation_id.clone(),
                        offer_generation: r.host_advertisement.offer_generation,
                        signature: vec![],
                    };
                    if wrong_body {
                        receipt.credential.body_id =
                            serde_json::from_value(serde_json::json!("09".repeat(32))).unwrap();
                    }
                    receipt.signature = key
                        .sign(&joined::transcript(&receipt.scope()))
                        .to_bytes()
                        .to_vec();
                    (200, serde_json::to_vec(&receipt).unwrap())
                };
                write!(
                    socket,
                    "HTTP/1.1 {status} OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .unwrap();
                socket.write_all(&body).unwrap();
            }
        });
        if lost_reply {
            assert!(join(&input, &state, &peer).is_err());
            assert!(state.join("submission-private.json").is_file());
            assert!(!state.join("pending-ticket.json").exists());
        }
        let result = join(&input, &state, &peer);
        server.join().unwrap();
        if wrong_body {
            assert!(result.is_err());
            assert!(!state.join("recorder-binding.json").exists());
            assert!(load::<Bootstrap>(&state.join("continuity-private.json")).is_ok());
            std::fs::remove_dir_all(dir).unwrap();
            return;
        }
        result.unwrap();
        let binding: serde_json::Value = load(&state.join("recorder-binding.json")).unwrap();
        assert_eq!(binding["schema"], "conduit.environment/joined-recorder@1");
        assert!(binding.get("secret").is_none());
        assert!(load::<Bootstrap>(&state.join("continuity-private.json")).is_ok());
        // No network is needed once the exact signed binding is durable.
        join(&input, &state, "127.0.0.1:1").unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[cfg(test)]
#[path = "body_join/custody.rs"]
mod custody_tests;
