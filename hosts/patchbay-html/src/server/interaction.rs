//! Typed interaction decoding and execution for the HTML delivery adapter.

use super::{PatchbayHtmlServer, ServerError};
use patchbay_model::{
    InteractionDisposition, PatchbayAction, PatchbayEdit, PatchbayEditBasis,
    PatchbayInteractionRequest, PatchbayInvocationOutcome, PatchbayRefusal, PatchbaySubjectRef,
};
use serde::Deserialize;

impl PatchbayHtmlServer {
    pub(super) fn apply_interaction(&mut self, bytes: &[u8]) -> Result<Vec<u8>, ServerError> {
        let input: HtmlInteractionInput =
            serde_json::from_slice(bytes).map_err(|_| ServerError::InvalidRequest)?;
        let stale_presentation =
            input.presentation_id != self.snapshot.presentation.identity.as_str();
        if input.kind == "clear" {
            self.snapshot.interaction.revision =
                self.snapshot.interaction.revision.saturating_add(1);
            if stale_presentation {
                self.snapshot.interaction.last_disposition =
                    Some("Refused(StalePresentation)".into());
            } else {
                match self
                    .snapshot
                    .entrance
                    .clear_selection(&self.snapshot.presentation)
                {
                    Ok(()) => {
                        self.snapshot.interaction.selected_subject = None;
                        self.snapshot.interaction.last_disposition = Some("Succeeded".into());
                    }
                    Err(error) => {
                        self.snapshot.interaction.last_disposition =
                            Some(format!("Refused({error:?})"));
                    }
                }
            }
            self.encoded_snapshot = self.snapshot.encode()?;
            return Ok(self.encoded_snapshot.clone());
        }
        if self.snapshot.presentation.basis.expanded_form_id.is_none() && input.kind == "select" {
            self.snapshot.interaction.revision =
                self.snapshot.interaction.revision.saturating_add(1);
            if stale_presentation {
                self.snapshot.interaction.last_disposition =
                    Some("Refused(StalePresentation)".into());
            } else {
                let subject = input.subject.ok_or(ServerError::InvalidRequest)?;
                match self
                    .snapshot
                    .entrance
                    .select(&self.snapshot.presentation, &subject)
                {
                    Ok(()) => {
                        self.snapshot.interaction.selected_subject = Some(subject);
                        self.snapshot.interaction.last_disposition = Some("Succeeded".into());
                    }
                    Err(error) => {
                        self.snapshot.interaction.last_disposition =
                            Some(format!("Refused({error:?})"));
                    }
                }
            }
            self.encoded_snapshot = self.snapshot.encode()?;
            return Ok(self.encoded_snapshot.clone());
        }
        let request_id = self
            .interaction
            .next_request_id(&input.kind)
            .map_err(|error| ServerError::Interaction(format!("{error:?}")))?;
        let request = match input.kind.as_str() {
            "select" => PatchbayInteractionRequest::select(
                request_id,
                &PatchbaySubjectRef {
                    expanded_form_id: if stale_presentation {
                        conduit_core::ExpandedFormId::from(input.presentation_id.clone())
                    } else {
                        self.snapshot
                            .presentation
                            .basis
                            .expanded_form_id
                            .clone()
                            .ok_or(ServerError::InvalidRequest)?
                    },
                    subject_identity: input.subject.ok_or(ServerError::InvalidRequest)?,
                },
            ),
            "invoke" => PatchbayInteractionRequest::invoke(
                request_id,
                parse_html_action(input.action.as_deref().ok_or(ServerError::InvalidRequest)?)?,
                input.target.ok_or(ServerError::InvalidRequest)?,
            ),
            "edit" => PatchbayInteractionRequest::edit(
                request_id,
                parse_html_edit(input.edit.ok_or(ServerError::InvalidRequest)?)?,
            ),
            _ => return Err(ServerError::InvalidRequest),
        }
        .map_err(|error| ServerError::Interaction(format!("{error:?}")))?;
        let expected_target = self
            .snapshot
            .presentation
            .basis
            .expanded_form_id
            .as_ref()
            .ok_or(ServerError::InvalidRequest)?
            .as_str()
            .to_owned();
        let requested_action = match &request {
            PatchbayInteractionRequest::Invoke { invocation, .. } => Some(invocation.action),
            _ => None,
        };
        let mut prepared_front_door = None;
        let prepared_outcome = match requested_action {
            Some(PatchbayAction::Plan | PatchbayAction::Play)
                if !stale_presentation
                    && matches!(
                        &request,
                        PatchbayInteractionRequest::Invoke { invocation, .. }
                            if invocation.target_identity == expected_target
                    ) =>
            {
                self.front_door.as_ref().map_or(
                    PatchbayInvocationOutcome::Refused(PatchbayRefusal::OperationUnavailable),
                    |session| {
                        let Ok(session) = session.lock() else {
                            return PatchbayInvocationOutcome::Failed;
                        };
                        let mut candidate = session.clone();
                        let result = if requested_action == Some(PatchbayAction::Plan) {
                            candidate.plan_form().map(|_| ())
                        } else {
                            candidate.play_plan().map(|_| ())
                        };
                        match result {
                            Ok(()) => {
                                prepared_front_door = Some(candidate);
                                PatchbayInvocationOutcome::Succeeded
                            }
                            Err(_) => PatchbayInvocationOutcome::Refused(
                                PatchbayRefusal::OperationRejected,
                            ),
                        }
                    },
                )
            }
            _ => PatchbayInvocationOutcome::Refused(PatchbayRefusal::OperationUnavailable),
        };
        let presentation = self.snapshot.presentation.clone();
        let receipt = self
            .interaction
            .execute_presentation(&presentation, request, |request| match request {
                PatchbayInteractionRequest::Invoke { invocation, .. }
                    if stale_presentation || invocation.target_identity != expected_target =>
                {
                    PatchbayInvocationOutcome::Refused(PatchbayRefusal::StalePresentation)
                }
                PatchbayInteractionRequest::Invoke { invocation, .. }
                    if invocation.action == PatchbayAction::ToggleLinearView =>
                {
                    PatchbayInvocationOutcome::Succeeded
                }
                PatchbayInteractionRequest::Invoke { invocation, .. }
                    if matches!(
                        invocation.action,
                        PatchbayAction::Plan | PatchbayAction::Play
                    ) =>
                {
                    prepared_outcome
                }
                PatchbayInteractionRequest::Edit { edit, .. }
                    if stale_presentation
                        || edit.basis().expanded_form_id.as_str() != expected_target =>
                {
                    PatchbayInvocationOutcome::Refused(PatchbayRefusal::StalePresentation)
                }
                _ => PatchbayInvocationOutcome::Refused(PatchbayRefusal::OperationUnavailable),
            })
            .map_err(|error| ServerError::Interaction(format!("{error:?}")))?;
        self.snapshot.interaction.revision = self.snapshot.interaction.revision.saturating_add(1);
        if receipt.disposition == InteractionDisposition::Succeeded {
            if let PatchbayInteractionRequest::Select {
                subject_identity, ..
            } = &receipt.request
            {
                self.snapshot
                    .entrance
                    .select(&presentation, subject_identity)
                    .map_err(|error| ServerError::Interaction(format!("{error:?}")))?;
                self.snapshot.interaction.selected_subject = Some(subject_identity.clone());
            }
        }
        if receipt.disposition == InteractionDisposition::Succeeded {
            if let Some(session) = prepared_front_door {
                let current = self.front_door.as_ref().ok_or_else(|| {
                    ServerError::Interaction("front-door session is absent".into())
                })?;
                *current.lock().map_err(|_| {
                    ServerError::Interaction("front-door session lock failed".into())
                })? = session;
                self.refresh_front_door()?;
            }
        }
        self.snapshot.interaction.last_request_id =
            Some(receipt.request.request_id().as_str().into());
        self.snapshot.interaction.last_disposition = Some(format!("{:?}", receipt.disposition));
        self.snapshot.interaction.interaction_plan_id = Some(receipt.plan_id.as_str().into());
        self.snapshot.interaction.interaction_play_id =
            Some(receipt.active_play_id.as_str().into());
        self.encoded_snapshot = self.snapshot.encode()?;
        Ok(self.encoded_snapshot.clone())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HtmlInteractionInput {
    presentation_id: String,
    kind: String,
    subject: Option<String>,
    action: Option<String>,
    target: Option<String>,
    edit: Option<HtmlEditInput>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HtmlEditInput {
    source_document_id: String,
    source_revision: u64,
    expanded_form_id: String,
    operation: String,
    primary: String,
    secondary: Option<String>,
    key: Option<String>,
    value: Option<conduit_core::ConfigurationValue>,
}

fn parse_html_edit(input: HtmlEditInput) -> Result<PatchbayEdit, ServerError> {
    let basis = PatchbayEditBasis::new(
        conduit_core::SourceDocumentId::from(input.source_document_id),
        input.source_revision,
        conduit_core::ExpandedFormId::from(input.expanded_form_id),
    )
    .map_err(|_| ServerError::InvalidRequest)?;
    match input.operation.as_str() {
        "place-gear" => Ok(PatchbayEdit::PlaceGear {
            basis,
            kind_id: input.primary,
        }),
        "duplicate-gear" => Ok(PatchbayEdit::DuplicateGear {
            basis,
            subject_identity: input.primary,
        }),
        "remove-gear" => Ok(PatchbayEdit::RemoveGear {
            basis,
            subject_identity: input.primary,
        }),
        "remove-cord" => Ok(PatchbayEdit::RemoveCord {
            basis,
            subject_identity: input.primary,
        }),
        "connect-ports" => Ok(PatchbayEdit::ConnectPorts {
            basis,
            source_identity: input.primary,
            sink_identity: input.secondary.ok_or(ServerError::InvalidRequest)?,
        }),
        "reroute-cord" => Ok(PatchbayEdit::RerouteCord {
            basis,
            cord_identity: input.primary,
            endpoint_identity: input.secondary.ok_or(ServerError::InvalidRequest)?,
        }),
        "configure-gear" => Ok(PatchbayEdit::ConfigureGear {
            basis,
            subject_identity: input.primary,
            key: input.key.ok_or(ServerError::InvalidRequest)?,
            value: input.value.ok_or(ServerError::InvalidRequest)?,
        }),
        _ => Err(ServerError::InvalidRequest),
    }
}

fn parse_html_action(value: &str) -> Result<PatchbayAction, ServerError> {
    match value {
        "toggle-linear-view" => Ok(PatchbayAction::ToggleLinearView),
        "plan" => Ok(PatchbayAction::Plan),
        "play" => Ok(PatchbayAction::Play),
        _ => Err(ServerError::InvalidRequest),
    }
}
