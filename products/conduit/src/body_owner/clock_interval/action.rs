//! Resolve lifecycle and navigation actions against the exact current Face and Show.
use super::{
    debug, ClockAction, FaceInteraction, MaskShow, Owner, BODY_LULL_ACTION, BODY_WAKE_ACTION,
    CLOCK_INTERVAL_ACTION, CLOCK_LULL_ACTION, CLOCK_START_ACTION,
};

impl Owner {
    pub(crate) fn resolve_clock_interaction(
        &self,
        show: &MaskShow,
        interaction: &FaceInteraction,
    ) -> Result<ClockAction, String> {
        let face = self.local_face_snapshot()?;
        show.validate(&face)
            .map_err(|error| format!("stale owner Mask Show: {error:?}"))?;
        interaction
            .validate_against(&face, show)
            .map_err(|error| format!("owner Face interaction refused: {error:?}"))?;
        let action = face
            .resolve_action(interaction.face_revision, &interaction.action_id)
            .map_err(debug)?;
        // The installed clock's Wake prepares its current checked workset and
        // starts the owner Play; Lull requests that same Play's terminal sign.
        // These are the lifecycle operations performed by Start and Stop below,
        // through one owner path rather than a Mask-owned state transition.
        match (action.intent.as_str(), interaction.arguments.len()) {
            (CLOCK_INTERVAL_ACTION, 1) => Ok(ClockAction::ChangeInterval),
            (CLOCK_START_ACTION | BODY_WAKE_ACTION, 0) => Ok(ClockAction::Start),
            (CLOCK_LULL_ACTION | BODY_LULL_ACTION, 0) => Ok(ClockAction::Lull),
            (
                "conduit.intent/open-body-overview@1"
                | "conduit.intent/open-plot-library@1"
                | "conduit.intent/open-resident-plot@1",
                0,
            ) if self.has_open_notebook() && self.current_play_id().is_some() => {
                Ok(ClockAction::NotebookNavigation)
            }
            ("conduit.intent/open-inspection@1" | "conduit.intent/open-body-overview@1", 0)
                if self.current_home_fragment().is_some() =>
            {
                Ok(ClockAction::HomeNavigation)
            }
            _ => Err("owner action is not a current clock control".into()),
        }
    }
}
