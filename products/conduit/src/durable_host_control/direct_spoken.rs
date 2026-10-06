//! Human-facing authenticated control of the installed owner's direct Mask.
use super::speech_route::{call, SpeechReply, SpeechRequest};
use super::PROTOCOL;
use crate::cli::SpokenMaskCommand;
use serde_json::{json, Value};
use std::path::Path;

pub(crate) fn run(state_dir: &Path, command: SpokenMaskCommand) -> Result<(), String> {
    let value = match command {
        SpokenMaskCommand::Admit { llm } => match call(state_dir, |token| {
            if llm {
                SpeechRequest::LlmAdmit {
                    protocol: PROTOCOL,
                    token,
                }
            } else {
                SpeechRequest::DirectAdmit {
                    protocol: PROTOCOL,
                    token,
                }
            }
        })? {
            SpeechReply::DirectRoute {
                protocol: PROTOCOL,
                report,
            } => *report,
            SpeechReply::Refused { code, .. } => return Err(code),
            _ => return Err("owner returned the wrong direct Mask admission response".into()),
        },
        SpokenMaskCommand::Select { llm } => match call(state_dir, |token| {
            if llm {
                SpeechRequest::LlmSelect {
                    protocol: PROTOCOL,
                    token,
                }
            } else {
                SpeechRequest::DirectSelect {
                    protocol: PROTOCOL,
                    token,
                }
            }
        })? {
            SpeechReply::DirectRoute {
                protocol: PROTOCOL,
                report,
            } => *report,
            SpeechReply::Refused { code, .. } => return Err(code),
            _ => return Err("owner returned the wrong direct Mask selection response".into()),
        },
        SpokenMaskCommand::Start { llm } => match call(state_dir, |token| {
            if llm {
                SpeechRequest::LlmStart {
                    protocol: PROTOCOL,
                    token,
                }
            } else {
                SpeechRequest::DirectStart {
                    protocol: PROTOCOL,
                    token,
                }
            }
        })? {
            SpeechReply::Started {
                protocol: PROTOCOL,
                operation_id,
            } => json!({
                "schema": if llm { "conduit.body/llm-spoken-start@1" } else { "conduit.body/direct-spoken-start@1" },
                "operation_id":operation_id,
                "state":"running",
                "show":null
            }),
            SpeechReply::Refused { code, .. } => return Err(code),
            _ => return Err("owner returned the wrong direct Mask Start response".into()),
        },
        SpokenMaskCommand::Status { operation_id, llm } => {
            match call(state_dir, |token| {
                if llm {
                    SpeechRequest::LlmStatus {
                        protocol: PROTOCOL,
                        token,
                        operation_id,
                    }
                } else {
                    SpeechRequest::DirectStatus {
                        protocol: PROTOCOL,
                        token,
                        operation_id,
                    }
                }
            })? {
                SpeechReply::Status {
                    protocol: PROTOCOL,
                    status,
                } => *status,
                SpeechReply::Refused { code, .. } => return Err(code),
                _ => return Err("owner returned the wrong direct Mask status response".into()),
            }
        }
        SpokenMaskCommand::Stop { operation_id, llm } => {
            match call(state_dir, |token| {
                if llm {
                    SpeechRequest::LlmStop {
                        protocol: PROTOCOL,
                        token,
                        operation_id: operation_id.clone(),
                    }
                } else {
                    SpeechRequest::DirectStop {
                        protocol: PROTOCOL,
                        token,
                        operation_id: operation_id.clone(),
                    }
                }
            })? {
                SpeechReply::StopRequested {
                    protocol: PROTOCOL,
                    operation_id: returned,
                } if returned == operation_id => json!({
                    "schema": if llm { "conduit.body/llm-spoken-stop@1" } else { "conduit.body/direct-spoken-stop@1" },
                    "operation_id":operation_id,
                    "stop_requested":true
                }),
                SpeechReply::Refused { code, .. } => return Err(code),
                _ => return Err("owner returned the wrong direct Mask stop response".into()),
            }
        }
    };
    display(value)
}

fn display(value: Value) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string_pretty(&value)
            .map_err(|error| format!("encode direct Mask result: {error}"))?
    );
    Ok(())
}
