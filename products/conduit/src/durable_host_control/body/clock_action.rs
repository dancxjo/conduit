//! Exact Face actions that control the installed owner clock.
use super::*;

impl DurableHostRuntime {
    pub(crate) fn owned_body_local_interaction(
        &mut self,
        show: &MaskShow,
        interaction: &FaceInteraction,
    ) -> Result<serde_json::Value, String> {
        #[cfg(unix)]
        if self
            .terminal_route
            .as_ref()
            .is_some_and(|route| route.show.show_id == show.show_id)
        {
            return Err("terminal-owner-route-read-only".into());
        }
        self.owned_body_clock_action(None, show, interaction)
    }

    pub(crate) fn owned_body_browser_interaction(
        &mut self,
        window_id: &str,
        binding: &conduit_core::LinkBindingId,
        request: &OwnerFaceSnapshotRequest,
        show: &MaskShow,
        interaction: &FaceInteraction,
    ) -> Result<serde_json::Value, String> {
        {
            let HostSource::Body { owner, .. } = &self.host else {
                return Err("installed Host does not own a live Body session".into());
            };
            owner.validate_browser_mask_show(window_id, binding, request, show)?;
        }
        self.owned_body_clock_action(Some(request), show, interaction)
    }

    /// The existing native guest return remains a distinct attended carrier;
    /// it does not borrow the browser window's selected route.
    pub(crate) fn owned_body_native_guest_interaction(
        &mut self,
        request: &OwnerFaceSnapshotRequest,
        show: &MaskShow,
        interaction: &FaceInteraction,
    ) -> Result<serde_json::Value, String> {
        let HostSource::Body { owner, .. } = &self.host else {
            return Err("installed Host does not own a live Body session".into());
        };
        owner.validate_native_mask_show(request, show)?;
        self.owned_body_clock_action(Some(request), show, interaction)
    }

    fn owned_body_clock_action(
        &mut self,
        browser: Option<&OwnerFaceSnapshotRequest>,
        show: &MaskShow,
        interaction: &FaceInteraction,
    ) -> Result<serde_json::Value, String> {
        let action = {
            let HostSource::Body { owner, .. } = &self.host else {
                return Err("installed Host does not own a live Body session".into());
            };
            if let Some(request) = browser {
                // Membership and the current Host/Boot remain distinct from
                // the action's exact Face and acknowledged Show basis.
                let face = owner.face_snapshot(request)?;
                if show.show.host_id != request.host_id
                    || show.show.boot_id != request.boot_id
                    || show.show.body_id.as_ref() != Some(&request.body_id)
                    || show.presentation_id != face.identity
                    || show.presentation_revision != face.revision
                {
                    return Err("browser-show-basis-mismatch".into());
                }
            }
            owner.resolve_clock_interaction(show, interaction)?
        };
        match action {
            crate::durable_host::owner::ClockAction::ChangeInterval => {
                let HostSource::Body {
                    owner,
                    root,
                    running,
                } = &mut self.host
                else {
                    unreachable!("clock action was validated on an installed Body")
                };
                if running.is_some() {
                    return Err("clock-play-must-lull".into());
                }
                owner.apply_clock_interval_interaction(root, show, interaction)
            }
            crate::durable_host::owner::ClockAction::Start => {
                self.start_owned_body(crate::durable_host::owner::CLOCK_RUN_MAXIMUM_MILLIS)?;
                Ok(serde_json::json!({
                    "schema":"conduit.body/clock-start-requested@1",
                    "body_id":show.show.body_id,
                    "prior_face_id":interaction.face_id,
                    "prior_show_id":interaction.show_id,
                    "interaction_id":interaction.identity.as_str(),
                    "maximum_millis":crate::durable_host::owner::CLOCK_RUN_MAXIMUM_MILLIS,
                    "play_state":"preparing"
                }))
            }
            crate::durable_host::owner::ClockAction::Lull => {
                let play = self.request_owned_body_lull()?;
                Ok(serde_json::json!({
                    "schema":"conduit.body/clock-lull-requested@1",
                    "body_id":show.show.body_id,
                    "prior_face_id":interaction.face_id,
                    "prior_show_id":interaction.show_id,
                    "interaction_id":interaction.identity.as_str(),
                    "active_play_id":play,
                    "play_state":"stop-requested"
                }))
            }
        }
    }
}
