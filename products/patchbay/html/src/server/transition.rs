//! Explicit zero-Body OPEN, JOIN, and BIRTH delivery actions.

use super::{PatchbayHtmlServer, ServerError};
use serde::Deserialize;
use std::sync::{Arc, Mutex};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TransitionInput {
    presentation_id: String,
    revision: u64,
    action: String,
    subject: Option<String>,
}

impl PatchbayHtmlServer {
    pub(super) fn apply_front_door_transition(
        &mut self,
        bytes: &[u8],
    ) -> Result<Vec<u8>, ServerError> {
        let input: TransitionInput =
            serde_json::from_slice(bytes).map_err(|_| ServerError::InvalidRequest)?;
        self.snapshot.interaction.revision = self.snapshot.interaction.revision.saturating_add(1);
        if input.presentation_id != self.snapshot.presentation.identity.as_str()
            || input.revision != self.snapshot.revision
        {
            self.snapshot.interaction.last_disposition = Some("Refused(StalePresentation)".into());
            if let Some(session) = &self.zero_body_front_door {
                session
                    .lock()
                    .map_err(|_| ServerError::Interaction("front-door session lock failed".into()))?
                    .record_refusal("StalePresentation")
                    .map_err(ServerError::Interaction)?;
                self.refresh_front_door()?;
            }
            self.encoded_snapshot = self.snapshot.encode()?;
            return Ok(self.encoded_snapshot.clone());
        }
        let Some(session) = self.zero_body_front_door.as_ref() else {
            self.snapshot.interaction.last_disposition = Some("Refused(AlreadyEmbodied)".into());
            self.encoded_snapshot = self.snapshot.encode()?;
            return Ok(self.encoded_snapshot.clone());
        };
        let candidate = session
            .lock()
            .map_err(|_| ServerError::Interaction("front-door session lock failed".into()))?
            .clone();
        let result = match input.action.as_str() {
            "open" => {
                let subject = input.subject.ok_or(ServerError::InvalidRequest)?;
                let result = session
                    .lock()
                    .map_err(|_| ServerError::Interaction("front-door session lock failed".into()))?
                    .open_subject(&subject, input.revision);
                result.map(|_| None)
            }
            "join" => candidate.join_open_body(input.revision).map(Some),
            "birth" => candidate.birth(input.revision).map(Some),
            _ => return Err(ServerError::InvalidRequest),
        };
        match result {
            Ok(Some(embodied)) => {
                self.front_door = Some(Arc::new(Mutex::new(embodied)));
                self.zero_body_front_door = None;
                self.snapshot.interaction.last_disposition = Some("Succeeded".into());
            }
            Ok(None) => {
                self.snapshot.interaction.last_disposition = Some("Succeeded".into());
            }
            Err(error) => {
                self.snapshot.interaction.last_disposition = Some(format!("Refused({error})"));
            }
        }
        self.refresh_front_door()?;
        self.snapshot.interaction.last_request_id = Some(format!(
            "front-door/{}/{}",
            input.action, self.snapshot.interaction.revision
        ));
        self.encoded_snapshot = self.snapshot.encode()?;
        Ok(self.encoded_snapshot.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_presentation::PresentationRole;

    fn request(server: &PatchbayHtmlServer, action: &str, subject: Option<&str>) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "presentation_id": server.snapshot.presentation.identity.as_str(),
            "revision": server.snapshot.revision,
            "action": action,
            "subject": subject,
        }))
        .unwrap()
    }

    #[test]
    fn html_open_form_is_inert_then_explicit_birth_establishes_body() {
        let explicit = patchbay_model::FormCandidate::from_source(
            "Text Lab",
            "text-lab.conduit",
            include_str!("../../../../../forms/text-lab/main.conduit"),
            "checked test source",
            conduit_core::SignId::from("test/text-lab/checked"),
            1,
        )
        .unwrap();
        let mut server =
            PatchbayHtmlServer::bind_front_door_with_forms_ephemeral(vec![explicit]).unwrap();
        let form = server
            .snapshot
            .presentation
            .subjects
            .iter()
            .find(|subject| subject.role == PresentationRole::Form && subject.label == "Text Lab")
            .unwrap()
            .identity
            .clone();
        let open = request(&server, "open", Some(&form));
        let opened = server.apply_front_door_transition(&open).unwrap();
        let opened: crate::RendererSnapshot = serde_json::from_slice(&opened).unwrap();
        assert!(opened.presentation.basis.body_id.is_none());
        assert_eq!(
            opened.interaction.last_disposition.as_deref(),
            Some("Succeeded")
        );
        assert!(opened.presentation.properties.iter().any(|property| {
            property.subject == form
                && property.name == "opened"
                && property.value == conduit_presentation::PresentationPropertyValue::Flag(true)
        }));

        let stale = serde_json::to_vec(&serde_json::json!({
            "presentation_id": opened.presentation.identity.as_str(),
            "revision": opened.revision - 1,
            "action": "birth",
            "subject": form,
        }))
        .unwrap();
        let stale: crate::RendererSnapshot =
            serde_json::from_slice(&server.apply_front_door_transition(&stale).unwrap()).unwrap();
        assert!(stale.presentation.basis.body_id.is_none());
        assert_eq!(
            stale.interaction.last_disposition.as_deref(),
            Some("Refused(StalePresentation)")
        );
        assert!(stale.presentation.subjects.iter().any(|subject| {
            subject.role == PresentationRole::Sign && subject.label == "Refused StalePresentation"
        }));

        let birth = server
            .snapshot
            .presentation
            .actions
            .iter()
            .find(|action| action.target == form && action.intent == "conduit.intent/birth@1")
            .unwrap();
        assert_eq!(
            birth.availability,
            conduit_presentation::PresentationActionAvailability::Available
        );
        let born = serde_json::to_vec(&serde_json::json!({
            "presentation_id": server.snapshot.presentation.identity.as_str(),
            "presentation_revision": server.snapshot.presentation.revision,
            "kind": "invoke",
            "subject": null,
            "action_id": birth.identity,
            "edit": null,
        }))
        .unwrap();
        let born: crate::RendererSnapshot =
            serde_json::from_slice(&server.apply_interaction(&born).unwrap()).unwrap();
        assert!(born.presentation.basis.body_id.is_some());
        assert_eq!(born.parts.as_ref().unwrap().parts.len(), 1);
        assert_eq!(
            born.interaction.last_disposition.as_deref(),
            Some("Succeeded")
        );
    }
}
