//! One selected owner readout, bound to the admitted browser carrier.
use super::{transport, BrowserAdmittedSnapshot, In, Out, PROTOCOL};
use conduit_core::LinkBindingId;
use std::path::Path;

#[derive(Default)]
pub(super) struct CarrierSpeech {
    operation_id: Option<String>,
}

impl CarrierSpeech {
    pub(super) fn handle(
        &mut self,
        frame: In,
        snapshot: &BrowserAdmittedSnapshot,
        socket: &mut transport::Socket,
        binding: &LinkBindingId,
        state_dir: Option<&Path>,
        window_id: Option<&str>,
    ) -> Result<(), String> {
        let credential = &snapshot.credential;
        let (request_id, outcome, operation_id, status, code, close) = match frame {
            In::SelectedSpeechStart {
                request_id,
                request,
                show,
                ..
            } => {
                if request.credential_id != credential.credential_id.as_str()
                    || request.body_id != credential.body_id
                    || request.part_id != credential.part_id
                    || request.host_id != credential.host_id
                    || request.boot_id != credential.boot_id
                {
                    (
                        request_id,
                        "refused",
                        None,
                        None,
                        Some("credential-mismatch".into()),
                        true,
                    )
                } else {
                    let result = state_dir
                        .zip(window_id)
                        .ok_or_else(|| "selected speech needs the installed owner".to_string())
                        .and_then(|(dir, window)| {
                            crate::durable_host_control::browser::selected_speech_start(
                                dir,
                                window,
                                binding.clone(),
                                request,
                                *show,
                            )
                        });
                    match result {
                        Ok(id) => {
                            self.operation_id = Some(id.clone());
                            (request_id, "started", Some(id), None, None, false)
                        }
                        Err(error) => {
                            eprintln!("selected speech start refused: {error}");
                            (
                                request_id,
                                "refused",
                                None,
                                None,
                                Some("selected-speech-unavailable".into()),
                                false,
                            )
                        }
                    }
                }
            }
            In::SelectedSpeechStatus {
                request_id,
                operation_id,
                ..
            } => {
                let result = if self.operation_id.as_deref() == Some(operation_id.as_str()) {
                    state_dir
                        .ok_or_else(|| "selected speech needs installed owner".to_string())
                        .and_then(|dir| {
                            crate::durable_host_control::browser::selected_speech_status(
                                dir,
                                &operation_id,
                            )
                        })
                } else {
                    Err("operation is not on this browser carrier".into())
                };
                match result {
                    Ok(status) => (
                        request_id,
                        "status",
                        Some(operation_id),
                        Some(Box::new(status)),
                        None,
                        false,
                    ),
                    Err(_) => (
                        request_id,
                        "refused",
                        Some(operation_id),
                        None,
                        Some("selected-speech-status-unavailable".into()),
                        false,
                    ),
                }
            }
            In::SelectedSpeechStop {
                request_id,
                operation_id,
                ..
            } => {
                let result = if self.operation_id.as_deref() == Some(operation_id.as_str()) {
                    state_dir
                        .ok_or_else(|| "selected speech needs installed owner".to_string())
                        .and_then(|dir| {
                            crate::durable_host_control::browser::selected_speech_stop(
                                dir,
                                &operation_id,
                            )
                        })
                } else {
                    Err("operation is not on this browser carrier".into())
                };
                match result {
                    Ok(()) => (
                        request_id,
                        "stop-requested",
                        Some(operation_id),
                        None,
                        None,
                        false,
                    ),
                    Err(_) => (
                        request_id,
                        "refused",
                        Some(operation_id),
                        None,
                        Some("selected-speech-stop-unavailable".into()),
                        false,
                    ),
                }
            }
            _ => return Err("not a selected speech browser frame".into()),
        };
        socket.send(&Out::SelectedSpeechResponse {
            protocol: PROTOCOL,
            request_id,
            outcome: outcome.into(),
            operation_id,
            status,
            code,
        })?;
        if close {
            return Err("selected speech request differs from admitted browser carrier".into());
        }
        Ok(())
    }
}
