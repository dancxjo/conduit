//! Native owner-facing mailbox. Only canonical Face/Show/interaction values cross
//! this boundary; a UI event or an encoded Available Show is not acknowledgement.
use super::*;

#[derive(Clone)]
pub struct CrecheEncounterController {
    pub(super) channel: Arc<Channel>,
}
impl CrecheEncounterController {
    /// No Face is exposed before actual begin or after retirement.
    pub fn face(&self) -> Result<Option<Arc<Face>>, String> {
        let state = self.channel.lock()?;
        Ok(if state.active {
            state.face.clone()
        } else {
            None
        })
    }
    pub fn reviewed(&self) -> Result<bool, String> {
        Ok(self.channel.lock()?.reviewed)
    }
    pub fn acknowledge_show(&self, show: MaskShow) -> Result<(), String> {
        let mut state = self.channel.lock()?;
        if !state.active || state.reviewed || state.command.is_some() {
            return Err(
                "Crèche cannot acknowledge an inactive, reviewed or occupied encounter".into(),
            );
        }
        let face = state.face.as_ref().ok_or("Crèche current Face absent")?;
        show.validate(face)
            .map_err(|e| format!("Crèche Show identity: {e:?}"))?;
        if show.show.lifecycle != ManifestationLifecycle::Available {
            return Err("Crèche requires its selected Mask's actual acknowledged Show".into());
        }
        self.channel
            .custody
            .lock()
            .map_err(|_| "Crèche Mask custody poisoned")?
            .validate_acknowledged(face, &show)?;
        state.acknowledged = Some(Arc::new(show));
        Ok(())
    }
    pub fn submit(&self, interaction: FaceInteraction) -> Result<(), String> {
        let mut state = self.channel.lock()?;
        if !state.active || state.reviewed || state.command.is_some() {
            return Err("Crèche interaction mailbox is inactive, reviewed or full".into());
        }
        let face = state.face.as_ref().ok_or("Crèche current Face absent")?;
        let show = state
            .acknowledged
            .as_ref()
            .ok_or("Crèche has no actually acknowledged current Show")?;
        interaction
            .validate_against(face, show)
            .map_err(|e| format!("Crèche interaction: {e:?}"))?;
        // Recheck actual selected renderer custody at acceptance, not only when
        // its Show was initially acknowledged. No cloned Show conveys authority.
        self.channel
            .custody
            .lock()
            .map_err(|_| "Crèche Mask custody poisoned")?
            .validate_acknowledged(face, show)?;
        state.command = Some(Command {
            show: show.clone(),
            interaction,
        });
        Ok(())
    }
    /// Move the canonical review/draft once to the admitted make owner. This
    /// does not invoke make, create a Body, or mark any artifact prepared.
    pub fn take_reviewed(&self) -> Result<Option<CrecheReviewedHandoff>, String> {
        let mut state = self.channel.lock()?;
        if !state.active || !state.reviewed || state.handoff_taken {
            return Ok(None);
        }
        let handoff = state.handoff.take();
        if handoff.is_some() {
            state.handoff_taken = true;
        }
        Ok(handoff)
    }
    /// Called by the concrete make owner only after it has taken this exact
    /// canonical review. There is intentionally no successful-artifact setter.
    pub fn classify_make_refusal(&self, reason: CrecheMakeRefusal) -> Result<(), String> {
        let mut state = self.channel.lock()?;
        if !state.active || !state.handoff_taken || state.make_refusal.is_some() {
            return Err("Crèche make refusal has no single admitted reviewed operation".into());
        }
        state.make_refusal = Some(reason);
        Ok(())
    }
}
