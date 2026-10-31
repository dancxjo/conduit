//! Explicit PRIVATE persistence bytes, never general-purpose serialization.
//! Integrity, encryption/access control and commit-last durable custody belong
//! to the storage adapter. These bytes establish no authority or Body identity.
use super::*;
const MAGIC: &[u8; 3] = b"CKN";
pub const KNOWN_NETWORK_PRIVATE_VERSION: u8 = 1;
pub const KNOWN_NETWORK_PRIVATE_HEADER_BYTES: usize = 5;
pub const MAXIMUM_KNOWN_NETWORK_PRIVATE_BYTES: usize = KNOWN_NETWORK_PRIVATE_HEADER_BYTES
    + MAXIMUM_KNOWN_NETWORKS * (4 + KNOWN_SSID_BYTES + KNOWN_CREDENTIAL_BYTES);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KnownNetworkWireRefusal {
    Magic,
    Version,
    Count,
    Truncated,
    Trailing,
    DuplicateSsid,
    Security,
    Capacity,
    Profile(KnownNetworkRefusal),
    Legacy,
}
impl From<KnownNetworkRefusal> for KnownNetworkWireRefusal {
    fn from(value: KnownNetworkRefusal) -> Self {
        Self::Profile(value)
    }
}
/// Encode only into caller-owned private backing. Caller MUST wipe the entire
/// backing on Drop, cancellation and refusal; returned length is not permission
/// to log, format or transmit its contents. Capacity refusal occurs before writes.
/// Credentials are borrowed only within the existing synchronous callbacks.
pub fn encode_private_known_networks(
    profiles: &KnownNetworkProfiles,
    out: &mut [u8],
) -> Result<usize, KnownNetworkWireRefusal> {
    let mut length = KNOWN_NETWORK_PRIVATE_HEADER_BYTES;
    for info in profiles.entries() {
        length += profiles.with_request(info.key, |r| 4 + r.ssid.len() + r.credential.len())?;
    }
    if length > MAXIMUM_KNOWN_NETWORK_PRIVATE_BYTES || out.len() < length {
        return Err(KnownNetworkWireRefusal::Capacity);
    }
    out[..3].copy_from_slice(MAGIC);
    out[3] = KNOWN_NETWORK_PRIVATE_VERSION;
    out[4] = profiles.len() as u8;
    let mut at = KNOWN_NETWORK_PRIVATE_HEADER_BYTES;
    for info in profiles.entries() {
        profiles.with_request(info.key, |request| {
            out[at] = match info.security {
                KnownSecurity::Wpa2Personal => 1,
                KnownSecurity::Wpa3Personal => 2,
            };
            out[at + 1] = info.priority as u8;
            out[at + 2] = request.ssid.len() as u8;
            out[at + 3] = request.credential.len() as u8;
            at += 4;
            out[at..at + request.ssid.len()].copy_from_slice(request.ssid);
            at += request.ssid.len();
            out[at..at + request.credential.len()].copy_from_slice(request.credential);
            at += request.credential.len();
        })?;
    }
    Ok(length)
}
/// Atomic fresh reconstruction: no caller set is mutated, and partial owned
/// secrets volatile-wipe on every refusal. The input remains caller-owned and
/// must be wiped independently. Runtime slots/generations are NOT serialized:
/// decoding reconstructs them in record order for a fresh Boot/explicit private
/// reconfiguration. Do not use decode to reset live policy attempt/round bounds,
/// resurrect old leases, or manufacture grants, membership or Line identities.
pub fn decode_private_known_networks(
    bytes: &[u8],
) -> Result<KnownNetworkProfiles, KnownNetworkWireRefusal> {
    if bytes.len() < KNOWN_NETWORK_PRIVATE_HEADER_BYTES {
        return Err(KnownNetworkWireRefusal::Truncated);
    }
    if &bytes[..3] != MAGIC {
        return Err(KnownNetworkWireRefusal::Magic);
    }
    if bytes[3] != KNOWN_NETWORK_PRIVATE_VERSION {
        return Err(KnownNetworkWireRefusal::Version);
    }
    let count = usize::from(bytes[4]);
    if count > MAXIMUM_KNOWN_NETWORKS {
        return Err(KnownNetworkWireRefusal::Count);
    }
    if bytes.len() > MAXIMUM_KNOWN_NETWORK_PRIVATE_BYTES {
        return Err(KnownNetworkWireRefusal::Capacity);
    }
    let mut profiles = KnownNetworkProfiles::new();
    let mut at = KNOWN_NETWORK_PRIVATE_HEADER_BYTES;
    for _ in 0..count {
        let header = bytes
            .get(at..at + 4)
            .ok_or(KnownNetworkWireRefusal::Truncated)?;
        at += 4;
        let security = match header[0] {
            1 => KnownSecurity::Wpa2Personal,
            2 => KnownSecurity::Wpa3Personal,
            _ => return Err(KnownNetworkWireRefusal::Security),
        };
        let ssid_length = usize::from(header[2]);
        let key_length = usize::from(header[3]);
        if ssid_length == 0 || ssid_length > KNOWN_SSID_BYTES {
            return Err(KnownNetworkWireRefusal::Profile(KnownNetworkRefusal::Ssid));
        }
        if !(8..=KNOWN_CREDENTIAL_BYTES).contains(&key_length) {
            return Err(KnownNetworkWireRefusal::Profile(
                KnownNetworkRefusal::Credential,
            ));
        }
        let ssid = bytes
            .get(at..at + ssid_length)
            .ok_or(KnownNetworkWireRefusal::Truncated)?;
        at += ssid_length;
        let credential = bytes
            .get(at..at + key_length)
            .ok_or(KnownNetworkWireRefusal::Truncated)?;
        at += key_length;
        if profiles.entries().any(|p| {
            profiles
                .with_request(p.key, |r| r.ssid == ssid)
                .unwrap_or(false)
        }) {
            return Err(KnownNetworkWireRefusal::DuplicateSsid);
        }
        profiles.upsert(PrivateNetworkInput::new(
            ssid,
            credential,
            security,
            header[1] as i8,
        )?)?;
    }
    if at != bytes.len() {
        return Err(KnownNetworkWireRefusal::Trailing);
    }
    Ok(profiles)
}
/// Explicit migration only from exact canonical CNJ1 single-network input.
/// Legacy format has no security/priority fields: its original WPA2 adapter
/// contract supplies WPA2 Personal and priority0, never a guessed scan suite.
pub fn migrate_private_legacy_network(
    bytes: &[u8],
) -> Result<KnownNetworkProfiles, KnownNetworkWireRefusal> {
    if bytes.get(..4) != Some(b"CNJ1".as_slice()) {
        return Err(KnownNetworkWireRefusal::Magic);
    }
    let request =
        crate::decode_network_join_request(bytes).map_err(|_| KnownNetworkWireRefusal::Legacy)?;
    let mut profiles = KnownNetworkProfiles::new();
    profiles.upsert(PrivateNetworkInput::new(
        request.ssid,
        request.credential,
        KnownSecurity::Wpa2Personal,
        0,
    )?)?;
    Ok(profiles)
}
/// Bounded persistence dispatch. Unknown magic is refused, never interpreted as
/// old credentials. CNJ1 still undergoes the existing exact canonical decoder.
pub fn decode_private_known_networks_or_legacy(
    bytes: &[u8],
) -> Result<KnownNetworkProfiles, KnownNetworkWireRefusal> {
    if bytes.starts_with(MAGIC) {
        decode_private_known_networks(bytes)
    } else if bytes.starts_with(b"CNJ1") {
        migrate_private_legacy_network(bytes)
    } else {
        Err(KnownNetworkWireRefusal::Magic)
    }
}
