//! Ordinary Presentation-to-Manifestation updates for retained shell surfaces.

use alloc::format;

use conduit_core::{SignId, bind_active_play};
use conduit_presentation::{
    GraphicsScene, LayoutRect, Manifestation, ManifestationLifecycle, Presentation,
};

use super::*;

impl TourShellPresenter {
    pub(super) fn present_surface(
        &mut self,
        slot: Slot,
        presentation: &Presentation,
        face_subject: &str,
        bounds: LayoutRect,
        z: u8,
        scene: &GraphicsScene,
    ) -> Result<CompositionReceipt, TourShellError> {
        let index = self
            .surfaces
            .iter()
            .position(|state| state.slot == slot)
            .ok_or(TourShellError::Identity)?;
        if self.surfaces[index]
            .face_subject
            .as_deref()
            .is_some_and(|current| current != face_subject)
        {
            self.dismiss(slot)?;
        }
        let effective = if self.surfaces[index]
            .presentation
            .as_ref()
            .is_some_and(|current| current.revision >= presentation.revision)
        {
            let revision = self.surfaces[index]
                .presentation
                .as_ref()
                .expect("matched current Presentation")
                .revision
                .checked_add(1)
                .ok_or(TourShellError::Identity)?;
            Presentation::new(
                revision,
                presentation.basis.clone(),
                presentation.subjects.clone(),
                presentation.relationships.clone(),
                presentation.properties.clone(),
                presentation.text.clone(),
            )
            .map_err(|_| TourShellError::Identity)?
        } else {
            presentation.clone()
        };
        let presentation = &effective;
        if !self.surfaces[index].admitted {
            self.compositor
                .admit_surface(slot.surface(), bounds, z)
                .map_err(TourShellError::Compositor)?;
            self.surfaces[index].admitted = true;
        } else {
            self.compositor
                .place_surface(slot.surface(), bounds, z)
                .map_err(TourShellError::Compositor)?;
        }
        self.play_sequence = self
            .play_sequence
            .checked_add(1)
            .ok_or(TourShellError::Identity)?;
        let active = bind_active_play(
            &self.plan.plan_id,
            &self.host_id,
            &self.boot_id,
            self.play_sequence,
        );
        let manifestation = Manifestation::prepared(
            presentation,
            &self.plan,
            active,
            self.surfaces[index].placement_id.clone(),
            face_subject.into(),
            slot.surface().into(),
            SignId::from(format!(
                "conduitos/shell/{}/prepared/{}",
                slot.gear(),
                self.play_sequence
            )),
        )
        .and_then(|value| {
            value.transition(
                ManifestationLifecycle::Available,
                SignId::from(format!(
                    "conduitos/shell/{}/available/{}",
                    slot.gear(),
                    self.play_sequence
                )),
            )
        })
        .map_err(|_| TourShellError::Identity)?;
        let receipt = self
            .compositor
            .update_surface(
                presentation,
                &manifestation,
                &self.plan,
                slot.surface(),
                &self.display_base_id,
                scene,
            )
            .map_err(TourShellError::Compositor)?
            .clone();
        self.surfaces[index].face_subject = Some(face_subject.into());
        self.surfaces[index].manifestation_id = Some(receipt.manifestation_id.clone());
        if matches!(slot, Slot::Inspector | Slot::Transient) {
            self.surfaces[index].scroll.configure(
                bounds.height,
                if face_subject == TourTransientKind::Chooser.subject_identity() {
                    432
                } else {
                    scene::SCROLL_CONTENT_HEIGHT
                },
            )?;
        }
        self.surfaces[index].bounds = Some(bounds);
        self.surfaces[index].presentation = Some(presentation.clone());
        Ok(receipt)
    }
}
