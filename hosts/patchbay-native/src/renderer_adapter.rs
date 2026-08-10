//! Native surface execution and typed Manifestation lifecycle correlation.

use super::{
    draw_document,
    gui::{draw_patchbay, LifecycleContext},
    PatchbayApplication, BACKGROUND,
};
use conduit_core::SignId;
use conduit_presentation::{ManifestationFailure, ManifestationLifecycle};
use std::num::NonZeroU32;

impl PatchbayApplication {
    pub(super) fn render(&mut self) -> Result<(), String> {
        match self.render_output() {
            Ok(()) => {
                if let Some(execution) = &mut self.renderer_execution {
                    let newly_available =
                        execution.manifestation.lifecycle == ManifestationLifecycle::Prepared;
                    execution
                        .mark_available(SignId::from("patchbay-native/window-presented"))
                        .map_err(|error| error.to_string())?;
                    if newly_available {
                        println!(
                            "patchbay manifestation={} renderer-plan={} renderer-play={} lifecycle=available",
                            execution.manifestation.manifestation_id.as_str(),
                            execution.manifestation.plan_id.as_str(),
                            execution.manifestation.active_play_id.as_str()
                        );
                    }
                }
                Ok(())
            }
            Err(error) => {
                if let Some(execution) = &mut self.renderer_execution {
                    let _ = execution.mark_failed(
                        ManifestationFailure::OutputRejected,
                        SignId::from("patchbay-native/window-rejected"),
                    );
                }
                Err(error)
            }
        }
    }

    fn render_output(&mut self) -> Result<(), String> {
        let window = self.window.as_ref().ok_or("native window is absent")?;
        let size = window.inner_size();
        let width = NonZeroU32::new(size.width).ok_or("native window width is zero")?;
        let height = NonZeroU32::new(size.height).ok_or("native window height is zero")?;
        let lines = self.presentation_lines();
        let selected = self.selected_graphical_identity().map(str::to_owned);
        let graph = self.graphical_form.as_ref();
        let linear_view = self.linear_view;
        let lifecycle = LifecycleContext {
            body_id: self
                .build_birth
                .body()
                .map(|body| body.body_id.as_str().to_owned()),
            wake_id: self
                .build_birth
                .wake_value()
                .map(|wake| wake.wake_id.as_str().to_owned()),
            plan_id: self
                .control
                .plan()
                .map(|plan| plan.plan_id.as_str().to_owned()),
            play_id: self.build_birth.wake_value().and_then(|wake| {
                wake.plans
                    .iter()
                    .rev()
                    .find_map(|plan| plan.active_play_id.as_ref())
                    .map(|play| play.as_str().to_owned())
            }),
        };
        let surface = self.surface.as_mut().ok_or("native surface is absent")?;
        surface
            .resize(width, height)
            .map_err(|error| error.to_string())?;
        let mut buffer = surface.buffer_mut().map_err(|error| error.to_string())?;
        buffer.fill(BACKGROUND);
        let hit_targets = if let Some(graph) = graph {
            if linear_view {
                draw_document(
                    &mut buffer,
                    size.width as usize,
                    size.height as usize,
                    &lines,
                );
                Vec::new()
            } else {
                draw_patchbay(
                    &mut buffer,
                    size.width as usize,
                    size.height as usize,
                    graph,
                    selected.as_deref(),
                    &lifecycle,
                    &self.palette_query,
                )
            }
        } else {
            draw_document(
                &mut buffer,
                size.width as usize,
                size.height as usize,
                &lines,
            );
            Vec::new()
        };
        buffer.present().map_err(|error| error.to_string())?;
        self.hit_targets = hit_targets;
        println!(
            "patchbay topology-rendered lines={} width={} height={}",
            lines.len(),
            size.width,
            size.height
        );
        if self
            .distributed_play
            .as_ref()
            .is_some_and(super::NativeDistributedPlay::is_complete)
        {
            println!("patchbay distributed-rendered status=completed");
        }
        self.rendered_once = true;
        Ok(())
    }
}
