//! Exact, bounded active-Form changes for an attached ordinary Body.

use super::{PatchbayHtmlServer, ServerError};
use conduit_core::SignId;
use conduit_presentation::PresentationActionAvailability;
use serde::Deserialize;
use std::net::TcpStream;

pub(super) fn open(
    snapshot: &crate::RendererSnapshot,
) -> Result<Option<patchbay_model::PatchbayBodyWorkloadSession>, ServerError> {
    snapshot
        .body_workbench
        .as_ref()
        .map(|workbench| {
            patchbay_model::PatchbayBodyWorkloadSession::open_serialized(
                &workbench.encoded_evidence,
                crate::body_workbench::model_entrance(&workbench.entrance),
            )
            .map_err(|error| ServerError::Interaction(format!("{error:?}")))
        })
        .transpose()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BodyWorkloadInput {
    presentation_id: String,
    presentation_revision: u64,
    workload_revision: u64,
    action_id: String,
}

impl PatchbayHtmlServer {
    pub(super) fn current_body_evidence(&self) -> Result<Vec<u8>, ServerError> {
        self.body_workload
            .as_ref()
            .map(|session| session.encoded_evidence().to_vec())
            .ok_or(ServerError::InvalidRequest)
    }

    pub(super) fn deliver_body_evidence(&self, stream: &mut TcpStream) -> Result<(), ServerError> {
        let evidence = self.current_body_evidence()?;
        super::write_response(
            stream,
            "200 OK",
            "application/json; charset=utf-8",
            &evidence,
        )
    }

    pub(super) fn deliver_body_workload(
        &mut self,
        stream: &mut TcpStream,
        bytes: &[u8],
    ) -> Result<(), ServerError> {
        let body = match self.apply_body_workload(bytes) {
            Ok(body) => body,
            Err(ServerError::InvalidRequest | ServerError::Interaction(_)) => {
                return super::write_response(
                    stream,
                    "400 Bad Request",
                    "text/plain; charset=utf-8",
                    b"invalid Body workload request",
                );
            }
            Err(error) => return Err(error),
        };
        super::write_response(stream, "200 OK", "application/json; charset=utf-8", &body)
    }

    pub(super) fn apply_body_workload(&mut self, bytes: &[u8]) -> Result<Vec<u8>, ServerError> {
        let input: BodyWorkloadInput =
            serde_json::from_slice(bytes).map_err(|_| ServerError::InvalidRequest)?;
        let stale_presentation = input.presentation_id
            != self.snapshot.presentation.identity.as_str()
            || input.presentation_revision != self.snapshot.presentation.revision;
        let action = self
            .snapshot
            .presentation
            .actions
            .iter()
            .find(|action| action.identity == input.action_id)
            .cloned();
        if stale_presentation {
            return self.body_workload_refusal("StalePresentation");
        }
        let Some(action) = action else {
            return self.body_workload_refusal("UnknownAction");
        };
        let operation = match action.intent.as_str() {
            "conduit.intent/add-form@1" => "add",
            "conduit.intent/remove-form@1" => "remove",
            _ => return self.body_workload_refusal("ActionUnavailable"),
        };
        if !matches!(
            action.availability,
            PresentationActionAvailability::Available
        ) {
            return self.body_workload_refusal("ActionUnavailable");
        }

        let mut candidate =
            self.body_workload.as_ref().cloned().ok_or_else(|| {
                ServerError::Interaction("Body workload session is absent".into())
            })?;
        let form = if operation == "remove" {
            candidate
                .evidence()
                .body
                .workset
                .forms()
                .iter()
                .find(|form| action.target == format!("form/{}", form.checked_form_id.as_str()))
                .cloned()
        } else {
            self.snapshot
                .body_workbench
                .as_ref()
                .and_then(|workbench| {
                    workbench
                        .reviewed_forms
                        .iter()
                        .find(|form| action.target == format!("form/{}", form.checked_form_id))
                })
                .map(|form| {
                    conduit_body::ResidentForm::new(
                        conduit_core::SourceDocumentId::from(form.source_document_id.as_str()),
                        conduit_core::CheckedFormId::from(form.checked_form_id.as_str()),
                    )
                })
        };
        let Some(form) = form else {
            return self.body_workload_refusal("WrongTarget");
        };
        let biography_sequence = candidate
            .evidence()
            .records
            .last()
            .and_then(|record| record.sequence.checked_add(1))
            .ok_or_else(|| ServerError::Interaction("Body biography sequence exhausted".into()))?;
        let next_workload_revision = input
            .workload_revision
            .checked_add(1)
            .ok_or_else(|| ServerError::Interaction("Body workload revision exhausted".into()))?;
        let sign_id = SignId::from(format!(
            "patchbay-html/body-workload/{operation}/{next_workload_revision}"
        ));
        let changed = if operation == "remove" {
            candidate.remove_form(input.workload_revision, form, sign_id, biography_sequence)
        } else {
            candidate.admit_form(input.workload_revision, form, sign_id, biography_sequence)
        };
        if changed.is_err() {
            return self.body_workload_refusal("OperationRejected");
        }

        let prior = self
            .snapshot
            .body_workbench
            .as_ref()
            .ok_or_else(|| ServerError::Interaction("Body workbench is absent".into()))?;
        let evidence_revision = prior
            .evidence_revision
            .checked_add(1)
            .ok_or_else(|| ServerError::Interaction("Body evidence revision exhausted".into()))?;
        let entrance = prior.entrance.clone();
        let reviewed_forms = prior.reviewed_forms.clone();
        let prior_interaction = self.snapshot.interaction.clone();
        let mut snapshot = crate::body_workbench::body_workbench_snapshot_with_reviewed(
            evidence_revision,
            candidate.encoded_evidence(),
            entrance,
            &reviewed_forms,
        )
        .map_err(|error| ServerError::Interaction(error.to_string()))?;
        snapshot.mark_available(SignId::from(format!(
            "patchbay-html/body-workload/evidence-{evidence_revision}/available"
        )))?;
        snapshot.interaction = prior_interaction;
        snapshot.interaction.revision = snapshot.interaction.revision.saturating_add(1);
        snapshot.interaction.last_request_id = Some(format!(
            "body-workload/{operation}/{next_workload_revision}"
        ));
        snapshot.interaction.last_disposition = Some("Succeeded".into());
        snapshot.body_host_offer_evidence = self.snapshot.body_host_offer_evidence.clone();
        snapshot.body_host_planning_offer = self.snapshot.body_host_planning_offer.clone();
        self.body_workload = Some(candidate);
        self.snapshot = snapshot;
        self.navigation = super::navigation_state(&self.snapshot)?;
        self.encoded_snapshot = self.snapshot.encode()?;
        Ok(self.encoded_snapshot.clone())
    }

    fn body_workload_refusal(&mut self, reason: &str) -> Result<Vec<u8>, ServerError> {
        self.snapshot.interaction.revision = self.snapshot.interaction.revision.saturating_add(1);
        self.snapshot.interaction.last_request_id = Some("body-workload/refused".into());
        self.snapshot.interaction.last_disposition = Some(format!("Refused({reason})"));
        self.encoded_snapshot = self.snapshot.encode()?;
        Ok(self.encoded_snapshot.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(server: &PatchbayHtmlServer, workload_revision: u64, intent: &str) -> Vec<u8> {
        let action = server
            .snapshot
            .presentation
            .actions
            .iter()
            .find(|action| action.intent == intent)
            .unwrap();
        serde_json::to_vec(&serde_json::json!({
            "presentation_id": server.snapshot.presentation.identity,
            "presentation_revision": server.snapshot.presentation.revision,
            "workload_revision": workload_revision,
            "action_id": action.identity,
        }))
        .unwrap()
    }

    #[test]
    fn exact_remove_refreshes_the_same_body_and_stale_workload_refuses_atomically() {
        let snapshot = crate::body_workbench_fixture_snapshot(false).unwrap();
        let body_id = snapshot.body_workbench.as_ref().unwrap().body_id.clone();
        let mut server = PatchbayHtmlServer::bind_ephemeral(&snapshot).unwrap();
        let mut stale_presentation: serde_json::Value =
            serde_json::from_slice(&request(&server, 0, "conduit.intent/remove-form@1")).unwrap();
        stale_presentation["presentation_revision"] = serde_json::json!(99);
        let stale_presentation: crate::RendererSnapshot = serde_json::from_slice(
            &server
                .apply_body_workload(&serde_json::to_vec(&stale_presentation).unwrap())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            stale_presentation.interaction.last_disposition.as_deref(),
            Some("Refused(StalePresentation)")
        );
        assert_eq!(
            stale_presentation.body_workbench.as_ref().unwrap().current["workload_revision"],
            0
        );

        let stale = request(&server, 9, "conduit.intent/remove-form@1");
        let stale: crate::RendererSnapshot =
            serde_json::from_slice(&server.apply_body_workload(&stale).unwrap()).unwrap();
        assert_eq!(
            stale.interaction.last_disposition.as_deref(),
            Some("Refused(OperationRejected)")
        );
        assert_eq!(
            stale.body_workbench.as_ref().unwrap().current["workload_revision"],
            0
        );

        let remove = request(&server, 0, "conduit.intent/remove-form@1");
        let removed: crate::RendererSnapshot =
            serde_json::from_slice(&server.apply_body_workload(&remove).unwrap()).unwrap();
        let workbench = removed.body_workbench.unwrap();
        assert_eq!(workbench.body_id, body_id);
        assert_eq!(workbench.current["workload_revision"], 1);
        assert_eq!(
            workbench.current["active_forms"].as_array().unwrap().len(),
            1
        );
        assert_eq!(workbench.history["entries"].as_array().unwrap().len(), 5);
        assert_eq!(
            removed.interaction.last_disposition.as_deref(),
            Some("Succeeded")
        );

        let last = request(&server, 1, "conduit.intent/remove-form@1");
        let last: crate::RendererSnapshot =
            serde_json::from_slice(&server.apply_body_workload(&last).unwrap()).unwrap();
        let workbench = last.body_workbench.as_ref().unwrap();
        assert_eq!(
            last.interaction.last_disposition.as_deref(),
            Some("Succeeded")
        );
        assert_eq!(workbench.current["workload_revision"], 2);
        assert!(workbench.current["active_forms"]
            .as_array()
            .unwrap()
            .is_empty());
        assert_eq!(workbench.history["entries"].as_array().unwrap().len(), 6);
        let navigation = last.navigation.as_ref().unwrap();
        assert_eq!(
            navigation.cursor.place,
            conduit_presentation::PresentationPlace::Body
        );
        assert_eq!(navigation.navigation.places.len(), 1);

        let add = request(&server, 2, "conduit.intent/add-form@1");
        let added: crate::RendererSnapshot =
            serde_json::from_slice(&server.apply_body_workload(&add).unwrap()).unwrap();
        let workbench = added.body_workbench.unwrap();
        assert_eq!(workbench.body_id, body_id);
        assert_eq!(workbench.current["workload_revision"], 3);
        assert_eq!(
            workbench.current["active_forms"].as_array().unwrap().len(),
            1
        );
        assert_eq!(workbench.history["entries"].as_array().unwrap().len(), 7);
        let exported: conduit_body::BodyBiographyEvidence =
            serde_json::from_slice(&server.current_body_evidence().unwrap()).unwrap();
        assert_eq!(exported.body.body_id.as_str(), body_id);
        assert_eq!(exported.body.workload_revision, 3);
        assert_eq!(exported.body.workset.forms().len(), 1);
    }
}
