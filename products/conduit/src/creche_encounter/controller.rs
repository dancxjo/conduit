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
        if !state.active || state.command.is_some() {
            return Err("Crèche cannot acknowledge an inactive or occupied encounter".into());
        }
        let face = state.face.as_ref().ok_or("Crèche current Face absent")?;
        show.validate(face)
            .map_err(|e| format!("Crèche Show identity: {e:?}"))?;
        if show.show.lifecycle != ManifestationLifecycle::Available {
            return Err("Crèche requires its selected Mask's actual acknowledged Show".into());
        }
        self.channel
            .custody
            .try_lock()
            .map_err(custody_pressure)?
            .validate_acknowledged(face, &show)?;
        state.acknowledged = Some(Arc::new(show));
        Ok(())
    }
    pub fn submit(&self, interaction: FaceInteraction) -> Result<(), String> {
        let mut state = self.channel.lock()?;
        if !state.active || (state.reviewed && state.images.is_none()) || state.command.is_some() {
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
        let mut custody = self.channel.custody.try_lock().map_err(custody_pressure)?;
        custody.validate_acknowledged(face, show)?;
        custody.validate_returned_interaction(face, show, &interaction)?;
        drop(custody);
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
            state.handoff_request = handoff.as_ref().map(|handoff| handoff.request);
        }
        Ok(handoff)
    }
    /// Called by the concrete make owner only after it has taken this exact
    /// canonical review. There is intentionally no successful-artifact setter.
    pub fn classify_make_refusal(&self, reason: CrecheMakeRefusal) -> Result<(), String> {
        let mut state = self.channel.lock()?;
        if !state.active
            || !state.handoff_taken
            || state.make_refusal.is_some()
            || state.products.is_some()
            || state.images.is_some()
        {
            return Err("Crèche make refusal has no single admitted reviewed operation".into());
        }
        state.make_refusal = Some(reason);
        Ok(())
    }

    /// Accept only the opaque result of actual image production and final byte
    /// revalidation for this one reviewed request. No raw successful outcome or
    /// externally populated receipt can pass this boundary.
    pub fn complete_images(
        &self,
        mut products: crate::creche_production::CrecheProducedImages,
    ) -> Result<(), (String, Box<crate::creche_production::CrecheProducedImages>)> {
        // This make-worker entrance may read files; unlike poll it must not be
        // called on the kernel thread. Recheck bytes after external custody.
        if let Err(error) = products.revalidate() {
            return Err((
                format!("Crèche image custody: {error:?}"),
                Box::new(products),
            ));
        }
        let mut state = match self.channel.lock() {
            Ok(state) => state,
            Err(error) => return Err((error, Box::new(products))),
        };
        if !state.active
            || !state.handoff_taken
            || state.handoff_request != Some(products.request())
            || state.products.is_some()
            || state.images.is_some()
            || state.make_refusal.is_some()
        {
            return Err((
                "Crèche images do not belong to this active single review".into(),
                Box::new(products),
            ));
        }
        if let Err(error) = images_runtime::install(&mut state, &products) {
            return Err((error, Box::new(products)));
        }
        state.products = Some(products);
        Ok(())
    }

    /// Move prepared files to the owning save/deploy encounter only after the
    /// installed operation has published its exact Prepared receipt digest.
    pub fn take_products(
        &self,
    ) -> Result<Option<crate::creche_production::CrecheProducedImages>, String> {
        let mut state = self.channel.lock()?;
        Ok(if state.products_completed {
            state.products.take()
        } else {
            None
        })
    }
}

// Native retirement may hold this guard while the UI thread drains its actual
// output. The UI must observe pressure rather than block that drain handshake.
fn custody_pressure<T>(error: TryLockError<T>) -> &'static str {
    match error {
        TryLockError::WouldBlock => "Crèche selected Mask custody is busy",
        TryLockError::Poisoned(_) => "Crèche Mask custody poisoned",
    }
}
