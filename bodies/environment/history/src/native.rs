//! Normal finite GraphicsScene adapter. The caller owns checked Mask/Show admission.
use crate::graph::{ChartGeometry, HistoryGraph};
use conduit_face::{GraphicsError, GraphicsScene};
/// The selected graphical Host measures bounded text with the same resources
/// used to paint it. Called only during scene preparation, never per pixel.
pub type NativeHistoryTextMeasure<'a> =
    dyn Fn(&str, conduit_face::GraphicsTextRole, u16) -> Result<u16, GraphicsError> + 'a;
/// Correlation travels beside the native graphical payload, never in renderer state.
pub struct NativeHistoryShow<'a> {
    pub content_id: &'a str,
    pub revision: u64,
    pub scene: GraphicsScene,
}
impl<'a> HistoryGraph<'a> {
    pub fn native_show(&self) -> Result<NativeHistoryShow<'a>, GraphicsError> {
        self.native_show_in_unit(conduit_core::Unit::Celsius)
    }
    pub fn native_show_in_unit(
        &self,
        unit: conduit_core::Unit,
    ) -> Result<NativeHistoryShow<'a>, GraphicsError> {
        self.native_show_in_unit_at_size(unit, 640, 360)
    }
    /// Prepare finite geometry and typography bounds at native pixel dimensions.
    pub fn native_show_in_unit_at_size(
        &self,
        unit: conduit_core::Unit,
        width: u16,
        height: u16,
    ) -> Result<NativeHistoryShow<'a>, GraphicsError> {
        Ok(NativeHistoryShow {
            content_id: self.content_id,
            revision: self.revision,
            scene: self.native_scene_in_unit_at_size(unit, width, height)?,
        })
    }
    pub fn native_scene(&self) -> Result<GraphicsScene, GraphicsError> {
        self.native_scene_in_unit(conduit_core::Unit::Celsius)
    }
    pub fn native_scene_in_unit(
        &self,
        unit: conduit_core::Unit,
    ) -> Result<GraphicsScene, GraphicsError> {
        self.native_scene_in_unit_at_size(unit, 640, 360)
    }
    /// Prepare a chart using the selected Look's actual text layout. Required
    /// labels reserve space before graph geometry is published or refuse intact.
    pub fn native_scene_in_unit_at_size_measured(
        &self,
        unit: conduit_core::Unit,
        width: u16,
        height: u16,
        measure: &NativeHistoryTextMeasure<'_>,
    ) -> Result<GraphicsScene, GraphicsError> {
        ChartGeometry {
            samples: &self.samples,
        }
        .native_scene_at_size_measured(
            self.omitted,
            "RECORDED HISTORY",
            unit,
            (width, height),
            Some(measure),
        )
    }
    /// Nominal text boxes; selected Looks use the measured entrypoint above.
    /// Refuses viewports smaller than 320×360 or outside signed coordinate bounds.
    pub fn native_scene_in_unit_at_size(
        &self,
        unit: conduit_core::Unit,
        width: u16,
        height: u16,
    ) -> Result<GraphicsScene, GraphicsError> {
        ChartGeometry {
            samples: &self.samples,
        }
        .native_scene_at_size(self.omitted, "RECORDED HISTORY", unit, width, height)
    }
}
pub(crate) mod layout;
mod scene;
