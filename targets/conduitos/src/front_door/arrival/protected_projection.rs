//! Root validates display admission; guest owns native layout and Face projection.
use super::{Arrival, Error, FrontDoor};
use crate::display::PixelTarget;
use conduit_face::{Face, GraphicsScene};
use conduitos_creche_core::data::NativeScope;
impl Arrival {
    fn native_scope(&self, door: &FrontDoor) -> NativeScope {
        NativeScope {
            revision: self.draft.revision(),
            outer_revision: door.revision,
            host_id: door.host_id.clone(),
            boot_id: door.boot_id.clone(),
        }
    }
    pub(in crate::front_door) fn face(&self, door: &FrontDoor) -> Result<Face, Error> {
        self.draft
            .native_face(self.native_scope(door))
            .map_err(|_| Error::Face)
    }
    pub(in crate::front_door) fn scene(
        &self,
        display: &impl PixelTarget,
        door: &FrontDoor,
    ) -> Result<GraphicsScene, Error> {
        let format = display.format().validate().map_err(Error::Display)?;
        let width = u16::try_from(format.width).map_err(|_| Error::Scene)?;
        let height = u16::try_from(format.height).map_err(|_| Error::Scene)?;
        if width < 480 || height < 480 {
            return Err(Error::Scene);
        }
        self.draft
            .native_scene(self.native_scope(door), width, height)
            .map_err(|_| Error::Scene)
    }
}
