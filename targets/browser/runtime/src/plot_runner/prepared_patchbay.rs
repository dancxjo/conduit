//! One retained Patchbay actor and one unhandled owner request; no second model.
use patchbay_application::{PatchbayApplicationPort, PatchbayApplicationRequest, PatchbayMaskMode};

pub(super) struct PreparedPatchbay {
    pub(super) port: PatchbayApplicationPort,
    pending: Option<PendingRequest>,
}
struct PendingRequest {
    revision: u32,
    request: PatchbayApplicationRequest,
}
impl PreparedPatchbay {
    pub(super) fn new(port: PatchbayApplicationPort) -> Self {
        Self {
            port,
            pending: None,
        }
    }

    pub(super) fn execute(&mut self, input: &[u8]) -> Result<Vec<u8>, String> {
        if self.pending.is_some() && !input.is_empty() {
            return Err("resident Patchbay owner request awaits disposition".into());
        }
        let output = self
            .port
            .apply(input)
            .map_err(|error| format!("resident Patchbay event: {error:?}"))?;
        if let Some(request) = output.request {
            self.pending = Some(PendingRequest {
                revision: self.port.revision(),
                request,
            });
        }
        Ok(output.view)
    }

    pub(super) fn pending_observation(&self) -> Option<serde_json::Value> {
        let pending = self.pending.as_ref()?;
        let request = match &pending.request {
            PatchbayApplicationRequest::EditCurrent {
                expanded_plot_id,
                subject_identity,
            } => serde_json::json!({
                "kind": "EditCurrent", "expanded_plot_id": expanded_plot_id, "subject_identity": subject_identity,
            }),
            PatchbayApplicationRequest::ChangeMasks { body_plan_id, mode } => serde_json::json!({
                "kind": "ChangeMasks", "body_plan_id": body_plan_id,
                "mode": match mode {
                    PatchbayMaskMode::Graphical => "graphical",
                    PatchbayMaskMode::GraphicalAndSpeech => "graphical-and-speech",
                    PatchbayMaskMode::Speech => "speech",
                },
            }),
        };
        Some(serde_json::json!({ "application_revision": pending.revision, "request": request }))
    }
}
