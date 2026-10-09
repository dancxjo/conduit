//! A command already resolved against the owner's exact committed Face.
use super::*;

impl TodoWaitingWorker {
    pub(crate) fn submit_committed_action(
        &mut self,
        owner: &Owner,
        expected: &TodoState,
        command: &conduit_todo_plot::TodoCommand,
        show: &MaskShow,
        interaction: &FaceInteraction,
    ) -> Result<BodyLiveForeAdmission, String> {
        let (play, current) = owner
            .todo_live
            .as_ref()
            .ok_or("Todo waiting Play has not been retained")?;
        if owner.current_play_id() != Some(play)
            || current != expected
            || current != &self.initial
            || self.accepted_interaction.is_some()
        {
            return Err("committed Todo action differs from newly admitted Play".into());
        }
        // Retaining the owner's started event acknowledges the runner callback;
        // the runner marks its queue started immediately after that callback.
        // Wait for that existing handshake before attempting queue admission.
        self.queue.wait_until_play_started()?;
        let admission = self.queue.submit(&command.encode_info().map_err(debug)?)?;
        if matches!(admission, BodyLiveForeAdmission::Accepted { .. }) {
            self.accepted_interaction = Some(interaction.identity.clone());
            self.accepted_mask = Some((show.clone(), interaction.clone()));
        }
        Ok(admission)
    }
}
