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
        let stale_presentation = input.presentation_id
            != self.snapshot.presentation.identity.as_str()
            || input.presentation_revision != self.snapshot.presentation.revision;
        if input.kind == "invoke"
            && !stale_presentation
            && self.body_planning.is_some()
            && self.snapshot.presentation.actions.iter().any(|action| {
                Some(action.identity.as_str()) == input.action_id.as_deref()
                    && action.intent == "conduit.intent/lull@1"
                    && matches!(
                        action.availability,
                        conduit_presentation::PresentationActionAvailability::Available
                    )
                    && self
                        .snapshot
                        .presentation
                        .basis
                        .body_id
                        .as_ref()
                        .is_some_and(|id| action.target == format!("body/{}", id.as_str()))
            })
        {
            let wake_id = self
                .body_planning
                .as_ref()
                .expect("checked session")
                .wake()
                .wake_id
                .clone();
            let request = serde_json::to_vec(&serde_json::json!({
                "schema": "conduit.patchbay/body-execution-request@1",
                "action": {"kind": "Lull", "wake_id": wake_id},
            }))
            .map_err(|_| ServerError::InvalidRequest)?;
            return self.apply_body_execution(&request);
        }
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
        let mut request = match input.kind.as_str() {
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
                &self.snapshot.presentation,
                input
                    .action_id
                    .as_deref()
                    .ok_or(ServerError::InvalidRequest)?,
            ),
            "edit" => PatchbayInteractionRequest::edit(
                request_id,
                parse_html_edit(input.edit.ok_or(ServerError::InvalidRequest)?)?,
            ),
            _ => return Err(ServerError::InvalidRequest),
        }
        .map_err(|error| ServerError::Interaction(format!("{error:?}")))?;
        if let PatchbayInteractionRequest::Invoke { invocation, .. } = &mut request {
            invocation.presentation_id = input.presentation_id.clone();
            invocation.presentation_revision = input.presentation_revision;
        }
        let expected_form_target = self
            .snapshot
            .presentation
            .basis
            .expanded_form_id
            .as_ref()
            .map(|identity| identity.as_str().to_owned());
        let requested_action = match &request {
            PatchbayInteractionRequest::Invoke { invocation, .. } => Some(invocation.action),
            _ => None,
        };
        let mut prepared_front_door = None;
        let mut prepared_zero_body = None;
        let prepared_outcome = match requested_action {
            Some(PatchbayAction::Wake | PatchbayAction::Plan | PatchbayAction::Play)
                if !stale_presentation
                    && matches!(
                        &request,
                        PatchbayInteractionRequest::Invoke { invocation, .. }
                            if Some(invocation.target_identity.as_str())
                                == expected_form_target.as_deref()
                    ) =>
            {
                self.front_door.as_ref().map_or(
                    PatchbayInvocationOutcome::Refused(PatchbayRefusal::OperationUnavailable),
                    |session| {
                        let Ok(session) = session.lock() else {
                            return PatchbayInvocationOutcome::Failed;
                        };
                        let mut candidate = session.clone();
                        let result = match requested_action {
                            Some(PatchbayAction::Wake) => candidate.wake_body().map(|_| ()),
                            Some(PatchbayAction::Plan) => candidate.plan_form().map(|_| ()),
                            Some(PatchbayAction::Play) => candidate.play_plan().map(|_| ()),
                            _ => unreachable!("guard restricts lifecycle action"),
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
            Some(PatchbayAction::OpenBack | PatchbayAction::Birth) if !stale_presentation => {
                self.zero_body_front_door.as_ref().map_or(
                    PatchbayInvocationOutcome::Refused(PatchbayRefusal::OperationUnavailable),
                    |session| {
                        let Ok(session) = session.lock() else {
                            return PatchbayInvocationOutcome::Failed;
                        };
                        let mut candidate = session.clone();
                        match &request {
                            PatchbayInteractionRequest::Invoke { invocation, .. } => {
                                let result = if invocation.action == PatchbayAction::Birth {
                                    candidate
                                        .birth(invocation.presentation_revision)
                                        .map(|born| {
                                            prepared_front_door = Some(born);
                                        })
                                } else {
                                    candidate
                                        .open_subject(
                                            &invocation.target_identity,
                                            invocation.presentation_revision,
                                        )
                                        .map(|_| prepared_zero_body = Some(candidate))
                                };
                                result
                                    .map(|_| PatchbayInvocationOutcome::Succeeded)
                                    .unwrap_or(PatchbayInvocationOutcome::Refused(
                                        PatchbayRefusal::OperationRejected,
                                    ))
                            }
                            _ => PatchbayInvocationOutcome::Refused(
                                PatchbayRefusal::OperationUnavailable,
                            ),
                        }
                    },
                )
            }
            Some(PatchbayAction::Save) if !stale_presentation => {
                self.zero_body_front_door.as_ref().map_or(
                    PatchbayInvocationOutcome::Refused(PatchbayRefusal::OperationUnavailable),
                    |session| {
                        let Ok(session) = session.lock() else {
                            return PatchbayInvocationOutcome::Failed;
                        };
                        let mut candidate = session.clone();
                        match save_opened_form(&mut candidate) {
                            Ok(()) => {
                                prepared_zero_body = Some(candidate);
                                PatchbayInvocationOutcome::Succeeded
                            }
                            Err(_) => PatchbayInvocationOutcome::Failed,
                        }
                    },
                )
            }
            None if !stale_presentation
                && matches!(&request, PatchbayInteractionRequest::Edit { .. })
                && self.zero_body_front_door.is_some() =>
            {
                let session = self.zero_body_front_door.as_ref().expect("guarded above");
                let Ok(session) = session.lock() else {
                    return Err(ServerError::Interaction(
                        "front-door session lock failed".into(),
                    ));
                };
                let mut candidate = session.clone();
                let PatchbayInteractionRequest::Edit { edit, .. } = &request else {
                    unreachable!("guard restricts request")
                };
                let outcome = candidate.apply_opened_form_edit(edit);
                if outcome == PatchbayInvocationOutcome::Succeeded {
                    prepared_zero_body = Some(candidate);
                }
                outcome
            }
            _ => PatchbayInvocationOutcome::Refused(PatchbayRefusal::OperationUnavailable),
        };
        let zero_body_edit_prepared = matches!(&request, PatchbayInteractionRequest::Edit { .. })
            && self.zero_body_front_door.is_some();
        let presentation = self.snapshot.presentation.clone();
        let receipt = self
            .interaction
            .execute_presentation(&presentation, request, |request| match request {
                PatchbayInteractionRequest::Invoke { .. } if stale_presentation => {
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
                        PatchbayAction::OpenBack
                            | PatchbayAction::Birth
                            | PatchbayAction::Wake
                            | PatchbayAction::Plan
                            | PatchbayAction::Play
                            | PatchbayAction::Save
                    ) =>
                {
                    prepared_outcome
                }
                PatchbayInteractionRequest::Edit { .. } if stale_presentation => {
                    PatchbayInvocationOutcome::Refused(PatchbayRefusal::StalePresentation)
                }
                PatchbayInteractionRequest::Edit { .. } if zero_body_edit_prepared => {
                    prepared_outcome
                }
                PatchbayInteractionRequest::Edit { edit, .. }
                    if Some(edit.basis().expanded_form_id.as_str())
                        != expected_form_target.as_deref() =>
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
                if let Some(current) = &self.front_door {
                    *current.lock().map_err(|_| {
                        ServerError::Interaction("front-door session lock failed".into())
                    })? = session;
                } else {
                    self.front_door = Some(std::sync::Arc::new(std::sync::Mutex::new(session)));
                    self.zero_body_front_door = None;
                }
                self.refresh_front_door()?;
            }
            if let Some(session) = prepared_zero_body {
                let current = self.zero_body_front_door.as_ref().ok_or_else(|| {
                    ServerError::Interaction("zero-Body front-door session is absent".into())
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

fn save_opened_form(session: &mut patchbay_model::ZeroBodyFrontDoor) -> Result<(), String> {
    let document = session
        .opened_form_document()
        .ok_or("SAVE requires an opened Form")?;
    let parent = document
        .path
        .parent()
        .ok_or("Form source has no parent directory")?;
    let file_name = document
        .path
        .file_name()
        .ok_or("Form source has no file name")?;
    let temporary = parent.join(format!(
        ".{}.patchbay-html-save",
        file_name.to_string_lossy()
    ));
    std::fs::write(&temporary, document.source.as_bytes())
        .map_err(|error| format!("write temporary canonical Form: {error}"))?;
    if let Err(error) = std::fs::rename(&temporary, &document.path) {
        let _ = std::fs::remove_file(&temporary);
        return Err(format!("replace canonical Form: {error}"));
    }
    session.mark_opened_form_saved(document.revision)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HtmlInteractionInput {
    presentation_id: String,
    presentation_revision: u64,
    kind: String,
    subject: Option<String>,
    action_id: Option<String>,
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
