//! Finite authenticated owner route for one portable Body invitation.

use super::invitation::{
    admit_body_request_document, issue_body_invitation_document, prepare_body_join,
    PortableAdmissionReceipt, PortableInvitation, PortableSpawnAdmissionRequest,
    ROUTED_INVITATION_SCHEMA,
};
use super::membership::complete_body_join_document;
use super::{bounded_read, current_time_millis, MAXIMUM_BODY_ADMISSION_BYTES};
use conduit_body::{RendezvousAuthentication, RendezvousCandidate, RendezvousLineFamily};
use conduit_std_host::secure_websocket::{
    SecureWebSocketClientLine, SecureWebSocketError, SecureWebSocketListener,
};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::{SocketAddr, ToSocketAddrs};
use std::path::Path;
use std::time::Duration;

const ROUTE_REQUEST_SCHEMA: &str = "conduit.body/routed-admission-request@1";
const ROUTE_RESPONSE_SCHEMA: &str = "conduit.body/routed-admission-response@1";
const MAXIMUM_ROUTE_FRAME_BYTES: usize = 512 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RoutedAdmissionRequest {
    schema: String,
    invitation_id: String,
    request: PortableSpawnAdmissionRequest,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "kebab-case", deny_unknown_fields)]
enum RoutedAdmissionResponse {
    Admitted {
        schema: String,
        receipt: Box<PortableAdmissionReceipt>,
    },
    Refused {
        schema: String,
        code: String,
    },
}

pub(crate) fn serve_body_invitation_route(
    state_dir: &Path,
    ttl_seconds: u64,
    bind: SocketAddr,
    public_url: &str,
    certificate: &Path,
    private_key: &Path,
    authorize_route: bool,
) -> Result<(), String> {
    if !authorize_route {
        return Err("exposing a Body admission route requires --authorize-route".into());
    }
    if !public_url.starts_with("wss://") || public_url.len() > 256 {
        return Err("Body admission --route-url must be one bounded wss URL".into());
    }
    let server_identity = route_server_identity(public_url)?;
    let listener = SecureWebSocketListener::bind(
        bind,
        certificate,
        private_key,
        MAXIMUM_ROUTE_FRAME_BYTES as u32,
        authorize_route,
    )
    .map_err(|error| format!("bind Body admission route: {error:?}"))?;
    if listener
        .local_addr()
        .map_err(|error| format!("inspect Body admission route: {error:?}"))?
        != bind
    {
        return Err("Body admission route did not retain its exact authorized bind".into());
    }
    let now_millis = current_time_millis()?;
    let expires_at_millis = now_millis
        .checked_add(ttl_seconds.saturating_mul(1_000))
        .ok_or("Body admission route expiry overflow")?;
    let invitation = issue_body_invitation_document(
        state_dir,
        ttl_seconds,
        Some(vec![RendezvousCandidate {
            candidate_id: "candidate/body-owner-route".into(),
            line_family: RendezvousLineFamily::AuthenticatedTlsStream,
            reachability: public_url.into(),
            authentication: RendezvousAuthentication {
                server_identity,
                transport_binding_sha256: listener.certificate_binding_sha256(),
            },
            expires_at_millis,
            maximum_attempts: 1,
            attempt_timeout_millis: ttl_seconds.min(30).saturating_mul(1_000) as u32,
        }]),
    )?;
    println!(
        "{}",
        serde_json::to_string(&invitation)
            .map_err(|error| format!("encode routed Body invitation: {error}"))?
    );
    std::io::stdout()
        .flush()
        .map_err(|error| format!("publish routed Body invitation: {error}"))?;
    let mut line = listener
        .accept_with_timeout(Duration::from_secs(ttl_seconds))
        .map_err(|error| format!("Body owner unreachable before invitation expiry: {error:?}"))?;
    let request: RoutedAdmissionRequest = receive(&mut line)?;
    let expected_invitation = invitation.claim.invitation_id.as_str();
    if request.schema != ROUTE_REQUEST_SCHEMA || request.invitation_id != expected_invitation {
        send(
            &mut line,
            &RoutedAdmissionResponse::Refused {
                schema: ROUTE_RESPONSE_SCHEMA.into(),
                code: "wrong-invitation".into(),
            },
        )?;
        return Err("routed admission request named another invitation".into());
    }
    match admit_body_request_document(request.request, state_dir, true) {
        Ok(receipt) => send(
            &mut line,
            &RoutedAdmissionResponse::Admitted {
                schema: ROUTE_RESPONSE_SCHEMA.into(),
                receipt: Box::new(receipt),
            },
        ),
        Err(error) => {
            send(
                &mut line,
                &RoutedAdmissionResponse::Refused {
                    schema: ROUTE_RESPONSE_SCHEMA.into(),
                    code: admission_refusal_code(&error).into(),
                },
            )?;
            Err(error)
        }
    }
}

pub(crate) fn join_body_over_route(
    invitation_path: &Path,
    state_dir: &Path,
    authorize_join: bool,
) -> Result<(), String> {
    if !authorize_join {
        return Err("joining a body requires --authorize-join".into());
    }
    let bytes = if invitation_path == Path::new("-") {
        let mut bytes = Vec::new();
        std::io::stdin()
            .take(MAXIMUM_BODY_ADMISSION_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| format!("read routed Body invitation: {error}"))?;
        bytes
    } else {
        bounded_read(invitation_path, MAXIMUM_BODY_ADMISSION_BYTES)?
    };
    if bytes.is_empty() || bytes.len() as u64 > MAXIMUM_BODY_ADMISSION_BYTES {
        return Err("routed Body invitation violates its finite byte bound".into());
    }
    let invitation: PortableInvitation = serde_json::from_slice(&bytes)
        .map_err(|error| format!("routed Body invitation: {error}"))?;
    if invitation.schema != ROUTED_INVITATION_SCHEMA {
        return Err("body join requires a routed spawn-invitation@2; spawn-invitation@1 remains diagnostic-only".into());
    }
    let rendezvous = invitation
        .rendezvous
        .clone()
        .ok_or("routed Body invitation omitted its owner route")?;
    let now_millis = current_time_millis()?;
    rendezvous
        .validate(now_millis)
        .map_err(|error| format!("Body invitation route refused: {error:?}"))?;
    let request = prepare_body_join(invitation, state_dir, authorize_join)?;
    let mut failures = Vec::new();
    for candidate in &rendezvous.candidates {
        if candidate.line_family != RendezvousLineFamily::AuthenticatedTlsStream {
            failures.push(format!("{}:unsupported-line", candidate.candidate_id));
            continue;
        }
        for _ in 0..candidate.maximum_attempts {
            match attempt_candidate(candidate, &request) {
                Ok(receipt) => {
                    complete_body_join_document(receipt.clone(), state_dir, true)?;
                    println!(
                        "{}",
                        serde_json::to_string(&receipt).map_err(|error| format!(
                            "encode retained admission receipt: {error}"
                        ))?
                    );
                    return Ok(());
                }
                Err(error) => failures.push(format!("{}:{error}", candidate.candidate_id)),
            }
        }
    }
    Err(format!(
        "Body join exhausted its admitted owner routes: {}",
        failures.join(",")
    ))
}

fn attempt_candidate(
    candidate: &RendezvousCandidate,
    request: &PortableSpawnAdmissionRequest,
) -> Result<PortableAdmissionReceipt, String> {
    let address = route_address(&candidate.reachability)?;
    let timeout = Duration::from_millis(u64::from(candidate.attempt_timeout_millis));
    let mut line = SecureWebSocketClientLine::connect_pinned(
        address,
        &candidate.reachability,
        &candidate.authentication.server_identity,
        candidate.authentication.transport_binding_sha256,
        timeout,
        MAXIMUM_ROUTE_FRAME_BYTES as u32,
    )
    .map_err(route_connect_refusal)?;
    send(
        &mut line,
        &RoutedAdmissionRequest {
            schema: ROUTE_REQUEST_SCHEMA.into(),
            invitation_id: request.invitation_id.as_str().into(),
            request: request.clone(),
        },
    )?;
    match receive::<RoutedAdmissionResponse>(&mut line)? {
        RoutedAdmissionResponse::Admitted { schema, receipt }
            if schema == ROUTE_RESPONSE_SCHEMA =>
        {
            Ok(*receipt)
        }
        RoutedAdmissionResponse::Refused { schema, code } if schema == ROUTE_RESPONSE_SCHEMA => {
            Err(format!("admission-refused:{code}"))
        }
        _ => Err("owner-response-schema".into()),
    }
}

fn route_connect_refusal(error: SecureWebSocketError) -> String {
    let code = match error {
        SecureWebSocketError::Transport(std::io::ErrorKind::TimedOut)
        | SecureWebSocketError::AcceptDeadline => "route-timeout",
        SecureWebSocketError::Transport(_) => "owner-unreachable",
        SecureWebSocketError::Tls | SecureWebSocketError::Handshake => {
            "endpoint-authentication-failed"
        }
        SecureWebSocketError::InvalidConfiguration => "invalid-route",
        SecureWebSocketError::Disconnected => "owner-disconnected",
        SecureWebSocketError::OversizedMessage | SecureWebSocketError::OutputTooSmall => {
            "route-frame-bound"
        }
        SecureWebSocketError::Protocol | SecureWebSocketError::TextMessageRejected => {
            "route-protocol"
        }
        SecureWebSocketError::Identity(_)
        | SecureWebSocketError::Bind(_)
        | SecureWebSocketError::Accept(_) => "route-mechanism",
    };
    format!("{code}:{error:?}")
}

trait BinaryRoute {
    fn send_binary(&mut self, bytes: &[u8]) -> Result<(), String>;
    fn receive_binary(&mut self, output: &mut [u8]) -> Result<usize, String>;
}

impl BinaryRoute for conduit_std_host::secure_websocket::SecureWebSocketLine {
    fn send_binary(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.send_binary(bytes)
            .map_err(|error| format!("send:{error:?}"))
    }

    fn receive_binary(&mut self, output: &mut [u8]) -> Result<usize, String> {
        self.receive_binary(output)
            .map_err(|error| format!("receive:{error:?}"))
    }
}

impl BinaryRoute for SecureWebSocketClientLine {
    fn send_binary(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.send_binary(bytes)
            .map_err(|error| format!("send:{error:?}"))
    }

    fn receive_binary(&mut self, output: &mut [u8]) -> Result<usize, String> {
        self.receive_binary(output)
            .map_err(|error| format!("receive:{error:?}"))
    }
}

fn send(line: &mut impl BinaryRoute, value: &impl Serialize) -> Result<(), String> {
    let bytes =
        serde_json::to_vec(value).map_err(|error| format!("encode route frame: {error}"))?;
    if bytes.len() > MAXIMUM_ROUTE_FRAME_BYTES {
        return Err("Body admission route frame exceeded its finite bound".into());
    }
    line.send_binary(&bytes)
}

fn receive<T: for<'de> Deserialize<'de>>(line: &mut impl BinaryRoute) -> Result<T, String> {
    let mut bytes = vec![0; MAXIMUM_ROUTE_FRAME_BYTES];
    let length = line.receive_binary(&mut bytes)?;
    serde_json::from_slice(&bytes[..length]).map_err(|error| format!("decode route frame: {error}"))
}

fn route_server_identity(url: &str) -> Result<String, String> {
    let authority = url
        .strip_prefix("wss://")
        .and_then(|remainder| remainder.split('/').next())
        .ok_or("Body admission route is not wss")?;
    let identity = if let Some(bracketed) = authority.strip_prefix('[') {
        bracketed
            .split(']')
            .next()
            .filter(|value| !value.is_empty())
    } else {
        authority
            .split(':')
            .next()
            .filter(|value| !value.is_empty())
    }
    .ok_or("Body admission route omitted its server identity")?;
    Ok(identity.into())
}

fn route_address(url: &str) -> Result<SocketAddr, String> {
    let authority = url
        .strip_prefix("wss://")
        .and_then(|remainder| remainder.split('/').next())
        .ok_or("invalid-wss-route")?;
    authority
        .to_socket_addrs()
        .map_err(|_| "unreachable-route-name".to_string())?
        .take(8)
        .next()
        .ok_or_else(|| "unreachable-route-name".to_string())
}

fn admission_refusal_code(error: &str) -> &'static str {
    if error.contains("expired") {
        "expired"
    } else if error.contains("signature") || error.contains("proof") {
        "tampered-request"
    } else if error.contains("already") || error.contains("replay") {
        "replay"
    } else if error.contains("stale") || error.contains("Boot") {
        "stale-host-boot"
    } else {
        "admission-refused"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reachability_and_endpoint_authentication_refusals_remain_distinct() {
        assert!(route_connect_refusal(SecureWebSocketError::Transport(
            std::io::ErrorKind::ConnectionRefused
        ))
        .starts_with("owner-unreachable:"));
        assert!(route_connect_refusal(SecureWebSocketError::Transport(
            std::io::ErrorKind::TimedOut
        ))
        .starts_with("route-timeout:"));
        assert!(route_connect_refusal(SecureWebSocketError::Tls)
            .starts_with("endpoint-authentication-failed:"));
        assert!(route_connect_refusal(SecureWebSocketError::Handshake)
            .starts_with("endpoint-authentication-failed:"));
    }
}
