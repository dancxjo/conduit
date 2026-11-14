use super::*;

/// Immutable semantic view retaining its producing region's memory charge.
/// Keeping this view does not keep an action current or authorize another Play.
/// Owned Face copies require independent caller workspace; no raw Arc escapes.
#[derive(Clone)]
pub struct CrecheFace {
    // Free exported Face data before releasing its last memory witness.
    face: Arc<Face>,
    _memory: Option<conduit_std_host::body_execution::ImageMemoryRetention>,
}
impl core::ops::Deref for CrecheFace {
    type Target = Face;
    fn deref(&self) -> &Face {
        &self.face
    }
}

#[derive(Clone)]
pub struct CrecheEncounterController {
    pub(super) channel: ChannelCustody,
}
impl CrecheEncounterController {
    pub fn face(&self) -> Result<Option<CrecheFace>, String> {
        let state = self.channel.lock()?;
        Ok(if state.active {
            state.face.as_ref().map(|face| CrecheFace {
                face: face.clone(),
                _memory: self.channel.retain_memory(),
            })
        } else {
            None
        })
    }
    pub fn acknowledge_show(&self, show: MaskShow) -> Result<(), String> {
        let mut state = self.channel.lock()?;
        if !state.active || state.command.is_some() || state.checked.is_some() || state.started {
            return Err("Crèche cannot acknowledge inactive or occupied encounter".into());
        }
        let face = state.face.as_ref().ok_or("Crèche Face absent")?;
        show.validate(face)
            .map_err(|e| format!("Crèche Show: {e:?}"))?;
        if show.show.lifecycle != conduit_face::ManifestationLifecycle::Available {
            return Err("Crèche requires actual available Show".into());
        }
        self.channel
            .custody
            .try_lock()
            .map_err(|_| "Crèche selected Mask custody busy")?
            .validate_acknowledged(face, &show)?;
        state.acknowledged = Some(show);
        Ok(())
    }
    pub fn submit(&self, input: FaceInteraction) -> Result<(), String> {
        let mut state = self.channel.lock()?;
        if !state.active || state.command.is_some() || state.checked.is_some() || state.started {
            return Err("Crèche interaction mailbox inactive, reviewed or full".into());
        }
        let face = state.face.as_ref().ok_or("Crèche current Face absent")?;
        let show = state
            .acknowledged
            .as_ref()
            .ok_or("Crèche actual acknowledged Show absent")?;
        input
            .validate_against(face, show)
            .map_err(|e| format!("Crèche interaction: {e:?}"))?;
        let mut custody = self
            .channel
            .custody
            .try_lock()
            .map_err(|_| "Crèche selected Mask custody busy")?;
        custody.validate_acknowledged(face, show)?;
        custody.validate_returned_interaction(face, show, &input)?;
        state.command = Some(Command {
            show: show.clone(),
            input,
        });
        Ok(())
    }

    /// Output custody transfers only after the installed operation retained its
    /// exact Prepared receipt. There is no externally supplied success setter.
    pub fn take_products(&self) -> Result<Option<ProducedJoiningHost<File>>, String> {
        let mut state = self.channel.lock()?;
        if self.channel.is_image() {
            return Err("Crèche IMAGE products require retained output custody".into());
        }
        Ok(if state.published {
            state.products.take()
        } else {
            None
        })
    }
}
