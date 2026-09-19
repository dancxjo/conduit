use serde_json::json;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use super::{hex, now_millis, SLOT_SCHEMA};

const HOST_ENDPOINT_SCHEMA: &str = "conduit.relay/host-endpoint@1";
const CANDIDATE_SCHEMA: &str = "conduit.relay/endpoint-candidate@1";
const MAXIMUM_IDENTITY_BYTES: usize = 128;

pub(crate) struct ProvisionOptions {
    pub(crate) relay_address: String,
    pub(crate) relay_url: String,
    pub(crate) server_identity: String,
    pub(crate) certificate_sha256: String,
    pub(crate) first_host_id: String,
    pub(crate) first_boot_id: String,
    pub(crate) second_host_id: String,
    pub(crate) second_boot_id: String,
    pub(crate) output: PathBuf,
    pub(crate) expires_in_seconds: u64,
    pub(crate) maximum_attempts: u8,
    pub(crate) authorize_provision: bool,
}

pub(crate) fn provision(options: ProvisionOptions) -> Result<(), String> {
    if !options.authorize_provision {
        return Err("relay provisioning requires --authorize-provision".into());
    }
    let address: SocketAddr = options
        .relay_address
        .parse()
        .map_err(|_| "relay --relay-address must be one exact socket address".to_string())?;
    if address.ip().is_unspecified() || address.port() == 0 {
        return Err("relay endpoint address must be exact and connectable".into());
    }
    if !options.relay_url.starts_with("wss://") || options.relay_url.len() > 256 {
        return Err("relay URL must be one bounded wss URL".into());
    }
    if !bounded(&options.server_identity)
        || !bounded(&options.first_host_id)
        || !bounded(&options.first_boot_id)
        || !bounded(&options.second_host_id)
        || !bounded(&options.second_boot_id)
    {
        return Err("relay endpoint identity violates its finite bound".into());
    }
    let certificate_binding = decode_sha256(&options.certificate_sha256)?;
    let relay_host = options
        .relay_url
        .strip_prefix("wss://")
        .and_then(|rest| rest.split(['/', ':']).next())
        .ok_or_else(|| "relay URL omitted its server identity".to_string())?;
    if relay_host != options.server_identity {
        return Err("relay URL and --server-identity differ".into());
    }
    if options.output.exists() {
        return Err("relay provision output already exists; refusing to overwrite".into());
    }

    let expires_at_millis = now_millis()?
        .checked_add(options.expires_in_seconds.saturating_mul(1_000))
        .ok_or_else(|| "relay candidate expiry overflowed".to_string())?;
    let first_binding = format!("{}/{}", options.first_host_id, options.first_boot_id);
    let second_binding = format!("{}/{}", options.second_host_id, options.second_boot_id);
    if first_binding.len() > MAXIMUM_IDENTITY_BYTES || second_binding.len() > MAXIMUM_IDENTITY_BYTES
    {
        return Err("combined relay endpoint binding exceeds 128 bytes".into());
    }
    let mut entropy = [0_u8; 176];
    getrandom::fill(&mut entropy)
        .map_err(|error| format!("create relay provisioning secrets: {error}"))?;
    if [
        &entropy[48..80],
        &entropy[80..112],
        &entropy[112..144],
        &entropy[144..176],
    ]
    .into_iter()
    .any(|secret| secret.iter().all(|byte| *byte == 0))
    {
        entropy.fill(0);
        return Err("system randomness returned weak relay provisioning material".into());
    }
    let route_id = format!("route/{}", hex(&entropy[..16]));
    let negotiation_id = format!("negotiation/{}", hex(&entropy[16..32]));
    let line_session_id = format!("line/{}", hex(&entropy[32..48]));
    let first_capability = &entropy[48..80];
    let second_capability = &entropy[80..112];
    let protected_session_psk = &entropy[112..144];
    let rendezvous_session_secret = &entropy[144..176];
    let transport_binding = format!("relay/wss/sha256:{}", hex(&certificate_binding));
    let limits = json!({
        "maximum_protected_frame_bytes": 65_553,
        "maximum_attempts": options.maximum_attempts,
        "attempt_timeout_millis": 30_000,
        "maximum_payload_bytes": 65_519,
        "maximum_frames_per_direction": 1_000_000_u64,
        "maximum_bytes_per_direction": 1_073_741_824_u64,
        "handshake_timeout_millis": 30_000,
        "idle_timeout_millis": 300_000
    });
    let mut slot = json!({
        "schema": SLOT_SCHEMA,
        "route_id": route_id,
        "negotiation_id": negotiation_id,
        "first_endpoint_binding": first_binding,
        "second_endpoint_binding": second_binding,
        "expires_at_millis": expires_at_millis,
        "first_capability": first_capability,
        "second_capability": second_capability,
        "limits": {
            "maximum_protected_frame_bytes": 65_553,
            "maximum_queued_frames_per_direction": 2,
            "maximum_queued_bytes_per_direction": 131_106,
            "maximum_attachment_attempts_per_slot": options.maximum_attempts.saturating_mul(2).saturating_add(2),
            "maximum_idle_millis": 300_000,
            "maximum_active_millis": 3_600_000
        }
    });
    let session_binding = json!({
        "initiator": {"host_id": options.first_host_id, "boot_id": options.first_boot_id},
        "responder": {"host_id": options.second_host_id, "boot_id": options.second_boot_id},
        "negotiation_id": negotiation_id,
        "line_session_id": line_session_id,
        "candidate_binding": route_id,
        "transport_binding": transport_binding
    });
    let endpoint = |role: &str, binding: &str, capability: &[u8]| {
        json!({
            "schema": HOST_ENDPOINT_SCHEMA,
            "address": address,
            "candidate": {
                "schema": CANDIDATE_SCHEMA,
                "relay_implementation_id": conduit_protected_line::RELAY_SERVICE_IMPLEMENTATION_ID,
                "relay_locator": options.relay_url,
                "relay_server_identity": options.server_identity,
                "certificate_binding_sha256": certificate_binding,
                "negotiation_id": negotiation_id,
                "route_id": route_id,
                "role": role,
                "endpoint_binding": binding,
                "session_binding": session_binding,
                "expires_at_millis": expires_at_millis,
                "relay_capability": capability,
                "protected_session_psk": protected_session_psk,
                "bounds": limits
            },
            "rendezvous_session_secret": rendezvous_session_secret
        })
    };
    let mut first = endpoint("initiator", &first_binding, first_capability);
    let mut second = endpoint("responder", &second_binding, second_capability);
    let create_result = create_private_directory(&options.output);
    let directory_created = create_result.is_ok();
    let result = create_result.and_then(|()| {
        write_private(&options.output.join("relay-slot.json"), &slot)?;
        write_private(&options.output.join("endpoint-first.json"), &first)?;
        write_private(&options.output.join("endpoint-second.json"), &second)
    });
    entropy.fill(0);
    erase_secret_array(&mut slot["first_capability"]);
    erase_secret_array(&mut slot["second_capability"]);
    erase_secret_array(&mut first["candidate"]["relay_capability"]);
    erase_secret_array(&mut first["candidate"]["protected_session_psk"]);
    erase_secret_array(&mut first["rendezvous_session_secret"]);
    erase_secret_array(&mut second["candidate"]["relay_capability"]);
    erase_secret_array(&mut second["candidate"]["protected_session_psk"]);
    erase_secret_array(&mut second["rendezvous_session_secret"]);
    if let Err(error) = result {
        if directory_created {
            cleanup_incomplete_output(&options.output)
                .map_err(|cleanup| format!("{error}; private output cleanup failed: {cleanup}"))?;
        }
        return Err(error);
    }
    println!(
        "Provisioned relay route={} negotiation={} expires-at-millis={} in {}",
        route_id,
        negotiation_id,
        expires_at_millis,
        options.output.display()
    );
    println!(
        "Keep all three files private; distribute exactly one endpoint file to each intended peer."
    );
    Ok(())
}

fn bounded(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAXIMUM_IDENTITY_BYTES
}

fn decode_sha256(value: &str) -> Result<[u8; 32], String> {
    if value.len() != 64 || !value.is_ascii() {
        return Err("relay certificate SHA-256 must contain exactly 64 hex digits".into());
    }
    let mut output = [0_u8; 32];
    for (index, byte) in output.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)
            .map_err(|_| "relay certificate SHA-256 is not hexadecimal".to_string())?;
    }
    if output == [0; 32] {
        return Err("relay certificate SHA-256 cannot be zero".into());
    }
    Ok(output)
}

fn erase_secret_array(value: &mut serde_json::Value) {
    if let Some(bytes) = value.as_array_mut() {
        bytes.iter_mut().for_each(|byte| *byte = 0.into());
    }
}

fn write_private(path: &Path, value: &serde_json::Value) -> Result<(), String> {
    let mut bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("encode {}: {error}", path.display()))?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let result = options
        .open(path)
        .map_err(|error| format!("create private {}: {error}", path.display()))
        .and_then(|mut file| {
            file.write_all(&bytes)
                .and_then(|()| file.sync_all())
                .map_err(|error| format!("write private {}: {error}", path.display()))
        });
    bytes.fill(0);
    result
}

fn create_private_directory(path: &Path) -> Result<(), String> {
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder
        .create(path)
        .map_err(|error| format!("create private relay directory {}: {error}", path.display()))
}

fn cleanup_incomplete_output(path: &Path) -> Result<(), String> {
    for name in [
        "relay-slot.json",
        "endpoint-first.json",
        "endpoint-second.json",
    ] {
        match fs::remove_file(path.join(name)) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("remove {}: {error}", path.join(name).display())),
        }
    }
    fs::remove_dir(path).map_err(|error| format!("remove {}: {error}", path.display()))
}

#[cfg(test)]
mod tests;
