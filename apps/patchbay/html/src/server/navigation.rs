//! Stale-fenced portable navigation over the current exact Presentation.

use super::{PatchbayHtmlServer, ServerError};
use conduit_presentation::{
    NavigationOperation, PresentationAspect, PresentationDepth, PresentationPlace,
};
use serde::Deserialize;

pub(super) fn navigation_state(
    snapshot: &crate::RendererSnapshot,
) -> Result<Option<conduit_presentation::NavigationState>, ServerError> {
    snapshot
        .navigation
        .as_ref()
        .map(|navigation| {
            conduit_presentation::NavigationState::new(
                &navigation.navigation,
                navigation.cursor.clone(),
                conduit_presentation::MAX_NAVIGATION_HISTORY,
            )
            .map_err(|error| ServerError::Interaction(format!("{error:?}")))
        })
        .transpose()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NavigationInput {
    presentation_id: String,
    presentation_revision: u64,
    navigation_id: String,
    operation: NavigationInputOperation,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum NavigationInputOperation {
    Enter {
        place: PresentationPlace,
    },
    Show {
        aspect: PresentationAspect,
    },
    Focus {
        subject: String,
        depth: Option<PresentationDepth>,
    },
    Follow {
        relationship: String,
    },
    Disclose {
        depth: PresentationDepth,
    },
    Back,
}

impl From<NavigationInputOperation> for NavigationOperation {
    fn from(value: NavigationInputOperation) -> Self {
        match value {
            NavigationInputOperation::Enter { place } => Self::Enter(place),
            NavigationInputOperation::Show { aspect } => Self::Show(aspect),
            NavigationInputOperation::Focus { subject, depth } => match depth {
                Some(depth) => Self::FocusAndDisclose(subject, depth),
                None => Self::Focus(subject),
            },
            NavigationInputOperation::Follow { relationship } => Self::Follow(relationship),
            NavigationInputOperation::Disclose { depth } => Self::Disclose(depth),
            NavigationInputOperation::Back => Self::Back,
        }
    }
}

impl PatchbayHtmlServer {
    pub(super) fn focus_debugger_subject(&mut self, subject: &str) -> Result<(), ServerError> {
        let Some(navigation) = self.snapshot.navigation.as_mut() else {
            return Err(ServerError::Interaction(
                "debugger subject navigation unavailable".into(),
            ));
        };
        let Some(state) = self.navigation.as_mut() else {
            return Err(ServerError::Interaction(
                "debugger subject navigation state unavailable".into(),
            ));
        };
        let cursor = state
            .navigate(
                &self.snapshot.presentation,
                &navigation.navigation,
                self.snapshot.presentation.revision,
                NavigationOperation::FocusAndDisclose(
                    subject.to_owned(),
                    PresentationDepth::Detail,
                ),
            )
            .map_err(|error| ServerError::Interaction(format!("debugger focus: {error:?}")))?;
        navigation.cursor = cursor.clone();
        self.snapshot.interaction.revision = self.snapshot.interaction.revision.saturating_add(1);
        self.snapshot.interaction.last_disposition = Some("Succeeded(DebuggerFocus)".into());
        Ok(())
    }

    pub(super) fn apply_navigation(&mut self, bytes: &[u8]) -> Result<Vec<u8>, ServerError> {
        let input: NavigationInput =
            serde_json::from_slice(bytes).map_err(|_| ServerError::InvalidRequest)?;
        self.snapshot.interaction.revision = self.snapshot.interaction.revision.saturating_add(1);
        let Some(navigation) = self.snapshot.navigation.as_mut() else {
            return self.finish_navigation("Refused(NavigationUnavailable)");
        };
        if input.presentation_id != self.snapshot.presentation.identity.as_str()
            || input.presentation_revision != self.snapshot.presentation.revision
        {
            return self.finish_navigation("Refused(StalePresentation)");
        }
        if input.navigation_id != navigation.navigation.identity.as_str() {
            return self.finish_navigation("Refused(StaleNavigation)");
        }
        let Some(state) = self.navigation.as_mut() else {
            return self.finish_navigation("Refused(NavigationUnavailable)");
        };
        match state.navigate(
            &self.snapshot.presentation,
            &navigation.navigation,
            input.presentation_revision,
            input.operation.into(),
        ) {
            Ok(cursor) => {
                navigation.cursor = cursor.clone();
                self.snapshot.interaction.last_disposition = Some("Succeeded".into());
            }
            Err(error) => {
                self.snapshot.interaction.last_disposition = Some(format!("Refused({error:?})"));
            }
        }
        self.snapshot.interaction.last_request_id =
            Some(format!("navigation/{}", self.snapshot.interaction.revision));
        self.encoded_snapshot = self.snapshot.encode()?;
        Ok(self.encoded_snapshot.clone())
    }

    pub(super) fn reset_navigation(&mut self) -> Result<(), ServerError> {
        self.navigation = navigation_state(&self.snapshot)?;
        Ok(())
    }

    fn finish_navigation(&mut self, disposition: &str) -> Result<Vec<u8>, ServerError> {
        self.snapshot.interaction.last_disposition = Some(disposition.into());
        self.snapshot.interaction.last_request_id =
            Some(format!("navigation/{}", self.snapshot.interaction.revision));
        self.encoded_snapshot = self.snapshot.encode()?;
        Ok(self.encoded_snapshot.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::SignId;
    use patchbay_model::SeedCandidate;

    fn server() -> PatchbayHtmlServer {
        let seed = SeedCandidate::from_source(
            "Text Lab",
            "text-lab.conduit",
            include_str!("../../../../../examples/text-lab.conduit"),
            "navigation test",
            SignId::from("test/navigation/seed"),
            1,
        )
        .unwrap();
        let mut server =
            PatchbayHtmlServer::bind_front_door_with_seeds_ephemeral(vec![seed]).unwrap();
        let seed = server
            .snapshot
            .presentation
            .subjects
            .iter()
            .find(|subject| subject.role == conduit_presentation::PresentationRole::Seed)
            .unwrap()
            .identity
            .clone();
        let action = server
            .snapshot
            .presentation
            .actions
            .iter()
            .find(|action| action.target == seed && action.intent == "conduit.intent/open@1")
            .unwrap();
        let request = serde_json::to_vec(&serde_json::json!({
            "presentation_id": server.snapshot.presentation.identity.as_str(),
            "presentation_revision": server.snapshot.presentation.revision,
            "kind": "invoke",
            "subject": null,
            "action_id": action.identity,
            "edit": null,
        }))
        .unwrap();
        server.apply_interaction(&request).unwrap();
        server
    }

    fn request(server: &PatchbayHtmlServer, operation: serde_json::Value) -> Vec<u8> {
        let navigation = server.snapshot.navigation.as_ref().unwrap();
        serde_json::to_vec(&serde_json::json!({
            "presentation_id": server.snapshot.presentation.identity.as_str(),
            "presentation_revision": server.snapshot.presentation.revision,
            "navigation_id": navigation.navigation.identity.as_str(),
            "operation": operation,
        }))
        .unwrap()
    }

    #[test]
    fn navigation_changes_only_the_portable_cursor_and_back_is_bounded() {
        let mut server = server();
        let before = server.snapshot.presentation.clone();
        let renderer = server.snapshot.renderer.clone();
        assert_eq!(
            server.snapshot.navigation.as_ref().unwrap().cursor.place,
            PresentationPlace::Program
        );

        let enter = request(
            &server,
            serde_json::json!({"kind":"enter", "place":"Entrance"}),
        );
        let entered: crate::RendererSnapshot =
            serde_json::from_slice(&server.apply_navigation(&enter).unwrap()).unwrap();
        assert_eq!(
            entered.navigation.as_ref().unwrap().cursor.place,
            PresentationPlace::Entrance
        );
        assert_eq!(entered.presentation, before);
        assert_eq!(entered.renderer, renderer);

        let back = request(&server, serde_json::json!({"kind":"back"}));
        let backed: crate::RendererSnapshot =
            serde_json::from_slice(&server.apply_navigation(&back).unwrap()).unwrap();
        assert_eq!(
            backed.navigation.as_ref().unwrap().cursor.place,
            PresentationPlace::Program
        );
        assert_eq!(backed.presentation, before);

        let exhausted = request(&server, serde_json::json!({"kind":"back"}));
        let exhausted: crate::RendererSnapshot =
            serde_json::from_slice(&server.apply_navigation(&exhausted).unwrap()).unwrap();
        assert_eq!(
            exhausted.interaction.last_disposition.as_deref(),
            Some("Refused(HistoryExhausted)")
        );
        assert_eq!(exhausted.presentation, before);
    }

    #[test]
    fn stale_navigation_and_unknown_place_refuse_without_moving_the_cursor() {
        let mut server = server();
        let before = server.snapshot.navigation.as_ref().unwrap().cursor.clone();
        let stale = serde_json::to_vec(&serde_json::json!({
            "presentation_id": server.snapshot.presentation.identity.as_str(),
            "presentation_revision": server.snapshot.presentation.revision,
            "navigation_id": "stale/navigation",
            "operation": {"kind":"enter", "place":"Entrance"},
        }))
        .unwrap();
        let stale: crate::RendererSnapshot =
            serde_json::from_slice(&server.apply_navigation(&stale).unwrap()).unwrap();
        assert_eq!(
            stale.interaction.last_disposition.as_deref(),
            Some("Refused(StaleNavigation)")
        );
        assert_eq!(stale.navigation.as_ref().unwrap().cursor, before);

        let unknown = request(&server, serde_json::json!({"kind":"enter", "place":"Body"}));
        let unknown: crate::RendererSnapshot =
            serde_json::from_slice(&server.apply_navigation(&unknown).unwrap()).unwrap();
        assert_eq!(
            unknown.interaction.last_disposition.as_deref(),
            Some("Refused(UnknownPlace)")
        );
        assert_eq!(unknown.navigation.as_ref().unwrap().cursor, before);
    }
}
