//! Compatibility facade over the shared finite display paint resolver.
#[cfg(feature = "native-face-raster")]
pub(super) use crate::display::look_paint::visual_paint;
#[cfg(feature = "native-face-raster")]
pub use crate::display::visual_look_paint;
