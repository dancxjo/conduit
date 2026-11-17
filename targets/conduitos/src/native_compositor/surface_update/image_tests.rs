use super::*;
use crate::display::{ImageResolver, OpaqueRgbaImage};
use conduit_face::{GraphicsCommand, GraphicsImageKey};
use core::cell::Cell;

struct Images {
    denied: Cell<bool>,
}
impl ImageResolver for Images {
    fn resolve(&self, key: GraphicsImageKey) -> Result<OpaqueRgbaImage<'_>, DisplayError> {
        if self.denied.get() {
            return Err(DisplayError::ImageAccessDenied);
        }
        if key != GraphicsImageKey::new([1; 32], [2; 32]).unwrap() {
            return Err(DisplayError::ImageUnavailable);
        }
        static PIXELS: [u8; 16] = [
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
        ];
        OpaqueRgbaImage::prepare(2, 2, &PIXELS)
    }
}
#[test]
fn image_custody_is_required_before_raster_and_rechecked_for_retained_pixels() {
    let (face, show, look, mut compositor) = super::metric_consumer_tests::setup(16);
    let base = compositor.admission().display_base_id.clone();
    let extent = super::metric_consumer_tests::extent();
    let mut scene = GraphicsScene::empty();
    scene
        .push(
            GraphicsCommand::image(
                extent,
                extent,
                GraphicsImageKey::new([1; 32], [2; 32]).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
    assert!(
        compositor
            .update_pending_mask_surface(&face, &show, &base, &scene)
            .is_err()
    );
    assert_eq!(compositor.receipts().count(), 0);
    let images = Images {
        denied: Cell::new(false),
    };
    compositor
        .update_pending_visual_mask_surface(
            &face,
            &show,
            &base,
            &scene,
            Some(&look.visual),
            Some(&images),
        )
        .unwrap();
    assert_eq!(compositor.receipts().count(), 1);
    let available = show
        .clone()
        .transition(ManifestationLifecycle::Available, "sign/available".into())
        .unwrap();
    let repaint = compositor
        .repaint_mask_surface_with_images(&face, &available, &base, &scene, &images)
        .unwrap();
    assert_eq!(repaint.display.pixels_written, 0);
    // Cached pixels and an unchanged exact generation cannot extend access.
    images.denied.set(true);
    let error = compositor
        .repaint_mask_surface_with_images(&face, &available, &base, &scene, &images)
        .unwrap_err();
    assert_eq!(
        error,
        NativeCompositorError::Display(DisplayError::ImageAccessDenied)
    );
    assert_eq!(compositor.receipts().count(), 1);
}
