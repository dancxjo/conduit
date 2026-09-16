//! Authenticated local control plane into the durable installed Host owner.

use conduit_body::{SpawnInvitationClaim, SpawnInvitationSecret};
use conduit_core::HostAdvertisement;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

const PROTOCOL: u16 = 1;
const MAXIMUM_CONTROL_FRAME_BYTES: usize = 128 * 1024;

#[derive(Debug, Clone)]
pub(crate) struct DurableHostTruth {
    pub(crate) target_id: String,
    pub(crate) image_content_digest: String,
    pub(crate) advertisement: HostAdvertisement,
}

#[derive(Debug, Clone)]
pub(crate) struct DurableJoinProof {
    pub(crate) advertisement: HostAdvertisement,
    pub(crate) invitation_id: String,
    pub(crate) body_id: String,
    pub(crate) nonce: [u8; 32],
    pub(crate) signature: Vec<u8>,
    pub(crate) observed_at_millis: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum Request {
    Status {
        protocol: u16,
        token: Vec<u8>,
    },
    Join {
        protocol: u16,
        token: Vec<u8>,
        expected_boot_id: String,
        expected_offer_generation: u64,
        claim: SpawnInvitationClaim,
        secret: Vec<u8>,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum Response {
    Status {
        protocol: u16,
        target_id: String,
        image_content_digest: String,
        advertisement: HostAdvertisement,
    },
    Join {
        protocol: u16,
        advertisement: HostAdvertisement,
        invitation_id: String,
        body_id: String,
        nonce: [u8; 32],
        signature: Vec<u8>,
        observed_at_millis: u64,
    },
    Refused {
        protocol: u16,
        code: String,
    },
}

pub(crate) fn ensure_secret(state_dir: &Path) -> Result<(), String> {
    let path = state_dir.join("control.token");
    if path.exists() {
        return read_secret(&path).map(|_| ());
    }
    let mut token = [0_u8; 32];
    getrandom::fill(&mut token).map_err(|error| format!("create local control token: {error}"))?;
    if token == [0; 32] {
        return Err("system randomness returned a weak local control token".into());
    }
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
        .open(&path)
        .and_then(|mut file| file.write_all(&token))
        .map_err(|error| format!("retain local control token: {error}"))?;
    token.fill(0);
    Ok(())
}

#[cfg(unix)]
pub(crate) fn serve(state_dir: &Path, truth: DurableHostTruth) -> Result<(), String> {
    use std::os::unix::{fs::PermissionsExt, net::UnixListener};

    let socket = state_dir.join("control.sock");
    if socket.exists() {
        fs::remove_file(&socket)
            .map_err(|error| format!("remove stale control endpoint: {error}"))?;
    }
    let listener = UnixListener::bind(&socket)
        .map_err(|error| format!("bind durable Host control endpoint: {error}"))?;
    fs::set_permissions(&socket, fs::Permissions::from_mode(0o600))
        .map_err(|error| format!("restrict durable Host control endpoint: {error}"))?;
    let mut token = read_secret(&state_dir.join("control.token"))?;
    for incoming in listener.incoming() {
        let mut stream = incoming.map_err(|error| format!("accept local Host control: {error}"))?;
        let response = handle(read_frame(&mut stream)?, &token, &truth);
        write_frame(&mut stream, &response)?;
    }
    token.fill(0);
    Ok(())
}

#[cfg(not(unix))]
pub(crate) fn serve(_state_dir: &Path, _truth: DurableHostTruth) -> Result<(), String> {
    Err("no reviewed local durable Host control carrier exists on this platform".into())
}

#[cfg(unix)]
pub(crate) fn current(state_dir: &Path) -> Result<DurableHostTruth, String> {
    use std::os::unix::net::UnixStream;
    let mut token = read_secret(&state_dir.join("control.token"))?;
    let mut stream = UnixStream::connect(state_dir.join("control.sock"))
        .map_err(|error| format!("connect to durable Host service: {error}"))?;
    write_frame(
        &mut stream,
        &Request::Status {
            protocol: PROTOCOL,
            token: token.to_vec(),
        },
    )?;
    stream
        .shutdown(std::net::Shutdown::Write)
        .map_err(|error| format!("finish durable Host status request: {error}"))?;
    token.fill(0);
    match read_frame::<_, Response>(&mut stream)? {
        Response::Status {
            protocol: PROTOCOL,
            target_id,
            image_content_digest,
            advertisement,
        } => Ok(DurableHostTruth {
            target_id,
            image_content_digest,
            advertisement,
        }),
        Response::Refused { code, .. } => Err(format!("durable Host refused status: {code}")),
        _ => Err("durable Host returned the wrong control response".into()),
    }
}

#[cfg(not(unix))]
pub(crate) fn current(_state_dir: &Path) -> Result<DurableHostTruth, String> {
    Err("no reviewed local durable Host control carrier exists on this platform".into())
}

#[cfg(unix)]
pub(crate) fn join(
    state_dir: &Path,
    expected: &HostAdvertisement,
    claim: SpawnInvitationClaim,
    secret: Vec<u8>,
) -> Result<DurableJoinProof, String> {
    use std::os::unix::net::UnixStream;
    let mut token = read_secret(&state_dir.join("control.token"))?;
    let mut stream = UnixStream::connect(state_dir.join("control.sock"))
        .map_err(|error| format!("connect to durable Host service: {error}"))?;
    let mut request = Request::Join {
        protocol: PROTOCOL,
        token: token.to_vec(),
        expected_boot_id: expected.boot_id.as_str().into(),
        expected_offer_generation: expected.offer_generation.0,
        claim,
        secret,
    };
    write_sensitive_frame(&mut stream, &request)?;
    stream
        .shutdown(std::net::Shutdown::Write)
        .map_err(|error| format!("finish durable Host join request: {error}"))?;
    token.fill(0);
    if let Request::Join { secret, token, .. } = &mut request {
        secret.fill(0);
        token.fill(0);
    }
    match read_frame::<_, Response>(&mut stream)? {
        Response::Join {
            protocol: PROTOCOL,
            advertisement,
            invitation_id,
            body_id,
            nonce,
            signature,
            observed_at_millis,
        } if advertisement == *expected => Ok(DurableJoinProof {
            advertisement,
            invitation_id,
            body_id,
            nonce,
            signature,
            observed_at_millis,
        }),
        Response::Refused { code, .. } => Err(format!("durable Host refused join: {code}")),
        _ => Err("durable Host changed identity while completing rendezvous".into()),
    }
}

#[cfg(not(unix))]
pub(crate) fn join(
    _state_dir: &Path,
    _expected: &HostAdvertisement,
    _claim: SpawnInvitationClaim,
    mut secret: Vec<u8>,
) -> Result<DurableJoinProof, String> {
    secret.fill(0);
    Err("no reviewed local durable Host control carrier exists on this platform".into())
}

fn handle(mut request: Request, token: &[u8; 32], truth: &DurableHostTruth) -> Response {
    let offered = match &mut request {
        Request::Status { token, .. } | Request::Join { token, .. } => token,
    };
    let authenticated = constant_time_equal(offered, token);
    offered.fill(0);
    if !authenticated {
        return refused("unauthorized");
    }
    match request {
        Request::Status { protocol, .. } if protocol == PROTOCOL => Response::Status {
            protocol: PROTOCOL,
            target_id: truth.target_id.clone(),
            image_content_digest: truth.image_content_digest.clone(),
            advertisement: truth.advertisement.clone(),
        },
        Request::Join {
            protocol,
            expected_boot_id,
            expected_offer_generation,
            claim,
            mut secret,
            ..
        } if protocol == PROTOCOL => {
            let result = create_join(
                truth,
                &expected_boot_id,
                expected_offer_generation,
                &claim,
                &secret,
            );
            secret.fill(0);
            result.unwrap_or_else(|code| refused(&code))
        }
        _ => refused("protocol"),
    }
}

fn create_join(
    truth: &DurableHostTruth,
    expected_boot_id: &str,
    expected_offer_generation: u64,
    claim: &SpawnInvitationClaim,
    secret: &[u8],
) -> Result<Response, String> {
    if truth.advertisement.boot_id.as_str() != expected_boot_id
        || truth.advertisement.offer_generation.0 != expected_offer_generation
    {
        return Err("stale-host-truth".into());
    }
    let now = now_millis().map_err(|_| "clock".to_string())?;
    claim.inspect(now).map_err(|_| "invitation".to_string())?;
    let secret: [u8; 32] = secret
        .try_into()
        .map_err(|_| "invitation-secret".to_string())?;
    let secret = SpawnInvitationSecret::from_csprng_bytes(secret)
        .map_err(|_| "invitation-secret".to_string())?;
    let transcript = claim.signing_transcript(
        &truth.advertisement.host_id,
        &truth.advertisement.boot_id,
        truth.advertisement.offer_generation,
    );
    Ok(Response::Join {
        protocol: PROTOCOL,
        advertisement: truth.advertisement.clone(),
        invitation_id: claim.invitation_id.as_str().into(),
        body_id: claim.body_id.as_str().into(),
        nonce: claim.nonce,
        signature: secret.sign(&transcript).to_vec(),
        observed_at_millis: now,
    })
}

fn refused(code: &str) -> Response {
    Response::Refused {
        protocol: PROTOCOL,
        code: code.into(),
    }
}

fn read_secret(path: &Path) -> Result<[u8; 32], String> {
    let bytes = fs::read(path).map_err(|error| format!("read local control token: {error}"))?;
    bytes
        .try_into()
        .map_err(|_| "local control token has the wrong finite bound".into())
}

fn read_frame<R: Read, T: for<'de> Deserialize<'de>>(reader: &mut R) -> Result<T, String> {
    let mut bytes = Vec::with_capacity(4096);
    reader
        .take((MAXIMUM_CONTROL_FRAME_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("read local control frame: {error}"))?;
    if bytes.is_empty() || bytes.len() > MAXIMUM_CONTROL_FRAME_BYTES {
        bytes.fill(0);
        return Err("local control frame violates its finite bound".into());
    }
    let value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("decode local control frame: {error}"));
    bytes.fill(0);
    value
}

fn write_frame<W: Write, T: Serialize>(writer: &mut W, value: &T) -> Result<(), String> {
    let bytes = serde_json::to_vec(value).map_err(|error| error.to_string())?;
    if bytes.len() > MAXIMUM_CONTROL_FRAME_BYTES {
        return Err("local control response violates its finite bound".into());
    }
    writer
        .write_all(&bytes)
        .map_err(|error| format!("write local control frame: {error}"))
}

fn write_sensitive_frame<W: Write, T: Serialize>(writer: &mut W, value: &T) -> Result<(), String> {
    let mut bytes = serde_json::to_vec(value).map_err(|error| error.to_string())?;
    if bytes.len() > MAXIMUM_CONTROL_FRAME_BYTES {
        bytes.fill(0);
        return Err("local control response violates its finite bound".into());
    }
    let result = writer
        .write_all(&bytes)
        .map_err(|error| format!("write local control frame: {error}"));
    bytes.fill(0);
    result
}

fn constant_time_equal(offered: &[u8], expected: &[u8; 32]) -> bool {
    let mut difference = offered.len() ^ expected.len();
    for (index, expected_byte) in expected.iter().copied().enumerate() {
        difference |= usize::from(offered.get(index).copied().unwrap_or(0) ^ expected_byte);
    }
    difference == 0
}

fn now_millis() -> Result<u64, String> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();
    u64::try_from(millis).map_err(|_| "system clock exceeds control representation".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{BootId, HostId, OfferGeneration};
    use conduit_std_host::{StdHost, StdHostConfig};

    fn truth() -> DurableHostTruth {
        let host = StdHost::new_with_config(StdHostConfig {
            host_id: HostId::from("host/durable-fixture"),
            boot_id: BootId::from("boot/durable-fixture"),
            offer_generation: OfferGeneration(7),
        });
        DurableHostTruth {
            target_id: "std/x86_64/computer".into(),
            image_content_digest: format!("sha256:{}", "a".repeat(64)),
            advertisement: host.advertisement().clone(),
        }
    }

    fn claim() -> SpawnInvitationClaim {
        serde_json::from_value(serde_json::json!({
            "invitation_id":"invitation/durable",
            "body_id":"body/durable",
            "nonce":vec![17;32],
            "expires_at_millis":4_000_000_000_000_u64
        }))
        .unwrap()
    }

    #[test]
    fn durable_owner_signs_only_its_exact_current_boot_and_generation() {
        let truth = truth();
        let token = [23_u8; 32];
        let response = handle(
            Request::Join {
                protocol: PROTOCOL,
                token: token.to_vec(),
                expected_boot_id: truth.advertisement.boot_id.as_str().into(),
                expected_offer_generation: 7,
                claim: claim(),
                secret: vec![29; 32],
            },
            &token,
            &truth,
        );
        let Response::Join {
            advertisement,
            signature,
            ..
        } = response
        else {
            panic!("current durable Host did not issue join proof")
        };
        assert_eq!(advertisement, truth.advertisement);
        assert_eq!(signature.len(), 64);

        let stale = handle(
            Request::Join {
                protocol: PROTOCOL,
                token: token.to_vec(),
                expected_boot_id: "boot/stale".into(),
                expected_offer_generation: 7,
                claim: claim(),
                secret: vec![29; 32],
            },
            &token,
            &truth,
        );
        assert!(matches!(stale, Response::Refused { ref code, .. } if code == "stale-host-truth"));
    }

    #[test]
    fn unauthorized_helper_cannot_obtain_or_substitute_advertisement() {
        let truth = truth();
        let response = handle(
            Request::Status {
                protocol: PROTOCOL,
                token: vec![0; 32],
            },
            &[23; 32],
            &truth,
        );
        assert!(matches!(response, Response::Refused { ref code, .. } if code == "unauthorized"));
    }
}
