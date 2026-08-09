//! Native surface execution and typed Manifestation lifecycle correlation.

use super::{draw_document, PatchbayApplication, BACKGROUND};
use conduit_core::ClueId;
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
                        .mark_available(ClueId::from("patchbay-native/window-presented"))
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
                        ClueId::from("patchbay-native/window-rejected"),
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
        let context =
            softbuffer::Context::new(window.clone()).map_err(|error| error.to_string())?;
        let mut surface = softbuffer::Surface::new(&context, window.clone())
            .map_err(|error| error.to_string())?;
        surface
            .resize(width, height)
            .map_err(|error| error.to_string())?;
        let mut buffer = surface.buffer_mut().map_err(|error| error.to_string())?;
        buffer.fill(BACKGROUND);
        let lines = self.presentation_lines();
        draw_document(
            &mut buffer,
            size.width as usize,
            size.height as usize,
            &lines,
        );
        buffer.present().map_err(|error| error.to_string())?;
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
