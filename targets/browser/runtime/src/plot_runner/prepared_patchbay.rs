//! One retained Patchbay actor and one unhandled owner request; no second model.
use patchbay_application::{PatchbayApplicationPort, PatchbayApplicationRequest, PatchbayMaskMode};

pub(super) struct PreparedPatchbay {
    pub(super) port: PatchbayApplicationPort,
    pending: Option<PendingRequest>,
    last_disposition: Option<LastDisposition>,
    last_input_refusal: Option<u32>,
    input_mode: InputMode,
}
enum InputMode {
    Legacy,
    CanonicalMask,
    CanonicalFore,
}
struct PendingRequest {
    revision: u32,
    request: PatchbayApplicationRequest,
}
struct LastDisposition {
    report: super::patchbay_disposition::RequestDisposition,
    request: PatchbayApplicationRequest,
}
impl PreparedPatchbay {
    pub(super) fn new(port: PatchbayApplicationPort) -> Self {
        Self {
            port,
            pending: None,
            last_disposition: None,
            last_input_refusal: None,
            input_mode: InputMode::Legacy,
        }
    }

    /// Source action meaning plus this retained owner's actual delivery pressure.
    pub(super) fn canonical_face_fragment(
        &self,
        basis: conduit_face::FaceContributionBasis,
        body: &conduit_body::BodyId,
    ) -> Result<conduit_face::FaceFragment, patchbay_application::PatchbayFaceProjectionError> {
        let mut fragment = self.port.face_fragment(basis, body, true)?;
        if self.pending.is_some() {
            for action in &mut fragment.actions {
                action.availability = conduit_face::FaceActionAvailability::Unavailable {
                    reason_code: "patchbay-owner-request-pending".into(),
                    explanation: "A prior owner request awaits disposition.".into(),
                };
            }
        }
        Ok(fragment)
    }

    /// Selected during preparation from the exact captured resident Mask selector.
    pub(super) fn require_canonical_input(&mut self) {
        self.input_mode = InputMode::CanonicalMask;
    }

    /// The installed FaceInteraction Fore has no ApplicationView output, even
    /// for an empty poll. Legacy Source fixtures may still initialize their
    /// declared view output when paired with a canonical Mask.
    pub(super) fn select_canonical_fore(&mut self) {
        self.input_mode = InputMode::CanonicalFore;
    }

    pub(super) fn execute(&mut self, input: &[u8]) -> Result<Vec<u8>, String> {
        if matches!(self.input_mode, InputMode::CanonicalFore)
            || (matches!(self.input_mode, InputMode::CanonicalMask) && !input.is_empty())
        {
            return Err("patchbay-canonical-interaction-required".into());
        }
        if let Some(pending) = &self.pending {
            if !input.is_empty() {
                self.last_input_refusal = Some(pending.revision);
            }
            return self.busy_view();
        }
        let output = self
            .port
            .apply(input)
            .map_err(|error| format!("resident Patchbay event: {error:?}"))?;
        if !input.is_empty() {
            self.last_input_refusal = None;
        }
        if let Some(request) = output.request {
            self.pending = Some(PendingRequest {
                revision: self.port.revision(),
                request,
            });
        }
        if self.pending.is_some() {
            return self.busy_view();
        }
        Ok(output.view)
    }

    fn busy_view(&mut self) -> Result<Vec<u8>, String> {
        use conduit_face::{
            ApplicationComponent as Component, ApplicationNodeState, ApplicationView,
        };
        let output = self
            .port
            .apply(&[])
            .map_err(|error| format!("Patchbay pending view: {error:?}"))?;
        let mut view = ApplicationView::decode(&output.view)
            .map_err(|error| format!("Patchbay pending view: {error:?}"))?;
        view.actions.clear();
        for node in &mut view.nodes {
            if node.action.take().is_some()
                && matches!(
                    node.component,
                    Component::Button
                        | Component::TextInput
                        | Component::TextArea
                        | Component::Select
                        | Component::IndependentChoice
                        | Component::ExclusiveChoice
                )
            {
                node.state = ApplicationNodeState::Busy;
            }
        }
        view.encode()
            .map_err(|error| format!("Patchbay busy view: {error:?}"))
    }
    pub(super) fn input_refusal(&self) -> Option<serde_json::Value> {
        self.last_input_refusal.map(|revision| {
            serde_json::json!({
                "reason_code": "patchbay-owner-request-pending", "application_revision": revision,
            })
        })
    }

    pub(super) fn record_disposition(
        &mut self,
        disposition: super::patchbay_disposition::RequestDisposition,
    ) -> Result<(), String> {
        disposition.validate()?;
        let pending = self
            .pending
            .as_ref()
            .ok_or("Patchbay has no pending owner request")?;
        if disposition.application_revision != pending.revision
            || self.port.revision() != pending.revision
        {
            return Err("Patchbay disposition has a stale application revision".into());
        }
        let uncertain = matches!(
            disposition.outcome,
            super::patchbay_disposition::RequestOutcome::Uncertain
        );
        self.last_disposition = Some(LastDisposition {
            report: disposition,
            request: pending.request.clone(),
        });
        if !uncertain {
            self.pending = None;
        }
        Ok(())
    }
    pub(super) fn last_disposition(&self) -> Option<serde_json::Value> {
        self.last_disposition.as_ref().map(|last| {
            let mut value =
                serde_json::to_value(&last.report).expect("bounded disposition serializes");
            value["request"] = request_observation(&last.request);
            value
        })
    }

    pub(super) fn pending_observation(&self) -> Option<serde_json::Value> {
        let pending = self.pending.as_ref()?;
        let request = request_observation(&pending.request);
        Some(serde_json::json!({ "application_revision": pending.revision, "request": request }))
    }
}

fn request_observation(request: &PatchbayApplicationRequest) -> serde_json::Value {
    match request {
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
    }
}

// Canonical and legacy delivery share this actor and this one pending slot.
impl crate::patchbay_ingress::PatchbayIngressOwner for PreparedPatchbay {
    fn port(&mut self) -> Result<&mut PatchbayApplicationPort, String> {
        if let Some(pending) = &self.pending {
            self.last_input_refusal = Some(pending.revision);
            return Err("Patchbay owner request is pending".into());
        }
        Ok(&mut self.port)
    }

    fn record_dispatch(&mut self, dispatch: &mut crate::patchbay_ingress::PatchbayIngressDispatch) {
        if let Ok(request) = &mut dispatch.owner_result {
            if let Some(request) = request.take() {
                debug_assert!(
                    self.pending.is_none(),
                    "delivery storage admitted before dispatch"
                );
                let revision = self.port.revision();
                self.pending = Some(PendingRequest { revision, request });
                dispatch.request_delivery =
                    crate::patchbay_ingress::PatchbayRequestDelivery::Retained {
                        application_revision: revision,
                    };
            }
        }
        self.last_input_refusal = None;
    }
}
