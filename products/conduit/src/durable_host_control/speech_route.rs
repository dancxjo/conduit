//! Finite authenticated local control for a selected browser-source speech Play.
//! The request is EOF-delimited; the worker outlives this short round trip.
use super::{
    constant_time_equal, read_frame, write_frame, DurableHostRuntime, Request as OrdinaryRequest,
    PROTOCOL,
};
use conduit_core::LinkBindingId;
use conduit_presentation::{MaskShow, OwnerFaceSnapshotRequest};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{io::Read, os::unix::net::UnixStream};

pub(crate) const MAGIC: &[u8; 8] = b"SDSPCH01";

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum SpeechRequest {
    Start {
        protocol: u16,
        token: Vec<u8>,
        window_id: String,
        binding: LinkBindingId,
        request: Box<OwnerFaceSnapshotRequest>,
        show: Box<MaskShow>,
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
pub(crate) enum SpeechReply {
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
                ..
            } => runtime
                .start_browser_speech(window_id, binding, *request, *show)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::Shutdown,
    };

    fn roundtrip(secret: &[u8; 32], offered: Vec<u8>) -> SpeechReply {
        let mut runtime = DurableHostRuntime::new(
            "test".into(),
            "test".into(),
            conduit_std_host::StdHost::new(),
        );
        let (mut client, mut server) = UnixStream::pair().unwrap();
        client.write_all(MAGIC).unwrap();
        write_frame(
            &mut client,
            &SpeechRequest::Status {
                protocol: PROTOCOL,
                token: offered,
                operation_id: "operation/unknown".into(),
            },
        )
        .unwrap();
        client.shutdown(Shutdown::Write).unwrap();
        let mut first = [0];
        server.read_exact(&mut first).unwrap();
        serve(&mut server, &mut runtime, secret, first[0]).unwrap();
        drop(server);
        read_frame(&mut client).unwrap()
    }

    #[test]
    fn authenticated_speech_status_distinguishes_unknown_operation_from_bad_token() {
        assert_ne!(MAGIC[0], super::super::terminal_attach::MAGIC[0]);
        let secret = [7; 32];
        assert!(
            matches!(roundtrip(&secret, vec![7;32]), SpeechReply::Refused { code, .. }
            if code == "unknown selected speech operation")
        );
        assert!(
            matches!(roundtrip(&secret, vec![8;32]), SpeechReply::Refused { code, .. }
            if code == "unauthorized")
        );
    }
}
