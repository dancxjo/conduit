//! Resolve read-only navigation from the browser's acknowledged presentation.
use super::*;
use conduit_face::FaceOperatorActionKind;

impl Owner {
    pub(crate) fn navigate_acknowledged_browser_face(
        &mut self,
        window_id: &str,
        binding: &LinkBindingId,
        request: &OwnerFaceSnapshotRequest,
        show: &MaskShow,
        interaction: &FaceInteraction,
    ) -> Result<Option<serde_json::Value>, String> {
        let Some(BrowserWindow {
            state:
                WindowState::Active {
                    route: Some(route),
                    acknowledged_show: Some(acknowledged),
                    navigation_face: Some(navigation),
                    ..
                },
            ..
        }) = self.pending_browser.as_ref()
        else {
            return Ok(None);
        };
        let (play, presented) = navigation.as_ref();
        if self.current_play_id() != Some(play)
            || **acknowledged != *show
            || !interaction.arguments.is_empty()
        {
            return Ok(None);
        }
        let Some(previous_action) = presented.actions.iter().find(|action| {
            action.identity == interaction.action_id
                && action.arguments.is_empty()
                && matches!(
                    action.intent.as_str(),
                    "conduit.intent/open-inspection@1" | "conduit.intent/open-body-overview@1"
                )
        }) else {
            return Ok(None);
        };

        self.validate_face_request(request)?;
        let current = self.project_body_face(None)?;
        if current.face.identity == presented.identity {
            return Ok(None);
        }
        // Only navigation explicitly offered by both presentations survives a
        // content update. Body, Wake, execution provenance and interaction
        // context must still agree.
        if current.face.basis != presented.basis
            || current.face.interaction_context != presented.interaction_context
            || current.face != self.local_face_snapshot()?
            || !current.face.actions.contains(previous_action)
        {
            return Ok(None);
        }
        let action = current
            .resolve_operator_action(current.face.revision, &interaction.action_id)
            .map_err(|error| format!("inspection action refused: {error:?}"))?;
        if !matches!(
            action.kind,
            FaceOperatorActionKind::OpenInspection(_) | FaceOperatorActionKind::OpenOverview
        ) {
            return Ok(None);
        }
        self.validate_browser_show_against(window_id, binding, request, show, presented)?;
        interaction
            .validate_against(presented, show)
            .map_err(|error| format!("inspection interaction refused: {error:?}"))?;
        route
            .validate_interaction_payload(interaction.encode().len())
            .map_err(|error| format!("browser-mask-interaction-refused:{error:?}"))?;

        let result = self.apply_inspection_navigation(&action.kind, show, interaction)?;
        if let Some(BrowserWindow {
            state: WindowState::Active {
                navigation_face, ..
            },
            ..
        }) = self.pending_browser.as_mut()
        {
            *navigation_face = None;
        }
        Ok(Some(result))
    }
}
