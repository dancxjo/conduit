//! Move bounded local state into a prepared replacement, never Show authority.
use super::*;
impl NativeFaceMask {
    pub fn preserve_encounter_from(
        &mut self,
        previous: &mut Self,
    ) -> Result<(), NativeFaceMaskError> {
        if self.producer.plan().plan_id != previous.producer.plan().plan_id {
            return Err(NativeFaceMaskError::Plan);
        }
        if self.session.is_some() || self.scene.is_some() || self.encounter_pending {
            return Err(NativeFaceMaskError::Face);
        }
        self.scene = Some(previous.scene.take().ok_or(NativeFaceMaskError::Face)?);
        self.encounter_pending = true;
        Ok(())
    }

    pub(super) fn restore_pending_encounter(
        &mut self,
        scene: &mut NativeFaceScene,
    ) -> Result<(), NativeFaceMaskError> {
        if core::mem::replace(&mut self.encounter_pending, false) {
            scene
                .retain_encounter(self.scene.take().ok_or(NativeFaceMaskError::Face)?)
                .map_err(NativeFaceMaskError::Scene)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selection_state_refuses_a_different_producer_host_before_moving_any_scene() {
        let provider = crate::product_bases::fixture_surface_provider();
        let prepare = |host: &str| {
            NativeFaceMask::prepare(
                host.into(),
                "boot".into(),
                OfferGeneration(1),
                "build",
                provider.entry.base_id.clone(),
                "surface",
                &provider,
            )
            .unwrap()
        };
        let mut previous = prepare("host/old");
        let mut fresh = prepare("host/new");
        assert_eq!(
            fresh.preserve_encounter_from(&mut previous),
            Err(NativeFaceMaskError::Plan)
        );
        assert!(!fresh.encounter_pending);
        assert!(fresh.scene.is_none());
        assert!(fresh.show().is_none());
    }
}
