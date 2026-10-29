//! Foreground recorder loop. Caller supplies an already admitted collector;
//! resolving a peer address or reading a signed directory grants no membership.
use crate::{observed_history::Collector, Result};
use std::{net::ToSocketAddrs, path::Path, time::Duration};
#[cfg(test)]
#[path = "local_recorder/custody_journal_prototype.rs"]
mod custody_journal_prototype;
#[path = "../../../../targets/esp32/proof/tm1637/src/native_joined_host_wire.rs"]
pub(crate) mod joined;
#[path = "../../../../targets/esp32/proof/tm1637/src/native_joined_host_recovery.rs"]
#[allow(dead_code)] // Producer-only verification is exercised by actual Store tests.
pub(crate) mod recovery;
#[derive(serde::Deserialize)]
#[serde(untagged)]
enum Receipt {
    Admission(joined::JoinedHostReceipt),
    Current(recovery::Receipt),
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Binding {
    schema: String,
    invitation: conduit_body::SpawnInvitationClaim,
    root_key: [u8; 32],
    local_host: conduit_core::HostId,
    local_boot: conduit_core::BootId,
    receipt: Receipt,
    #[serde(default)]
    recovery_request: Option<recovery::Request>,
}
fn private_metadata(path: &Path, directory: bool) -> Result<()> {
    use std::os::unix::fs::MetadataExt;
    let metadata = std::fs::symlink_metadata(path)?;
    // SAFETY: geteuid only observes this process's actual local identity.
    let uid = unsafe { libc::geteuid() };
    if metadata.uid() != uid
        || metadata.mode() & 0o077 != 0
        || (directory && !metadata.is_dir())
        || (!directory && !metadata.is_file())
    {
        return Err("recorder state requires current-owner private directories/files".into());
    }
    Ok(())
}
pub(crate) struct ValidatedBinding(Binding);
impl ValidatedBinding {
    pub(crate) fn producer_host(&self) -> &conduit_core::HostId {
        match &self.0.receipt {
            Receipt::Admission(r) => &r.producer_host,
            Receipt::Current(r) => &r.producer_host,
        }
    }
    pub(crate) fn root(&self) -> &[u8; 32] {
        &self.0.root_key
    }
    #[allow(dead_code)] // Dormant typed custody signer validates the retained invitation.
    pub(crate) fn invitation(&self) -> &conduit_body::SpawnInvitationClaim {
        &self.0.invitation
    }
    pub(crate) fn credential(&self) -> &conduit_body::MembershipCredential {
        match &self.0.receipt {
            Receipt::Admission(r) => &r.credential,
            Receipt::Current(r) => &r.credential,
        }
    }
}
pub(crate) fn validated_binding(path: &Path) -> Result<ValidatedBinding> {
    use std::{io::Read, os::unix::fs::OpenOptionsExt};
    private_metadata(path.parent().ok_or("recorder binding parent")?, true)?;
    private_metadata(path, false)?;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)?;
    let mut bytes = Vec::new();
    file.take(16_385).read_to_end(&mut bytes)?;
    if bytes.len() > 16_384 {
        return Err("joined recorder binding extent".into());
    }
    let binding: Binding = serde_json::from_slice(&bytes)?;
    let credential = match &binding.receipt {
        Receipt::Admission(receipt) => {
            crate::checked(receipt.verify(&binding.root_key))?;
            if binding.schema != "conduit.environment/joined-recorder@1"
                || binding.recovery_request.is_some()
                || binding.invitation.invitation_id != receipt.invitation_id
                || receipt.credential.issued_at_millis > binding.invitation.expires_at_millis
            {
                return Err("joined recorder admission scope".into());
            }
            &receipt.credential
        }
        Receipt::Current(receipt) => {
            let query = binding
                .recovery_request
                .as_ref()
                .ok_or("recorder recovery query missing")?;
            crate::checked(receipt.verify(&binding.root_key, query))?;
            if binding.schema != "conduit.environment/joined-recorder@2"
                || query.body_id != binding.invitation.body_id
                || query.host_id != binding.local_host
                || query.boot_id != binding.local_boot
            {
                return Err("joined recorder recovery scope".into());
            }
            &receipt.credential
        }
    };
    if binding.invitation.body_id != credential.body_id
        || binding.invitation.nonce == [0; 32]
        || binding.local_host != credential.host_id
        || binding.local_boot != credential.boot_id
    {
        return Err("joined recorder invitation/credential scope".into());
    }
    Ok(ValidatedBinding(binding))
}
pub(crate) fn admitted_collector(path: &Path) -> Result<Collector> {
    let binding = validated_binding(path)?;
    let credential = binding.credential();
    use sha2::{Digest, Sha256};
    let mut scope = [0; 96];
    scope[..32].copy_from_slice(&Sha256::digest(credential.body_id.as_str().as_bytes()));
    // The verified durable admission receipt supplies actual same-Body custody;
    // Root-signed epoch discovery supplies each original Boot/Play separately.
    // receipt.producer_boot identifies the admission event, not a permanent
    // producer Boot. This read-only history path neither refreshes collector
    // presence nor grants execution authority after either member restarts.
    Collector::from_authenticated_scope(scope, binding.root())
}
/// Only a private binding saved by successful invitation admission may enter
/// this command. A producer response cannot replace its pinned Root key.
pub(crate) fn run_joined(
    binding: &Path,
    peer: &str,
    directory: &Path,
    once: bool,
    projection: Option<crate::observed_history::retained::Projection<'_>>,
) -> Result<()> {
    let collector = admitted_collector(binding)?;
    crate::observed_history::create_durable_directory(directory)?;
    private_metadata(directory, true)?;
    if projection.is_none() {
        return run(collector, peer, directory, once);
    }
    if peer.is_empty() || peer.len() > 255 || peer.contains(['/', '@', '\n', '\r']) {
        return Err("recorder peer must be HOST:PORT".into());
    }
    run_loop(
        collector,
        vec![peer],
        directory,
        once,
        None,
        None,
        projection.map(|projection| (binding, projection)),
    )
}
#[derive(Default)]
struct Backoff {
    failures: u8,
}
impl Backoff {
    fn failed(&mut self) -> Duration {
        let seconds = [1, 2, 4, 8, 16, 30][usize::from(self.failures.min(5))];
        self.failures = self.failures.saturating_add(1);
        Duration::from_secs(seconds)
    }
    fn succeeded(&mut self) {
        self.failures = 0;
    }
}
fn collect(collector: &Collector, peer: &str, directory: &Path) -> Result<usize> {
    // Resolve afresh after DHCP/link interruptions. Address resolution remains
    // transport selection; every directory and batch verifies the pinned key.
    let addresses = peer.to_socket_addrs()?;
    let mut failure = None;
    for address in addresses.filter(|address| address.is_ipv4()).take(8) {
        match collector.collect_epochs(address, directory) {
            Ok(epochs) => return Ok(epochs),
            Err(error) => failure = Some(error),
        }
    }
    Err(failure.unwrap_or_else(|| "recorder peer has no bounded IPv4 route".into()))
}
/// Run in the foreground for systemd or an ordinary private host service.
/// SIGTERM/default process termination leaves only complete fsynced batch files;
/// a subsequent invocation restores each original Boot's durable cursor.
pub fn run(collector: Collector, peer: &str, directory: &Path, once: bool) -> Result<()> {
    if peer.is_empty() || peer.len() > 255 || peer.contains(['/', '@', '\n', '\r']) {
        return Err("recorder peer must be HOST:PORT".into());
    }
    run_loop(collector, vec![peer], directory, once, None, None, None)
}

#[path = "local_recorder/network.rs"]
pub mod network;
pub(crate) fn run_joined_continuing(
    binding: &Path,
    continuity: &Path,
    peer: &str,
    directory: &Path,
    network: Option<network::Options<'_>>,
    projection: Option<crate::observed_history::retained::Projection<'_>>,
) -> Result<()> {
    let peers = continuing_peers(peer)?;
    let collector = admitted_collector(binding)?;
    crate::observed_history::create_durable_directory(directory)?;
    private_metadata(directory, true)?;
    private_metadata(continuity.parent().ok_or("continuity parent")?, true)?;
    private_metadata(continuity, false)?;
    run_loop(
        collector,
        peers,
        directory,
        false,
        Some((binding, continuity)),
        network
            .map(|options| network::Network::open(binding, continuity, options))
            .transpose()?,
        projection.map(|projection| (binding, projection)),
    )
}
/// Explicit transport alternatives never change the authenticated collector.
/// DNS is resolved only when trying a route; malformed addresses fail up front.
fn continuing_peers(value: &str) -> Result<Vec<&str>> {
    let peers: Vec<_> = value.split(',').take(5).collect();
    if peers.is_empty() || peers.len() > 4 {
        return Err("continuing recorder requires one to four explicit peers".into());
    }
    for peer in &peers {
        validate_peer(peer)?;
    }
    Ok(peers)
}
fn validate_peer(peer: &str) -> Result<()> {
    let (host, port) = peer
        .rsplit_once(':')
        .ok_or("recorder peer must be HOST:PORT")?;
    if peer.len() > 255
        || host.is_empty()
        || !host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
        || port.is_empty()
        || !port.bytes().all(|b| b.is_ascii_digit())
        || port.parse::<u16>().ok().filter(|port| *port != 0).is_none()
    {
        return Err("recorder peer must be an IPv4 address or DNS host and nonzero port".into());
    }
    Ok(())
}
fn collect_routes(
    collector: &Collector,
    peers: &[&str],
    preferred: usize,
    directory: &Path,
) -> Result<(usize, usize)> {
    let mut failure = None;
    for offset in 0..peers.len() {
        let route = (preferred + offset) % peers.len();
        match collect(collector, peers[route], directory) {
            Ok(epochs) => return Ok((route, epochs)),
            Err(error) => failure = Some(error),
        }
    }
    Err(failure.unwrap_or_else(|| "no explicit recorder routes".into()))
}
#[path = "local_recorder/foreground.rs"]
mod foreground;
use foreground::run_loop;
#[cfg(test)]
mod tests {
    use super::*;
    include!("local_recorder/restart.rs");
    include!("local_recorder/recovery.rs");
    #[test]
    fn explicit_routes_are_finite_and_validated_without_resolving() {
        assert_eq!(
            continuing_peers("127.0.0.1:8767,blue.local:80")
                .unwrap()
                .len(),
            2
        );
        for invalid in [
            "",
            "host",
            "host:0",
            "host:65536",
            "host:-1",
            "host:80/path",
            "user@host:80",
            "host:80,",
            "host:80, host:81",
            "host:80,host:81,host:82,host:83,host:84",
        ] {
            assert!(continuing_peers(invalid).is_err(), "{invalid}");
        }
    }
    #[test]
    fn retry_delay_is_bounded_and_only_success_resets_it() {
        let mut backoff = Backoff::default();
        for expected in [1, 2, 4, 8, 16, 30, 30, 30] {
            assert_eq!(backoff.failed(), Duration::from_secs(expected));
        }
        for _ in 0..1024 {
            assert_eq!(backoff.failed(), Duration::from_secs(30));
        }
        backoff.succeeded();
        assert_eq!(backoff.failed(), Duration::from_secs(1));
    }
    // CPU keys/credentials only; this fixture admits no real Host or Body.
    fn receipt() -> (ed25519_dalek::SigningKey, joined::JoinedHostReceipt) {
        use ed25519_dalek::Signer;
        let key = ed25519_dalek::SigningKey::from_bytes(&[41; 32]);
        let credential: conduit_body::MembershipCredential =
            serde_json::from_value(serde_json::json!({
                "credential_id":"01".repeat(32), "body_id":"02".repeat(32),
                "part_id":"03".repeat(32), "host_id":"04".repeat(32),
                "boot_id":"05".repeat(32), "issued_at_millis":123,
            }))
            .unwrap();
        let mut receipt = joined::JoinedHostReceipt {
            schema: joined::PROFILE.into(),
            credential,
            membership_revision: 2,
            producer_host: "06".repeat(32).into(),
            producer_boot: "07".repeat(32).into(),
            producer_key: key.verifying_key().to_bytes(),
            invitation_id: serde_json::from_value(serde_json::json!("08".repeat(32))).unwrap(),
            offer_generation: conduit_core::OfferGeneration(1),
            signature: Vec::new(),
        };
        receipt.signature = key
            .sign(&joined::transcript(&receipt.scope()))
            .to_bytes()
            .to_vec();
        (key, receipt)
    }
    #[test]
    fn joined_receipt_transcript_authenticates_every_admission_field() {
        let (key, receipt) = receipt();
        let root = key.verifying_key().to_bytes();
        assert!(receipt.verify(&root).is_ok());
        let baseline = serde_json::to_value(&receipt).unwrap();
        for pointer in [
            "/credential/credential_id",
            "/credential/body_id",
            "/credential/part_id",
            "/credential/host_id",
            "/credential/boot_id",
            "/producer_host",
            "/producer_boot",
            "/invitation_id",
        ] {
            let mut value = baseline.clone();
            *value.pointer_mut(pointer).unwrap() = serde_json::json!("09".repeat(32));
            let altered: joined::JoinedHostReceipt = serde_json::from_value(value).unwrap();
            assert!(altered.verify(&root).is_err(), "{pointer}");
        }
        for pointer in [
            "/credential/issued_at_millis",
            "/membership_revision",
            "/offer_generation",
        ] {
            let mut value = baseline.clone();
            *value.pointer_mut(pointer).unwrap() = serde_json::json!(333);
            let altered: joined::JoinedHostReceipt = serde_json::from_value(value).unwrap();
            assert!(altered.verify(&root).is_err(), "{pointer}");
        }
        assert!(receipt
            .verify(
                &ed25519_dalek::SigningKey::from_bytes(&[42; 32])
                    .verifying_key()
                    .to_bytes()
            )
            .is_err());
    }
    #[test]
    fn recorder_loads_only_private_exact_invitation_admission_binding() {
        use std::io::Write;
        use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
        let (key, receipt) = receipt();
        let directory =
            std::env::temp_dir().join(format!("conduit-joined-recorder-{}", std::process::id()));
        if directory.exists() {
            std::fs::remove_dir_all(&directory).unwrap();
        }
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .unwrap();
        let path = directory.join("joined.json");
        let mut binding = serde_json::json!({
            "schema":"conduit.environment/joined-recorder@1",
            "invitation":{"invitation_id":receipt.invitation_id,"body_id":receipt.credential.body_id,
                "nonce":vec![3;32], "expires_at_millis":999},
            "root_key":key.verifying_key().to_bytes(),
            "local_host":receipt.credential.host_id,"local_boot":receipt.credential.boot_id,
            "receipt":receipt,
        });
        let write = |value: &serde_json::Value| {
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .mode(0o600)
                .open(&path)
                .unwrap();
            file.write_all(&serde_json::to_vec(value).unwrap()).unwrap();
        };
        write(&binding);
        assert!(admitted_collector(&path).is_ok());
        binding["local_boot"] = serde_json::json!("09".repeat(32));
        write(&binding);
        assert!(admitted_collector(&path).is_err());
        binding["local_boot"] = binding["receipt"]["credential"]["boot_id"].clone();
        binding["invitation"]["expires_at_millis"] = serde_json::json!(1);
        write(&binding);
        assert!(admitted_collector(&path).is_err());
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(admitted_collector(&path).is_err());
        let link = directory.join("link.json");
        std::os::unix::fs::symlink(&path, &link).unwrap();
        assert!(admitted_collector(&link).is_err());
        std::fs::remove_dir_all(directory).unwrap();
    }
}

#[cfg(test)]
#[path = "local_recorder/projection_tests.rs"]
mod projection_tests;
