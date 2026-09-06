//! Retained pending Host effects and explicit platform cancellation handshake.
use super::*;
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub(super) enum Command {
    Poll {
        play: ActivePlayId,
    },
    Complete {
        play: ActivePlayId,
        placement_id: String,
        request: u32,
        output: Option<Vec<u8>>,
    },
    Admit {
        play: ActivePlayId,
        sequence: u64,
        bytes: Vec<u8>,
    },
    Accepted {
        play: ActivePlayId,
        sequence: u64,
    },
    Delivered {
        play: ActivePlayId,
        sequence: u64,
    },
    CloseInput {
        play: ActivePlayId,
    },
    Cancel {
        play: ActivePlayId,
    },
    AcknowledgeCancel {
        play: ActivePlayId,
        placement_id: String,
        request: u32,
    },
}

impl Command {
    fn play(&self) -> &ActivePlayId {
        match self {
            Self::Poll { play }
            | Self::Complete { play, .. }
            | Self::Admit { play, .. }
            | Self::Accepted { play, .. }
            | Self::Delivered { play, .. }
            | Self::CloseInput { play }
            | Self::Cancel { play }
            | Self::AcknowledgeCancel { play, .. } => play,
        }
    }
}

pub(super) struct Session {
    pub(super) execution: RemoteExecution,
    pub(super) play: ActivePlayId,
    pending: Option<PendingHostEffect>,
    cancelling: bool,
    pub(super) retired: bool,
}

impl Session {
    pub(super) fn new(execution: RemoteExecution, play: ActivePlayId) -> Self {
        Self {
            execution,
            play,
            pending: None,
            cancelling: false,
            retired: false,
        }
    }

    fn identity(&self, pending: &PendingHostEffect) -> Result<&conduit_core::PlannedGear, String> {
        self.execution
            .fragment
            .placements
            .get(usize::from(pending.request.node.0))
            .ok_or_else(|| "pending placement absent".into())
    }

    fn exact_pending(&self, placement: &str, request: u32) -> Result<&PendingHostEffect, String> {
        let pending = self.pending.as_ref().ok_or("no pending Host effect")?;
        if pending.request.request.0 != request
            || self.identity(pending)?.placement_id.as_str() != placement
        {
            return Err("stale Host effect identity".into());
        }
        Ok(pending)
    }

    pub(super) fn command(&mut self, command: Command) -> Result<Value, String> {
        if command.play() != &self.play || self.retired {
            return Err("stale remote Play".into());
        }
        if self.cancelling
            && !matches!(
                command,
                Command::Poll { .. } | Command::Cancel { .. } | Command::AcknowledgeCancel { .. }
            )
        {
            return Err("remote cancellation awaits platform acknowledgement".into());
        }
        match command {
            Command::Poll { .. } => self.progress(),
            Command::Complete {
                placement_id,
                request,
                output,
                ..
            } => {
                let pending = self.exact_pending(&placement_id, request)?;
                let operation = self
                    .identity(pending)?
                    .host_operations
                    .get(usize::from(pending.request.operation.0))
                    .ok_or("Host operation absent")?;
                if output
                    .as_ref()
                    .is_some_and(|bytes| bytes.len() > operation.maximum_output_bytes as usize)
                {
                    return Err("Host output exceeds admitted bound".into());
                }
                let expects_output = !matches!(
                    pending.effect,
                    engine::BrowserHostEffect::Manifestation(_)
                        | engine::BrowserHostEffect::Timer { .. }
                );
                if output.is_some() != expects_output {
                    return Err("Host completion output shape differs from effect".into());
                }
                // Keep pending ownership on invalid completion.
                self.execution
                    .complete_effect(self.pending.as_ref().unwrap(), output.as_deref())?;
                self.pending = None;
                self.progress()
            }
            Command::Admit {
                sequence, bytes, ..
            } => {
                let outcome = self.execution.admit(sequence, &bytes)?;
                let outcome = match outcome {
                    RemoteIngressOutcome::Accepted { sequence } => {
                        json!({"kind":"accepted", "sequence":sequence})
                    }
                    RemoteIngressOutcome::Full { sequence } => {
                        json!({"kind":"full", "sequence":sequence})
                    }
                };
                Ok(self.envelope(json!({"kind":"ingress", "outcome":outcome})))
            }
            Command::Accepted { sequence, .. } => {
                self.execution.accepted(sequence)?;
                Ok(self.envelope(json!({"kind":"accepted", "sequence":sequence})))
            }
            Command::Delivered { sequence, .. } => {
                self.execution.delivered(sequence)?;
                self.progress()
            }
            Command::CloseInput { .. } => {
                self.execution.close_input()?;
                self.progress()
            }
            Command::Cancel { .. } => {
                self.cancelling = true;
                if self.pending.is_some() {
                    self.progress()
                } else {
                    self.finish_cancel()
                }
            }
            Command::AcknowledgeCancel {
                placement_id,
                request,
                ..
            } => {
                if !self.cancelling {
                    return Err("no pending cancellation".into());
                }
                self.exact_pending(&placement_id, request)?;
                self.finish_cancel()
            }
        }
    }

    fn finish_cancel(&mut self) -> Result<Value, String> {
        self.execution.cancel()?;
        self.pending = None;
        self.retired = true;
        Ok(self.envelope(json!({"kind":"terminal", "disposition":"cancelled"})))
    }

    pub(super) fn progress(&mut self) -> Result<Value, String> {
        if let Some(pending) = self.pending.as_ref() {
            return self.effect(pending);
        }
        match self.execution.drive()? {
            DriveStatus::Effect(pending) => {
                self.pending = Some(pending);
                self.effect(self.pending.as_ref().unwrap())
            }
            DriveStatus::Waiting { pending_effects } => {
                if self.execution.remote.direction == RemoteCordDirection::Egress {
                    if let Some(offer) = self.execution.offer()? {
                        return Ok(self.envelope(json!({"kind":"egress", "sequence":offer.sequence,"bytes":offer.payload})));
                    }
                }
                Ok(self.envelope(json!({"kind":"waiting", "pending_effects":pending_effects})))
            }
            DriveStatus::Complete => {
                if self.execution.remote.direction == RemoteCordDirection::Egress
                    && !self.execution.terminal()?
                {
                    return Err("kernel completed before remote egress terminal".into());
                }
                self.retired = true;
                Ok(self.envelope(
                    json!({"kind":"terminal", "disposition":"kernel-fragment-completed"}),
                ))
            }
        }
    }

    fn envelope(&self, progress: Value) -> Value {
        json!({"schema":"conduit.browser/remote-execution@1", "active_play_id":self.play,
            "plan_id":self.execution.fragment.plan_id, "fragment_id":self.execution.fragment.fragment_id,
            "host_id":self.execution.fragment.host_id, "boot_id":self.execution.fragment.boot_id,
            "connection_id":self.execution.remote.connection_id, "progress":progress})
    }

    fn effect(&self, pending: &PendingHostEffect) -> Result<Value, String> {
        let placement = self.identity(pending)?;
        let operation = placement
            .host_operations
            .get(usize::from(pending.request.operation.0))
            .ok_or("Host operation absent")?;
        let details = if self.cancelling {
            json!({"effect_kind":"cancel"})
        } else {
            match &pending.effect {
                engine::BrowserHostEffect::ButtonTransition => {
                    json!({"effect_kind":"button-transition"})
                }
                engine::BrowserHostEffect::KeyEvent => json!({"effect_kind":"key-event"}),
                engine::BrowserHostEffect::PointerEvent => json!({"effect_kind":"pointer-event"}),
                engine::BrowserHostEffect::ClockObservation => {
                    json!({"effect_kind":"clock-observation"})
                }
                engine::BrowserHostEffect::Timer { duration_millis } => {
                    json!({"effect_kind":"timer", "duration_millis":duration_millis})
                }
                engine::BrowserHostEffect::Snapshot { .. } => {
                    json!({"effect_kind":"snapshot", "support":"requires-resource-specific-adapter"})
                }
                engine::BrowserHostEffect::Manifestation(value) => {
                    json!({"effect_kind":"manifestation", "presentation_kind":value.kind_id,"canonical_value":value.canonical_value})
                }
            }
        };
        Ok(self.envelope(json!({"kind":"host-effect", "placement_id":placement.placement_id,
            "request_sequence":pending.request.request.0, "maximum_output_bytes":operation.maximum_output_bytes,
            "effect":details})))
    }
}
