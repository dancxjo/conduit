//! Explicit zero-body OPEN, JOIN, and BIRTH delivery actions.

use super::{PatchbayHtmlServer, ServerError};
use serde::Deserialize;
use std::sync::{Arc, Mutex};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TransitionInput {
    face_id: String,
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
        if input.face_id != self.snapshot.face.identity.as_str()
            || input.revision != self.snapshot.revision
        {
            self.snapshot.interaction.last_disposition = Some("Refused(StaleFace)".into());
            if let Some(session) = &self.zero_body_front_door {
                session
                    .lock()
                    .map_err(|_| ServerError::Interaction("front-door session lock failed".into()))?
                    .record_refusal("StaleFace")
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
    use conduit_face::FaceRole;

    fn request(server: &PatchbayHtmlServer, action: &str, subject: Option<&str>) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "face_id": server.snapshot.face.identity.as_str(),
            "revision": server.snapshot.revision,
            "action": action,
            "subject": subject,
        }))
        .unwrap()
    }

    #[test]
    fn html_open_plot_is_inert_then_explicit_birth_establishes_body() {
        let explicit = conduit_patchbay_workbench::PlotCandidate::from_source(
            "Text Lab",
            "text-lab.conduit",
            include_str!("../../../../../plots/text-lab/main.conduit"),
            "checked test source",
            conduit_core::SignId::from("test/text-lab/checked"),
            1,
        )
        .unwrap();
        let mut server =
            PatchbayHtmlServer::bind_front_door_with_plots_ephemeral(vec![explicit]).unwrap();
        let plot = server
            .snapshot
            .face
            .subjects
            .iter()
            .find(|subject| subject.role == FaceRole::Plot && subject.name == "Text Lab")
            .unwrap()
            .identity
            .clone();
        let open = request(&server, "open", Some(&plot));
        let opened = server.apply_front_door_transition(&open).unwrap();
        let opened: crate::RendererSnapshot = serde_json::from_slice(&opened).unwrap();
        assert!(opened.face.basis.body_id.is_none());
        assert_eq!(
            opened.interaction.last_disposition.as_deref(),
            Some("Succeeded")
        );
        assert!(opened.face.properties.iter().any(|property| {
            property.subject == plot
                && property.name == "opened"
                && property.value == conduit_face::FacePropertyValue::Flag(true)
        }));

        let stale = serde_json::to_vec(&serde_json::json!({
            "face_id": opened.face.identity.as_str(),
            "revision": opened.revision - 1,
            "action": "birth",
            "subject": plot,
        }))
        .unwrap();
        let stale: crate::RendererSnapshot =
            serde_json::from_slice(&server.apply_front_door_transition(&stale).unwrap()).unwrap();
        assert!(stale.face.basis.body_id.is_none());
        assert_eq!(
            stale.interaction.last_disposition.as_deref(),
            Some("Refused(StaleFace)")
        );
        assert!(stale.face.subjects.iter().any(|subject| {
            subject.role == FaceRole::Sign && subject.name == "Front-door refusal Sign StaleFace"
        }));

        let birth = server
            .snapshot
            .face
            .actions
            .iter()
            .find(|action| action.target == plot && action.intent == "conduit.intent/birth@1")
            .unwrap();
        assert_eq!(
            birth.availability,
            conduit_face::FaceActionAvailability::Available
        );
        let born = serde_json::to_vec(&serde_json::json!({
            "face_id": server.snapshot.face.identity.as_str(),
            "face_revision": server.snapshot.face.revision,
            "kind": "invoke",
            "subject": null,
            "action_id": birth.identity,
            "edit": null,
        }))
        .unwrap();
        let born: crate::RendererSnapshot =
            serde_json::from_slice(&server.apply_interaction(&born).unwrap()).unwrap();
        assert!(born.face.basis.body_id.is_some());
        assert_eq!(born.parts.as_ref().unwrap().parts.len(), 1);
        assert_eq!(
            born.interaction.last_disposition.as_deref(),
            Some("Succeeded")
        );
    }
}
