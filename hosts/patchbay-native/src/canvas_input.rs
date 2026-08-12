//! Renderer-local pointer gesture state for Patchbay canvas authoring.

use crate::{
    gui::GuiAction,
    interaction_status::{InteractionStatusCode, InteractionStatusLevel},
    PatchbayApplication,
};

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
        if self.environment.is_some() && (self.prewake.is_none() || self.prewake_environment_view) {
            if let GuiAction::EnvironmentSelect(part_id) = &action {
                self.environment_drag = Some((part_id.clone(), self.cursor_position));
                self.publish_gesture(format!("Moving environment part {part_id}"));
            }
            if matches!(
                action,
                GuiAction::PrewakeToggleWorkspace
                    | GuiAction::PrewakeToggleHold
                    | GuiAction::PrewakeRelease
                    | GuiAction::PrewakeExit
                    | GuiAction::PrewakeNextImplementation(_)
            ) {
                return self.handle_prewake_action(action);
            }
            return self.handle_environment_action(action);
        }
        if let GuiAction::BeginPaletteDrag(kind) = action {
            if let Err(error) = self.palette.select_kind(&kind) {
                self.publish_refusal(error.message());
                return Ok(());
            }
            self.publish_gesture(format!(
                "Dragging Gear {kind}; release on the canvas to place it"
            ));
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
                Some(
                    patchbay_model::PatchbaySubjectKind::PortOutput
                    | patchbay_model::PatchbaySubjectKind::FaceInput,
                ) => {
                    self.cord_drag = Some(subject.clone());
                    let candidates = self
                        .graphical_form
                        .as_ref()
                        .map(|graph| graph.connection_candidates(&subject.subject_identity))
                        .unwrap_or_default();
                    let compatible = candidates
                        .iter()
                        .filter(|candidate| {
                            candidate.compatibility
                                == patchbay_model::PatchbayPortCompatibility::Compatible
                        })
                        .count();
                    self.publish_gesture(format!(
                        "Connecting from {}; {compatible} compatible exact Port target(s), {} incompatible or occupied",
                        subject.subject_identity,
                        candidates.len().saturating_sub(compatible)
                    ));
                }
                Some(patchbay_model::PatchbaySubjectKind::Composition) => {
                    let now = std::time::Instant::now();
                    let double_click =
                        self.last_gear_click
                            .as_ref()
                            .is_some_and(|(prior, instant)| {
                                prior == subject
                                    && now.duration_since(*instant)
                                        <= std::time::Duration::from_millis(500)
                            });
                    self.last_gear_click = Some((subject.clone(), now));
                    if double_click {
                        self.last_gear_click = None;
                        self.handle_gui_action(action)?;
                        return self.handle_gui_action(GuiAction::OpenBack);
                    }
                }
                Some(patchbay_model::PatchbaySubjectKind::Gear) => {
                    let now = std::time::Instant::now();
                    let double_click =
                        self.last_gear_click
                            .as_ref()
                            .is_some_and(|(prior, instant)| {
                                prior == subject
                                    && now.duration_since(*instant)
                                        <= std::time::Duration::from_millis(500)
                            });
                    self.last_gear_click = Some((subject.clone(), now));
                    if double_click {
                        self.last_gear_click = None;
                        self.gear_drag = None;
                        self.handle_gui_action(action)?;
                        return self.handle_gui_action(GuiAction::OpenBack);
                    }
                    self.gear_drag = Some((subject.clone(), self.cursor_position));
                    self.publish_gesture(format!(
                        "Moving Gear {}; release to place or click to select",
                        subject.subject_identity
                    ));
                }
                Some(patchbay_model::PatchbaySubjectKind::Cord) => {
                    self.cord_route_drag = Some(subject.clone());
                    self.publish_gesture(format!(
                        "Routing Cord {}; release on a Port to reroute its endpoint",
                        subject.subject_identity
                    ));
                }
                _ => {}
            }
        }
        self.handle_gui_action(action)
    }

    pub(super) fn handle_canvas_release(&mut self) -> Result<(), String> {
        if let Some((part_id, start)) = self.environment_drag.take() {
            if (self.cursor_position.0 - start.0).abs() > 2.0
                || (self.cursor_position.1 - start.1).abs() > 2.0
            {
                self.environment
                    .as_mut()
                    .ok_or("authored environment is absent")?
                    .move_part(
                        &part_id,
                        self.cursor_position.0 as i32 - 90,
                        self.cursor_position.1 as i32 - 34,
                    )
                    .map_err(|error| format!("environment movement: {error:?}"))?;
                self.refresh_prewake()?;
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
                self.publish_completed(format!("Moved environment part {part_id}"));
            } else {
                self.publish_cancelled("Environment move cancelled");
            }
            return Ok(());
        }
        if let Some(kind) = self.palette_drag.take() {
            match crate::palette_state::PaletteChooser::pointer_target(
                self.cursor_position.0,
                self.cursor_position.1,
            ) {
                Ok(target) => {
                    self.handle_gui_action(GuiAction::PlacePaletteKind { kind, target })?
                }
                Err(error) => self.publish_refusal(error.message()),
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
                                || inspection.subject_kind
                                    == patchbay_model::PatchbaySubjectKind::FaceOutput
                        })
                });
            if let Some(sink) = sink {
                self.handle_gui_action(GuiAction::ConnectPorts { source, sink })?;
            } else {
                self.publish_cancelled("Cord connection cancelled: release on a compatible Port");
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
                self.publish_completed("Updated Cord presentation route");
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
                self.publish_completed(format!("Moved Gear {}", gear.subject_identity));
            } else {
                self.publish_cancelled("Gear move cancelled; selection retained");
            }
        }
        Ok(())
    }

    pub(super) fn cancel_transient_gestures(&mut self, reason: &str) {
        let had_gesture = self.environment_drag.take().is_some()
            | self.palette_drag.take().is_some()
            | self.cord_drag.take().is_some()
            | self.cord_route_drag.take().is_some()
            | self.gear_drag.take().is_some();
        self.last_gear_click = None;
        if had_gesture {
            self.publish_cancelled(format!("Gesture cancelled: {reason}"));
        }
    }

    fn publish_gesture(&mut self, message: impl Into<String>) {
        self.interaction_status.publish(
            InteractionStatusLevel::Information,
            InteractionStatusCode::Gesture,
            message,
        );
        self.request_redraw();
    }

    fn publish_cancelled(&mut self, message: impl Into<String>) {
        self.interaction_status.publish(
            InteractionStatusLevel::Information,
            InteractionStatusCode::Cancelled,
            message,
        );
        self.request_redraw();
    }

    pub(super) fn publish_completed(&mut self, message: impl Into<String>) {
        self.interaction_status.publish(
            InteractionStatusLevel::Success,
            InteractionStatusCode::Completed,
            message,
        );
        self.request_redraw();
    }

    fn request_redraw(&self) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}
