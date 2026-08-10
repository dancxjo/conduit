//! Renderer-local pointer gesture state for Patchbay canvas authoring.

use crate::{gui::GuiAction, PatchbayApplication};

impl PatchbayApplication {
    pub(super) fn handle_canvas_press(&mut self) -> Result<(), String> {
        let Some(action) = self
            .hit_targets
            .iter()
            .rev()
            .find(|target| target.contains(self.cursor_position.0, self.cursor_position.1))
            .map(|target| target.action.clone())
        else {
            return Ok(());
        };
        if let GuiAction::PlacePaletteKind(kind) = action {
            self.palette_drag = Some(kind);
            return Ok(());
        }
        if let GuiAction::SelectSubject(subject) = &action {
            let subject_kind = self
                .graphical_form
                .as_ref()
                .and_then(|graph| graph.inspect(&subject.subject_identity).ok())
                .map(|inspection| inspection.subject_kind);
            match subject_kind {
                Some(patchbay_model::PatchbaySubjectKind::PortOutput) => {
                    self.cord_drag = Some(subject.clone());
                }
                Some(patchbay_model::PatchbaySubjectKind::Gear) => {
                    self.gear_drag = Some((subject.clone(), self.cursor_position));
                }
                Some(patchbay_model::PatchbaySubjectKind::Cord) => {
                    self.cord_route_drag = Some(subject.clone());
                }
                _ => {}
            }
        }
        self.handle_gui_action(action)
    }

    pub(super) fn handle_canvas_release(&mut self) -> Result<(), String> {
        if let Some(kind) = self.palette_drag.take() {
            if self.cursor_position.0 > 176.0 {
                self.handle_gui_action(GuiAction::PlacePaletteKind(kind))?;
            }
            return Ok(());
        }
        if let Some(source) = self.cord_drag.take() {
            let sink = self
                .hit_targets
                .iter()
                .rev()
                .find(|target| target.contains(self.cursor_position.0, self.cursor_position.1))
                .and_then(|target| match &target.action {
                    GuiAction::SelectSubject(subject) => Some(subject.clone()),
                    _ => None,
                })
                .filter(|subject| {
                    self.graphical_form
                        .as_ref()
                        .and_then(|graph| graph.inspect(&subject.subject_identity).ok())
                        .is_some_and(|inspection| {
                            inspection.subject_kind
                                == patchbay_model::PatchbaySubjectKind::PortInput
                        })
                });
            if let Some(sink) = sink {
                self.handle_gui_action(GuiAction::ConnectPorts { source, sink })?;
            }
            return Ok(());
        }
        if let Some(cord) = self.cord_route_drag.take() {
            let endpoint = self
                .hit_targets
                .iter()
                .rev()
                .find(|target| target.contains(self.cursor_position.0, self.cursor_position.1))
                .and_then(|target| match &target.action {
                    GuiAction::SelectSubject(subject) => Some(subject.clone()),
                    _ => None,
                })
                .filter(|subject| {
                    self.graphical_form
                        .as_ref()
                        .and_then(|graph| graph.inspect(&subject.subject_identity).ok())
                        .is_some_and(|inspection| {
                            inspection.subject_kind
                                == patchbay_model::PatchbaySubjectKind::PortInput
                                || inspection.subject_kind
                                    == patchbay_model::PatchbaySubjectKind::PortOutput
                        })
                });
            if let Some(endpoint) = endpoint {
                self.handle_gui_action(GuiAction::RerouteCord { cord, endpoint })?;
            } else {
                let graph = self
                    .graphical_form
                    .as_ref()
                    .ok_or("graphical Form projection is absent")?;
                self.layout
                    .route_cord(
                        graph,
                        &cord,
                        self.cursor_position.0 as i32,
                        self.cursor_position.1 as i32,
                    )
                    .map_err(|error| format!("native Cord routing failed: {error:?}"))?;
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            return Ok(());
        }
        if let Some((gear, start)) = self.gear_drag.take() {
            let moved = (self.cursor_position.0 - start.0).abs() > 2.0
                || (self.cursor_position.1 - start.1).abs() > 2.0;
            if moved {
                let position = (
                    (self.cursor_position.0 as i32 - 95).max(177),
                    (self.cursor_position.1 as i32 - 20).max(53),
                );
                let graph = self
                    .graphical_form
                    .as_ref()
                    .ok_or("graphical Form projection is absent")?;
                self.layout
                    .move_gear(graph, &gear, position.0, position.1)
                    .map_err(|error| format!("native Gear movement failed: {error:?}"))?;
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
        }
        Ok(())
    }
}
