//! Root supplies bounded observed facts; guest owns Home layout and local Face content.
use super::{Error, FrontDoor};
use alloc::{format, vec::Vec};
use conduit_face::{FaceProperty, FaceText, GraphicsScene};
use conduitos_creche_core::{
    data::NativeScope,
    home::presentation::{Extent, Plot, Request},
};
impl FrontDoor {
    fn home_presentation(&self, extent: Option<Extent>) -> Result<Request, Error> {
        let home = self.home.as_ref().ok_or(Error::Face)?;
        if !home.is_live() {
            return Err(Error::Face);
        }
        let journey = self.journey.as_ref().ok_or(Error::Face)?;
        if journey.host_id != self.host_id || journey.boot_id != self.boot_id {
            return Err(Error::StaleInput);
        }
        Ok(Request {
            scope: NativeScope {
                revision: home.presentation_revision(),
                outer_revision: self.revision,
                host_id: self.host_id.clone(),
                boot_id: self.boot_id.clone(),
            },
            plots: self
                .home_installed_plots()
                .iter()
                .map(|plot| Plot {
                    title: plot.title.into(),
                    plot: plot.plot.clone(),
                })
                .collect(),
            friendly_name: journey.friendly_name.clone(),
            wake_id: journey.wake_id.as_ref().map(|id| id.as_str().into()),
            plan_id: journey.plan_id.as_ref().map(|id| id.as_str().into()),
            play_id: journey.active_play_id.as_ref().map(|id| id.as_str().into()),
            line_count: u8::from(self.connectivity.is_some()),
            // This is the separate Root lifecycle-refusal overlay. Home's own
            // refusal stays in the guest controller and is never supplied here.
            root_refusal: self
                .refusal
                .as_ref()
                .map(|refusal| format!("{} {}", refusal.heading(), refusal.reason())),
            extent,
        })
    }
    pub(in crate::front_door) fn home_scene(
        &self,
        display: &impl crate::display::PixelTarget,
    ) -> Result<GraphicsScene, Error> {
        let format = display.format().validate().map_err(Error::Display)?;
        let extent = Extent {
            width: u16::try_from(format.width).map_err(|_| Error::Scene)?,
            height: u16::try_from(format.height).map_err(|_| Error::Scene)?,
        };
        let request = self.home_presentation(Some(extent))?;
        self.home
            .as_ref()
            .ok_or(Error::Scene)?
            .scene(&request)
            .map_err(|_| Error::Scene)
    }
    pub(in crate::front_door) fn home_face_content(
        &self,
    ) -> Result<(Vec<FaceProperty>, Vec<FaceText>), Error> {
        let Some(home) = self.home.as_ref() else {
            return Ok((Vec::new(), Vec::new()));
        };
        let request = self.home_presentation(None)?;
        home.face_content(&request).map_err(|_| Error::Face)
    }
}
