//! Finite authenticated local control for selected owner speech Plays.
//! The request is EOF-delimited; the worker outlives this short round trip.
use super::{
    constant_time_equal, read_frame, read_secret, write_frame, DurableHostRuntime,
    Request as OrdinaryRequest, PROTOCOL,
};
use conduit_core::{LinkBindingId, PlanId};
use conduit_presentation::{MaskShow, MaskWardrobeAction, OwnerFaceSnapshotRequest};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    io::{Read, Write},
    os::unix::net::UnixStream,
    path::Path,
    time::Duration,
};

pub(crate) const MAGIC: &[u8; 8] = b"SDSPCH01";

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum SpeechRequest {
    OwnerWardrobe {
        protocol: u16,
        token: Vec<u8>,
        owner_plan_id: Option<PlanId>,
        basis_revision: u64,
        action: Option<MaskWardrobeAction>,
    },
    DirectAdmit {
        protocol: u16,
        token: Vec<u8>,
    },
    DirectSelect {
        protocol: u16,
        token: Vec<u8>,
    },
    DirectStart {
        protocol: u16,
        token: Vec<u8>,
    },
    DirectStatus {
        protocol: u16,
        token: Vec<u8>,
        operation_id: String,
    },
    DirectStop {
        protocol: u16,
        token: Vec<u8>,
        operation_id: String,
    },
    LlmAdmit {
        protocol: u16,
        token: Vec<u8>,
    },
    LlmSelect {
        protocol: u16,
        token: Vec<u8>,
    },
    LlmStart {
        protocol: u16,
        token: Vec<u8>,
    },
    LlmStatus {
        protocol: u16,
        token: Vec<u8>,
        operation_id: String,
    },
    LlmStop {
        protocol: u16,
        token: Vec<u8>,
        operation_id: String,
    },
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
    OwnerWardrobe { protocol: u16, report: Box<Value> },
    DirectRoute { protocol: u16, report: Box<Value> },
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

/// One authenticated short control exchange, shared by browser speech and
/// the owner-selected direct Mask. A lost reply remains an unknown outcome.
pub(super) fn call(
    state_dir: &Path,
    request: impl FnOnce(Vec<u8>) -> SpeechRequest,
) -> Result<SpeechReply, String> {
    let mut secret = read_secret(&state_dir.join("control.token"))?;
    let mut stream = UnixStream::connect(state_dir.join("control.sock"))
        .map_err(|error| format!("connect to selected speech owner: {error}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|error| format!("bound selected speech control read: {error}"))?;
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .map_err(|error| format!("bound selected speech control write: {error}"))?;
    stream.write_all(MAGIC).map_err(|error| error.to_string())?;
    let mut request = request(secret.to_vec());
    secret.fill(0);
    let sent = write_frame(&mut stream, &request);
    match &mut request {
        SpeechRequest::OwnerWardrobe { token, .. }
        | SpeechRequest::Start { token, .. }
        | SpeechRequest::Status { token, .. }
        | SpeechRequest::Stop { token, .. }
        | SpeechRequest::DirectAdmit { token, .. }
        | SpeechRequest::DirectSelect { token, .. }
        | SpeechRequest::DirectStart { token, .. }
        | SpeechRequest::DirectStatus { token, .. }
        | SpeechRequest::DirectStop { token, .. } => token.fill(0),
        SpeechRequest::LlmAdmit { token, .. }
        | SpeechRequest::LlmSelect { token, .. }
        | SpeechRequest::LlmStart { token, .. }
        | SpeechRequest::LlmStatus { token, .. }
        | SpeechRequest::LlmStop { token, .. } => token.fill(0),
    }
    sent?;
    stream
        .shutdown(std::net::Shutdown::Write)
        .map_err(|error| error.to_string())?;
    read_frame(&mut stream)
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
            SpeechRequest::OwnerWardrobe {
                protocol, token, ..
            }
            | SpeechRequest::DirectAdmit { protocol, token }
            | SpeechRequest::DirectSelect { protocol, token }
            | SpeechRequest::DirectStart { protocol, token }
            | SpeechRequest::DirectStatus {
                protocol, token, ..
            }
            | SpeechRequest::DirectStop {
                protocol, token, ..
            }
            | SpeechRequest::LlmAdmit { protocol, token }
            | SpeechRequest::LlmSelect { protocol, token }
            | SpeechRequest::LlmStart { protocol, token }
            | SpeechRequest::LlmStatus {
                protocol, token, ..
            }
            | SpeechRequest::LlmStop {
                protocol, token, ..
            }
            | SpeechRequest::Start {
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
            SpeechRequest::OwnerWardrobe {
                owner_plan_id,
                basis_revision,
                action,
                ..
            } => runtime
                .local_owner_wardrobe_report(owner_plan_id.as_ref(), basis_revision, action)
                .map(|report| SpeechReply::OwnerWardrobe {
                    protocol: PROTOCOL,
                    report: Box::new(report),
                }),
            SpeechRequest::DirectAdmit { .. } => {
                runtime
                    .admit_direct_spoken()
                    .map(|report| SpeechReply::DirectRoute {
                        protocol: PROTOCOL,
                        report: Box::new(report),
                    })
            }
            SpeechRequest::DirectSelect { .. } => {
                runtime
                    .select_direct_spoken()
                    .map(|report| SpeechReply::DirectRoute {
                        protocol: PROTOCOL,
                        report: Box::new(report),
                    })
            }
            SpeechRequest::DirectStart { .. } => {
                runtime
                    .start_direct_spoken()
                    .map(|operation_id| SpeechReply::Started {
                        protocol: PROTOCOL,
                        operation_id,
                    })
            }
            SpeechRequest::DirectStatus { operation_id, .. } => runtime
                .direct_spoken_status(&operation_id)
                .map(|status| SpeechReply::Status {
                    protocol: PROTOCOL,
                    status: Box::new(status),
                }),
            SpeechRequest::DirectStop { operation_id, .. } => runtime
                .stop_direct_spoken(&operation_id)
                .map(|()| SpeechReply::StopRequested {
                    protocol: PROTOCOL,
                    operation_id,
                }),
            SpeechRequest::LlmAdmit { .. } => {
                runtime
                    .admit_llm_spoken()
                    .map(|report| SpeechReply::DirectRoute {
                        protocol: PROTOCOL,
                        report: Box::new(report),
                    })
            }
            SpeechRequest::LlmSelect { .. } => {
                runtime
                    .select_llm_spoken()
                    .map(|report| SpeechReply::DirectRoute {
                        protocol: PROTOCOL,
                        report: Box::new(report),
                    })
            }
            SpeechRequest::LlmStart { .. } => {
                runtime
                    .start_llm_spoken()
                    .map(|operation_id| SpeechReply::Started {
                        protocol: PROTOCOL,
                        operation_id,
                    })
            }
            SpeechRequest::LlmStatus { operation_id, .. } => runtime
                .llm_spoken_status(&operation_id)
                .map(|status| SpeechReply::Status {
                    protocol: PROTOCOL,
                    status: Box::new(status),
                }),
            SpeechRequest::LlmStop { operation_id, .. } => runtime
                .stop_llm_spoken(&operation_id)
                .map(|()| SpeechReply::StopRequested {
                    protocol: PROTOCOL,
                    operation_id,
                }),
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
