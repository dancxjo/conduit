//! Root supplies scoped facts and admits canonical graphics; Mask owns scene policy.
use super::{Error, FrontDoor};
use crate::display::PixelTarget;
use alloc::string::String;
use conduit_face::GraphicsScene;
impl FrontDoor {
    pub fn scene(&self, display: &impl PixelTarget) -> Result<GraphicsScene, Error> {
        self.reading()?;
        if let Some(arrival) = &self.arrival {
            return arrival.scene(display, self);
        }
        if self.home.is_some() {
            return self.home_scene(display);
        }
        let format = display.format().validate().map_err(Error::Display)?;
        let facts = self.entrance_facts(
            u16::try_from(format.width).map_err(|_| Error::Scene)?,
            u16::try_from(format.height).map_err(|_| Error::Scene)?,
        )?;
        #[cfg(all(target_arch = "x86_64", target_os = "none"))]
        let (workspace, scene) = self
            .workspace_mask
            .try_borrow_mut()
            .map_err(|_| Error::Scene)?
            .entrance_scene(facts)
            .map_err(|_| Error::Scene)?;
        #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
        let (workspace, scene) = conduitos_mask_core::entrance::scene(&facts, self.reading()?)
            .map_err(|_| Error::Scene)?;
        if !workspace {
            return Ok(scene);
        }
        #[cfg(all(target_arch = "x86_64", target_os = "none"))]
        {
            self.workspace_mask_scene(display)
        }
        #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
        {
            super::workspace_scene::scene(
                self.journey.as_ref().ok_or(Error::Scene)?,
                self.workspace
                    .as_ref()
                    .and_then(|workspace| workspace.plots.iter().find(|plot| plot.foreground))
                    .map(|plot| plot.title)
                    .ok_or(Error::Scene)?,
                self.application_view.as_ref(),
                self.application_viewport,
                self.patchbay_graph.is_some(),
                self.refusal.as_ref().map(|r| (r.heading(), r.reason())),
                display,
            )
        }
    }
    pub fn current_detail(&self) -> (&'static str, String) {
        self.try_current_detail()
            .unwrap_or_else(|_| ("DETAILS UNAVAILABLE", String::new()))
    }
    fn try_current_detail(&self) -> Result<(&'static str, String), Error> {
        // No display extent is claimed by this read-only telemetry query.
        let facts = self.entrance_facts(0, 0)?;
        #[cfg(all(target_arch = "x86_64", target_os = "none"))]
        let (kind, value) = self
            .workspace_mask
            .try_borrow_mut()
            .map_err(|_| Error::Scene)?
            .entrance_detail(facts)
            .map_err(|_| Error::Scene)?;
        #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
        let (kind, value) = conduitos_mask_core::entrance::current_detail(&facts, self.reading()?);
        Ok((kind.label(), value))
    }
}
