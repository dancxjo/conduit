//! Maps typed interaction outcomes into the finite renderer-local status channel.

use super::{
    interaction_status::{InteractionStatusCode, InteractionStatusLevel},
    PatchbayApplication,
};
use patchbay_model::{
    InteractionDisposition, PatchbayAction, PatchbayInteractionRequest, PatchbayRefusal,
};

impl PatchbayApplication {
    pub(super) fn finish_interaction(
        &mut self,
        result: Result<patchbay_model::InteractionReceipt, patchbay_model::InteractionError>,
    ) -> Result<(), String> {
        let receipt = match result {
            Ok(receipt) => receipt,
            Err(error) => {
                let message = format!("Interaction execution failed: {error:?}");
                self.interaction_status.publish(
                    InteractionStatusLevel::Failure,
                    InteractionStatusCode::PlatformFailure,
                    &message,
                );
                return Err(message);
            }
        };
        match receipt.disposition {
            InteractionDisposition::Succeeded => {
                if matches!(receipt.request, PatchbayInteractionRequest::Select { .. })
                    && (self.palette_drag.is_some()
                        || self.cord_drag.is_some()
                        || self.cord_route_drag.is_some()
                        || self.gear_drag.is_some())
                {
                    return Ok(());
                }
                let (level, code, message) = match &receipt.request {
                    PatchbayInteractionRequest::Select {
                        subject_identity, ..
                    } => (
                        InteractionStatusLevel::Information,
                        InteractionStatusCode::Selection,
                        self.selection_status(subject_identity),
                    ),
                    PatchbayInteractionRequest::Invoke { invocation, .. }
                        if invocation.action == PatchbayAction::Save =>
                    {
                        (
                            InteractionStatusLevel::Success,
                            InteractionStatusCode::Completed,
                            format!("Saved {}", invocation.target_identity),
                        )
                    }
                    PatchbayInteractionRequest::Invoke { invocation, .. } => (
                        InteractionStatusLevel::Success,
                        InteractionStatusCode::Completed,
                        format!("Completed {}", invocation.action.as_str()),
                    ),
                    PatchbayInteractionRequest::Edit { edit, .. } => (
                        InteractionStatusLevel::Success,
                        InteractionStatusCode::Completed,
                        format!("Completed {}", edit.operation()),
                    ),
                };
                self.interaction_status.publish(level, code, message);
                Ok(())
            }
            InteractionDisposition::Refused(PatchbayRefusal::IncompatiblePorts) => {
                self.publish_refusal(
                    "Cannot connect: incompatible exact Port Info or temporal contracts",
                );
                Ok(())
            }
            InteractionDisposition::Refused(PatchbayRefusal::DuplicateCord) => {
                self.publish_refusal("Cannot connect: those exact Ports already have a Cord");
                Ok(())
            }
            InteractionDisposition::Refused(PatchbayRefusal::InvalidConfiguration) => {
                self.publish_refusal(
                    "Cannot configure: value does not fit this Gear Face type or bounds",
                );
                Ok(())
            }
            InteractionDisposition::Refused(
                PatchbayRefusal::OperationUnavailable | PatchbayRefusal::ActionUnavailable,
            ) => {
                let message = match &receipt.request {
                    PatchbayInteractionRequest::Invoke { invocation, .. }
                        if crate::lifecycle_flow::is_lifecycle_action(invocation.action) =>
                    {
                        self.lifecycle_unavailable_reason(invocation.action)
                            .unwrap_or_else(|| match invocation.action {
                                PatchbayAction::Plan => {
                                    "Plan unavailable for the current exact Form and Host offers"
                                        .into()
                                }
                                PatchbayAction::Play => {
                                    "Play unavailable for the current exact Plan and Host offers"
                                        .into()
                                }
                                PatchbayAction::Stop => {
                                    "Stop request unavailable or already pending".into()
                                }
                                _ => "Lifecycle action is unavailable".into(),
                            })
                    }
                    _ => "Action is unavailable for the current exact state".into(),
                };
                self.publish_refusal(message);
                Ok(())
            }
            InteractionDisposition::Refused(reason) => {
                self.publish_refusal(format!("Interaction refused: {reason:?}"));
                Ok(())
            }
            InteractionDisposition::Failed => {
                self.interaction_status.publish(
                    InteractionStatusLevel::Failure,
                    InteractionStatusCode::PlatformFailure,
                    "Interaction failed in the application or platform adapter",
                );
                Err("interaction failed".into())
            }
        }
    }

    pub(super) fn publish_refusal(&mut self, message: impl Into<String>) {
        self.interaction_status.publish(
            InteractionStatusLevel::Refusal,
            InteractionStatusCode::Refused,
            message,
        );
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn selection_status(&self, subject_identity: &str) -> String {
        let Some(graph) = &self.graphical_form else {
            return format!("Selected {subject_identity}");
        };
        let source_port = graph.inspect(subject_identity).is_ok_and(|inspection| {
            matches!(
                inspection.subject_kind,
                patchbay_model::PatchbaySubjectKind::PortOutput
                    | patchbay_model::PatchbaySubjectKind::FaceInput
            )
        });
        if !source_port {
            return format!("Selected {subject_identity}");
        }
        let candidates = graph.connection_candidates(subject_identity);
        let compatible = candidates
            .iter()
            .filter(|candidate| {
                candidate.compatibility == patchbay_model::PatchbayPortCompatibility::Compatible
            })
            .count();
        format!(
            "Selected {subject_identity}; {compatible} compatible exact Port target(s), {} incompatible or occupied",
            candidates.len().saturating_sub(compatible)
        )
    }
}
