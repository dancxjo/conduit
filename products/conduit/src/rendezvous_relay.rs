//! User-operated two-endpoint opaque relay service.

use conduit_protected_line::{
    OpaqueRelayService, RelayAttachmentDisposition, RelayEndpointRole, RelayServiceError,
    RelayServiceLimits, RelaySlotDescriptor, RelaySlotDisposition, RELAY_SERVICE_IMPLEMENTATION_ID,
};
use conduit_std_host::secure_websocket::{
    SecureWebSocketError, SecureWebSocketLine, SecureWebSocketListener,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const SLOT_SCHEMA: &str = "conduit.relay/private-slot@1";
const ATTACH_SCHEMA: &str = "conduit.relay/attach@1";
const CONTROL_SCHEMA: &str = "conduit.relay/control@1";
const OUTCOME_SCHEMA: &str = "conduit.relay/outcome@1";
const MAXIMUM_SLOT_FILE_BYTES: u64 = 16 * 1024;
const MAXIMUM_CONTROL_BYTES: usize = 4 * 1024;

mod provision;
pub(crate) use provision::{provision, ProvisionOptions};

pub(crate) struct ServeOptions {
    pub(crate) bind: String,
    pub(crate) public_url: String,
    pub(crate) tls_cert: PathBuf,
    pub(crate) tls_key: PathBuf,
    pub(crate) slot: PathBuf,
    pub(crate) accept_timeout_seconds: u64,
    pub(crate) authorize_network: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PrivateSlotFile {
    schema: String,
    route_id: String,
    negotiation_id: String,
    first_endpoint_binding: String,
    second_endpoint_binding: String,
    expires_at_millis: u64,
    first_capability: Vec<u8>,
    second_capability: Vec<u8>,
    limits: PrivateRelayLimits,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PrivateRelayLimits {
    maximum_protected_frame_bytes: u32,
    maximum_queued_frames_per_direction: u16,
    maximum_queued_bytes_per_direction: u32,
    maximum_attachment_attempts_per_slot: u8,
    maximum_idle_millis: u32,
    maximum_active_millis: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
enum AttachmentRole {
    First,
    Second,
}

impl From<AttachmentRole> for RelayEndpointRole {
    fn from(role: AttachmentRole) -> Self {
        match role {
            AttachmentRole::First => Self::First,
            AttachmentRole::Second => Self::Second,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Attachment {
    schema: String,
    route_id: String,
    role: AttachmentRole,
    endpoint_binding: String,
    capability: Vec<u8>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Control {
    schema: String,
    kind: ControlKind,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
enum ControlKind {
    Close,
}

#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
enum OutcomeStatus {
    WaitingForPeer,
    Paired,
    Pressure,
    Closed,
    Lost,
}

#[derive(Serialize)]
struct Outcome<'a> {
    schema: &'static str,
    implementation_id: &'static str,
    route_id: &'a str,
    status: OutcomeStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    code: Option<&'static str>,
}

struct RelayRuntime {
    service: OpaqueRelayService,
    terminal: Option<RelaySlotDisposition>,
}

pub(crate) fn serve(options: ServeOptions) -> Result<(), String> {
    if !options.authorize_network {
        return Err("relay network listening requires --authorize-network".into());
    }
    let bind = options
        .bind
        .parse::<SocketAddr>()
        .map_err(|_| "relay --bind must be one exact socket address".to_string())?;
    if bind.ip().is_loopback() || bind.port() == 0 {
        return Err("relay --bind must be one explicit non-loopback socket".into());
    }
    if !options.public_url.starts_with("wss://") || options.public_url.len() > 256 {
        return Err("relay --public-url must be one bounded wss URL".into());
    }
    let mut slot = read_slot(&options.slot)?;
    let mut first_capability = take_capability(&mut slot.first_capability)?;
    let second_capability = match take_capability(&mut slot.second_capability) {
        Ok(capability) => capability,
        Err(error) => {
            first_capability.fill(0);
            return Err(error);
        }
    };
    let limits = RelayServiceLimits {
        maximum_slots: 1,
        maximum_protected_frame_bytes: slot.limits.maximum_protected_frame_bytes,
        maximum_queued_frames_per_direction: slot.limits.maximum_queued_frames_per_direction,
        maximum_queued_bytes_per_direction: slot.limits.maximum_queued_bytes_per_direction,
        maximum_attachment_attempts_per_slot: slot.limits.maximum_attachment_attempts_per_slot,
        maximum_idle_millis: slot.limits.maximum_idle_millis,
        maximum_active_millis: slot.limits.maximum_active_millis,
    };
    let descriptor = RelaySlotDescriptor::new(
        slot.route_id.clone(),
        slot.negotiation_id,
        slot.first_endpoint_binding,
        slot.second_endpoint_binding,
        slot.expires_at_millis,
        first_capability,
        second_capability,
    )
    .map_err(debug("validate relay slot"))?;
    let now = now_millis()?;
    let mut service = OpaqueRelayService::new(limits).map_err(debug("validate relay limits"))?;
    service
        .install_slot(descriptor, now)
        .map_err(debug("install relay slot"))?;

    let maximum_outer_bytes = usize::try_from(limits.maximum_protected_frame_bytes)
        .ok()
        .and_then(|bytes| bytes.checked_add(256))
        .ok_or_else(|| "relay frame bound overflowed".to_string())?
        .max(MAXIMUM_CONTROL_BYTES);
    let listener = SecureWebSocketListener::bind(
        bind,
        &options.tls_cert,
        &options.tls_key,
        maximum_outer_bytes as u32,
        options.authorize_network,
    )
    .map_err(debug("bind relay WSS listener"))?;
    if listener.local_addr().map_err(debug("read relay bind"))? != bind {
        return Err("relay did not bind the exact authorized socket".into());
    }
    println!(
        "Relay ready: {} route={} implementation={} tls-sha256={}",
        options.public_url,
        slot.route_id,
        RELAY_SERVICE_IMPLEMENTATION_ID,
        hex(&listener.certificate_binding_sha256()),
    );
    println!("Capabilities remain only in the private slot file and endpoint descriptors.");

    let shared = Arc::new(Mutex::new(RelayRuntime {
        service,
        terminal: None,
    }));
    let timeout = Duration::from_secs(options.accept_timeout_seconds);
    std::thread::scope(|scope| -> Result<(), String> {
        let first = listener
            .accept_with_timeout(timeout)
            .map_err(debug("accept first relay endpoint"))?;
        let first_service = Arc::clone(&shared);
        let first_thread = scope.spawn(move || serve_endpoint(first, first_service, 1, limits));
        let second = listener
            .accept_with_timeout(timeout)
            .map_err(debug("accept second relay endpoint"))?;
        let second_service = Arc::clone(&shared);
        let second_thread = scope.spawn(move || serve_endpoint(second, second_service, 2, limits));
        let first_result = first_thread
            .join()
            .map_err(|_| "first relay endpoint worker panicked".to_string())?;
        let second_result = second_thread
            .join()
            .map_err(|_| "second relay endpoint worker panicked".to_string())?;
        first_result.and(second_result)
    })
}

fn serve_endpoint(
    mut line: SecureWebSocketLine,
    runtime: Arc<Mutex<RelayRuntime>>,
    connection_id: u64,
    limits: RelayServiceLimits,
) -> Result<(), String> {
    let maximum_outer_bytes =
        (limits.maximum_protected_frame_bytes as usize + 256).max(MAXIMUM_CONTROL_BYTES);
    let mut buffer = vec![0; maximum_outer_bytes];
    let attachment_length = line
        .receive_binary(&mut buffer[..MAXIMUM_CONTROL_BYTES])
        .map_err(debug("receive relay attachment"))?;
    let decoded = serde_json::from_slice(&buffer[..attachment_length])
        .map_err(|error| format!("decode relay attachment: {error}"));
    buffer[..attachment_length].fill(0);
    let mut attachment: Attachment = decoded?;
    if attachment.schema != ATTACH_SCHEMA {
        attachment.capability.fill(0);
        return Err("relay attachment used the wrong protocol".into());
    }
    let role = RelayEndpointRole::from(attachment.role);
    let disposition = runtime
        .lock()
        .map_err(|_| "relay state lock poisoned".to_string())?
        .service
        .attach(
            &attachment.route_id,
            role,
            &attachment.endpoint_binding,
            core::mem::take(&mut attachment.capability),
            connection_id,
            now_millis()?,
        )
        .map_err(debug("attach relay endpoint"))?;
    send_outcome(
        &mut line,
        &attachment.route_id,
        match disposition {
            RelayAttachmentDisposition::WaitingForPeer => OutcomeStatus::WaitingForPeer,
            RelayAttachmentDisposition::Paired => OutcomeStatus::Paired,
        },
        None,
    )?;
    line.set_read_timeout(Some(Duration::from_millis(25)))
        .map_err(debug("set relay poll timeout"))?;
    let mut paired_reported = disposition == RelayAttachmentDisposition::Paired;
    loop {
        let now = now_millis()?;
        let (status, terminal) = {
            let mut runtime = runtime
                .lock()
                .map_err(|_| "relay state lock poisoned".to_string())?;
            let terminal = runtime.terminal;
            let status =
                runtime
                    .service
                    .attachment_status(&attachment.route_id, role, connection_id, now);
            (status, terminal)
        };
        match status {
            Ok(RelayAttachmentDisposition::Paired) if !paired_reported => {
                send_outcome(&mut line, &attachment.route_id, OutcomeStatus::Paired, None)?;
                paired_reported = true;
            }
            Ok(_) => {}
            Err(RelayServiceError::UnknownRoute) => match terminal {
                Some(RelaySlotDisposition::Closed) => {
                    send_outcome(
                        &mut line,
                        &attachment.route_id,
                        OutcomeStatus::Closed,
                        Some("peer-explicit-close"),
                    )?;
                    line.close().map_err(debug("close relay WSS line"))?;
                    return Ok(());
                }
                Some(RelaySlotDisposition::Lost) => {
                    send_outcome(
                        &mut line,
                        &attachment.route_id,
                        OutcomeStatus::Lost,
                        Some("peer-connection-lost"),
                    )?;
                    line.close().map_err(debug("close relay WSS line"))?;
                    return Ok(());
                }
                _ => return Err("relay slot disappeared without terminal truth".into()),
            },
            Err(error) => return Err(format!("relay attachment ended: {error:?}")),
        }
        while let Some(frame) = {
            runtime
                .lock()
                .map_err(|_| "relay state lock poisoned".to_string())?
                .service
                .receive(&attachment.route_id, role, connection_id, now)
                .map_err(debug("receive queued relay frame"))?
        } {
            line.send_binary(&frame)
                .map_err(debug("send relayed opaque frame"))?;
        }
        match line.receive_binary(&mut buffer) {
            Ok(length) => {
                if !buffer[..length].starts_with(b"CNDR") {
                    let control: Control = serde_json::from_slice(&buffer[..length])
                        .map_err(|error| format!("decode relay control: {error}"))?;
                    if control.schema != CONTROL_SCHEMA {
                        return Err("relay control used the wrong protocol".into());
                    }
                    match control.kind {
                        ControlKind::Close => {
                            let evidence = {
                                let mut runtime = runtime
                                    .lock()
                                    .map_err(|_| "relay state lock poisoned".to_string())?;
                                runtime.terminal = Some(RelaySlotDisposition::Closed);
                                runtime
                                    .service
                                    .close(&attachment.route_id, RelaySlotDisposition::Closed)
                                    .map_err(debug("close relay slot"))?
                            };
                            send_outcome(
                                &mut line,
                                &attachment.route_id,
                                OutcomeStatus::Closed,
                                Some("explicit-close"),
                            )?;
                            eprintln!("Relay closed: {evidence:?}");
                            // The explicit protocol close is already terminal; the
                            // endpoint may have closed its outer WSS immediately.
                            let _outer_close = line.close();
                            return Ok(());
                        }
                    }
                }
                let forwarded = runtime
                    .lock()
                    .map_err(|_| "relay state lock poisoned".to_string())?
                    .service
                    .forward(
                        &attachment.route_id,
                        role,
                        connection_id,
                        &buffer[..length],
                        now,
                    );
                if forwarded == Err(RelayServiceError::QueuePressure) {
                    send_outcome(
                        &mut line,
                        &attachment.route_id,
                        OutcomeStatus::Pressure,
                        Some("queue-pressure"),
                    )?;
                } else {
                    forwarded.map_err(debug("forward opaque relay frame"))?;
                }
            }
            Err(SecureWebSocketError::Transport(
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut,
            )) => {}
            Err(
                SecureWebSocketError::Disconnected
                | SecureWebSocketError::Transport(
                    std::io::ErrorKind::UnexpectedEof
                    | std::io::ErrorKind::ConnectionReset
                    | std::io::ErrorKind::BrokenPipe,
                ),
            ) => {
                let evidence = {
                    let mut runtime = runtime
                        .lock()
                        .map_err(|_| "relay state lock poisoned".to_string())?;
                    if runtime.terminal.is_some() {
                        return Ok(());
                    }
                    runtime.terminal = Some(RelaySlotDisposition::Lost);
                    runtime
                        .service
                        .close(&attachment.route_id, RelaySlotDisposition::Lost)
                };
                if let Ok(evidence) = evidence {
                    eprintln!("Relay lost: {evidence:?}");
                }
                return Err("relay endpoint connection lost".into());
            }
            Err(error) => return Err(format!("receive opaque relay frame: {error:?}")),
        }
    }
}

fn send_outcome(
    line: &mut SecureWebSocketLine,
    route_id: &str,
    status: OutcomeStatus,
    code: Option<&'static str>,
) -> Result<(), String> {
    let bytes = serde_json::to_vec(&Outcome {
        schema: OUTCOME_SCHEMA,
        implementation_id: RELAY_SERVICE_IMPLEMENTATION_ID,
        route_id,
        status,
        code,
    })
    .map_err(|error| format!("encode relay outcome: {error}"))?;
    line.send_binary(&bytes)
        .map_err(debug("send relay outcome"))
}

fn read_slot(path: &PathBuf) -> Result<PrivateSlotFile, String> {
    let metadata =
        fs::metadata(path).map_err(|error| format!("inspect relay slot file: {error}"))?;
    if metadata.len() == 0 || metadata.len() > MAXIMUM_SLOT_FILE_BYTES {
        return Err("relay slot file violates its finite byte bound".into());
    }
    let mut bytes = fs::read(path).map_err(|error| format!("read relay slot file: {error}"))?;
    let decoded =
        serde_json::from_slice(&bytes).map_err(|error| format!("decode relay slot file: {error}"));
    bytes.fill(0);
    let mut slot: PrivateSlotFile = decoded?;
    if slot.schema != SLOT_SCHEMA {
        slot.first_capability.fill(0);
        slot.second_capability.fill(0);
        return Err("relay slot file used the wrong protocol".into());
    }
    Ok(slot)
}

fn take_capability(bytes: &mut [u8]) -> Result<[u8; 32], String> {
    if bytes.len() != 32 {
        bytes.fill(0);
        return Err("relay capability must contain exactly 32 bytes".into());
    }
    let mut capability = [0; 32];
    capability.copy_from_slice(bytes);
    bytes.fill(0);
    Ok(capability)
}

fn now_millis() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "system clock precedes Unix epoch".to_string())?
        .as_millis()
        .try_into()
        .map_err(|_| "system clock exceeds relay representation".to_string())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn debug<T: core::fmt::Debug>(context: &'static str) -> impl FnOnce(T) -> String {
    move |error| format!("{context}: {error:?}")
}

#[cfg(test)]
mod tests;
