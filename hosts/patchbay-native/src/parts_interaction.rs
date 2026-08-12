//! Read-only native Parts mode over canonical Body membership truth.

use crate::{gui::GuiAction, PatchbayApplication};
use patchbay_model::PartsView;

impl PatchbayApplication {
    pub(super) fn parts_projection(&self) -> Result<Option<PartsView>, String> {
        if !self.parts_open {
            return Ok(None);
        }
        let body = self
            .build_birth
            .body()
            .ok_or("Parts view requires a born Body")?;
        let membership = self
            .build_birth
            .membership()
            .ok_or("Parts view requires Body membership truth")?;
        let candidates = self
            .body_candidates
            .as_ref()
            .ok_or("Parts view requires candidate inventory truth")?;
        let here = membership
            .parts
            .first()
            .map(|part| &part.part_id)
            .ok_or("Parts view requires the explicit Here Part")?;
        let play = self
            .control
            .is_running()
            .then(|| self.control.planned_play_identity())
            .flatten();
        PartsView::project(
            body,
            membership,
            candidates,
            here,
            self.control.plan(),
            play.as_ref(),
            self.build_birth.wake_value().is_some(),
        )
        .map(Some)
        .map_err(|error| format!("Parts projection: {error:?}"))
    }

    pub(super) fn handle_parts_action(&mut self, action: GuiAction) -> Result<(), String> {
        match action {
            GuiAction::TogglePartsView => {
                if self.build_birth.body().is_none() {
                    return Err("Birth a Body before opening Parts".into());
                }
                self.parts_open = !self.parts_open;
                if !self.parts_open {
                    self.selected_part = None;
                    self.selected_candidate = None;
                }
            }
            GuiAction::InspectPart(part_id) => {
                let view = self.parts_projection()?.ok_or("Parts view is not open")?;
                if !view.parts.iter().any(|row| row.details.part_id == part_id) {
                    return Err("selected Part is not in the current Body projection".into());
                }
                self.selected_part = Some(part_id);
                self.selected_candidate = None;
            }
            GuiAction::InspectCandidate(candidate_id) => {
                let view = self.parts_projection()?.ok_or("Parts view is not open")?;
                if !view
                    .wants_to_join
                    .iter()
                    .any(|row| row.candidate_id == candidate_id)
                {
                    return Err("selected candidate is not in the current Body projection".into());
                }
                self.selected_candidate = Some(candidate_id);
                self.selected_part = None;
            }
            _ => return Err("action does not belong to Parts".into()),
        }
        if let Some(window) = &self.window {
            window.request_redraw();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arguments::Arguments;
    use patchbay_model::PatchbayAction;

    #[test]
    fn parts_mode_projects_here_and_selection_without_mutating_body_truth() {
        let directory =
            std::env::temp_dir().join(format!("patchbay-native-parts-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("clock.conduit");
        std::fs::write(&path, include_str!("../../../examples/clock.conduit")).unwrap();
        let mut application = PatchbayApplication::new(Arguments {
            form_path: Some(path.clone()),
            ..Arguments::default()
        })
        .unwrap();

        assert!(application.parts_projection().unwrap().is_none());
        assert!(application
            .handle_parts_action(GuiAction::TogglePartsView)
            .is_err());
        application
            .handle_gui_action(GuiAction::Lifecycle(PatchbayAction::Birth))
            .unwrap();
        let before = application.build_birth.membership().unwrap().clone();
        application
            .handle_parts_action(GuiAction::TogglePartsView)
            .unwrap();
        let view = application.parts_projection().unwrap().unwrap();
        assert_eq!(view.parts.len(), 1);
        assert_eq!(view.parts[0].label, "This computer");
        assert_eq!(
            view.parts[0].state,
            patchbay_model::PartPresentationState::Here
        );
        assert!(view.parts[0].available);
        let part_id = view.parts[0].details.part_id.clone();
        let mut pixels = vec![crate::BACKGROUND; 1_100 * 720];
        let lifecycle = crate::gui::LifecycleContext {
            body_id: Some(view.body_id.as_str().into()),
            parts: Some(view.clone()),
            ..Default::default()
        };
        let targets = crate::gui::draw_patchbay(
            &mut pixels,
            1_100,
            720,
            application.graphical_form.as_ref().unwrap(),
            crate::gui::PatchbayViewContext {
                selected: None,
                breadcrumb: "",
                lifecycle: &lifecycle,
                palette: &Default::default(),
                exact_identity_open: false,
                face_control_focus: 0,
                presentation_layout: &application.layout,
                realization_plan: None,
                realization_hosts: &[],
                status: None,
                gesture: Default::default(),
                viewport: &Default::default(),
            },
        );
        assert!(targets.iter().any(
            |target| matches!(&target.action, GuiAction::InspectPart(candidate) if candidate == &part_id)
        ));
        assert!(pixels.contains(&patchbay_model::PHOSPHOR_THEME.focus.packed_rgb()));
        application
            .handle_parts_action(GuiAction::InspectPart(part_id.clone()))
            .unwrap();
        assert_eq!(application.selected_part, Some(part_id));
        assert_eq!(application.build_birth.membership(), Some(&before));

        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn f12_uses_the_same_typed_toggle_as_pointer_activation() {
        let mut application = PatchbayApplication::new(Arguments::default()).unwrap();
        assert_eq!(
            application
                .handle_parts_key(&winit::keyboard::Key::Named(winit::keyboard::NamedKey::F12)),
            Err("Birth a Body before opening Parts".into())
        );
        assert_eq!(
            application.handle_parts_key(&winit::keyboard::Key::Character("p".into())),
            Ok(false)
        );
    }
}
