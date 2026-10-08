//! Finite authenticated local control for a selected browser-source speech Play.
//! The request is EOF-delimited; the worker outlives this short round trip.
use super::{
    body, constant_time_equal, read_frame, write_frame, DurableHostRuntime,
    Request as OrdinaryRequest, PROTOCOL,
};
use conduit_core::LinkBindingId;
use conduit_presentation::{MaskShow, OwnerFaceSnapshotRequest};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    io::{Read, Write},
    os::unix::net::UnixStream,
    path::Path,
    time::Duration,
};

pub(crate) const MAGIC: &[u8; 8] = b"CDSPCH01";

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum SpeechRequest {
    Start {
        protocol: u16,
        token: Vec<u8>,
        window_id: String,
        binding: LinkBindingId,
        request: OwnerFaceSnapshotRequest,
        show: Box<MaskShow>,
        selection: body::SpeechSelection,
    },
    Status {
        protocol: u16,
        token: Vec<u8>,
        operation_id: String,
    },
    Stop {
        protocol: u16,
        token: Vec<u8>,
        operation_id: String,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum SpeechReply {
    Started { protocol: u16, operation_id: String },
    Status { protocol: u16, status: Box<Value> },
    StopRequested { protocol: u16, operation_id: String },
    Refused { protocol: u16, code: String },
}

pub(super) fn ordinary_request_allowed(request: &OrdinaryRequest) -> bool {
    matches!(
        request,
        OrdinaryRequest::Status { .. }
            | OrdinaryRequest::BodyInspect { .. }
            | OrdinaryRequest::BodyFace { .. }
            | OrdinaryRequest::BodyLocalFace { .. }
            | OrdinaryRequest::BodyBrowserLeave { .. }
            | OrdinaryRequest::BodyBrowserCancel { .. }
            | OrdinaryRequest::BodyBrowserAbort { .. }
    )
}

/// A broken or malformed speech client cannot terminate the installed owner.
pub(super) fn serve(
    stream: &mut UnixStream,
    runtime: &mut DurableHostRuntime,
    secret: &[u8; 32],
    first: u8,
) -> Result<(), String> {
    let result = (|| {
        let mut magic = [0; 8];
        magic[0] = first;
        stream
            .read_exact(&mut magic[1..])
            .map_err(|error| error.to_string())?;
        if &magic != MAGIC {
            return Err("selected-speech-protocol".into());
        }
        let mut request: SpeechRequest = read_frame(stream)?;
        let (protocol, token) = match &mut request {
            SpeechRequest::Start {
                protocol, token, ..
            }
            | SpeechRequest::Status {
                protocol, token, ..
            }
            | SpeechRequest::Stop {
                protocol, token, ..
            } => (*protocol, token),
        };
        let authenticated = constant_time_equal(token, secret);
        token.fill(0);
        if !authenticated {
            return Err("unauthorized".into());
        }
        if protocol != PROTOCOL {
            return Err("selected-speech-protocol".into());
        }
        match request {
            SpeechRequest::Start {
                window_id,
                binding,
                request,
                show,
                selection,
                ..
            } => runtime
                .start_browser_speech(window_id, binding, request, *show, selection)
                .map(|operation_id| SpeechReply::Started {
                    protocol: PROTOCOL,
                    operation_id,
                }),
            SpeechRequest::Status { operation_id, .. } => runtime
                .browser_speech_status(&operation_id)
                .map(|status| SpeechReply::Status {
                    protocol: PROTOCOL,
                    status: Box::new(status),
                }),
            SpeechRequest::Stop { operation_id, .. } => runtime
                .stop_browser_speech(&operation_id)
                .map(|()| SpeechReply::StopRequested {
                    protocol: PROTOCOL,
                    operation_id,
                }),
        }
    })();
    let reply = result.unwrap_or_else(|code| SpeechReply::Refused {
        protocol: PROTOCOL,
        code,
    });
    let _ = write_frame(stream, &reply);
    Ok(())
}

fn call(state_dir: &Path, mut request: SpeechRequest) -> Result<SpeechReply, String> {
    let mut bytes = serde_json::to_vec(&request).map_err(|error| error.to_string())?;
    match &mut request {
        SpeechRequest::Start { token, .. }
        | SpeechRequest::Status { token, .. }
        | SpeechRequest::Stop { token, .. } => token.fill(0),
    }
    if bytes.is_empty() || bytes.len() > super::MAXIMUM_CONTROL_FRAME_BYTES {
        bytes.fill(0);
        return Err("selected speech control frame exceeds its finite bound".into());
    }
    let socket = socket2::Socket::new(socket2::Domain::UNIX, socket2::Type::STREAM, None)
        .map_err(|error| error.to_string())?;
    let address = socket2::SockAddr::unix(state_dir.join("control.sock"))
        .map_err(|error| error.to_string())?;
    socket
        .connect_timeout(&address, Duration::from_secs(2))
        .map_err(|_| super::CONTROL_OUTCOME_UNKNOWN.to_owned())?;
    let mut stream = UnixStream::from(std::os::fd::OwnedFd::from(socket));
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .map_err(|error| error.to_string())?;
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|error| error.to_string())?;
    let result = stream
        .write_all(MAGIC)
        .and_then(|()| stream.write_all(&bytes))
        .and_then(|()| stream.shutdown(std::net::Shutdown::Write));
    bytes.fill(0);
    result.map_err(|_| super::CONTROL_OUTCOME_UNKNOWN.to_owned())?;
    read_frame(&mut stream).map_err(|_| super::CONTROL_OUTCOME_UNKNOWN.to_owned())
}

/// These calls are the product seam for a caller holding an acknowledged
/// browser Show. Start acknowledges only a request, never completed audio.
pub(crate) fn start(
    state_dir: &Path,
    window_id: String,
    binding: LinkBindingId,
    request: OwnerFaceSnapshotRequest,
    show: MaskShow,
    selection: body::SpeechSelection,
) -> Result<String, String> {
    match call(
        state_dir,
        SpeechRequest::Start {
            protocol: PROTOCOL,
            token: body::token(state_dir)?,
            window_id,
            binding,
            request,
            show: Box::new(show),
            selection,
        },
    )? {
        SpeechReply::Started {
            protocol: PROTOCOL,
            operation_id,
        } => Ok(operation_id),
        SpeechReply::Refused { code, .. } => Err(code),
        _ => Err("selected speech start returned the wrong response".into()),
    }
}

pub(crate) fn status(state_dir: &Path, operation_id: String) -> Result<Value, String> {
    match call(
        state_dir,
        SpeechRequest::Status {
            protocol: PROTOCOL,
            token: body::token(state_dir)?,
            operation_id,
        },
    )? {
        SpeechReply::Status {
            protocol: PROTOCOL,
            status,
        } => Ok(*status),
        SpeechReply::Refused { code, .. } => Err(code),
        _ => Err("selected speech status returned the wrong response".into()),
    }
}

pub(crate) fn stop(state_dir: &Path, operation_id: String) -> Result<(), String> {
    match call(
        state_dir,
        SpeechRequest::Stop {
            protocol: PROTOCOL,
            token: body::token(state_dir)?,
            operation_id,
        },
    )? {
        SpeechReply::StopRequested {
            protocol: PROTOCOL, ..
        } => Ok(()),
        SpeechReply::Refused { code, .. } => Err(code),
        _ => Err("selected speech stop returned the wrong response".into()),
    }
}
