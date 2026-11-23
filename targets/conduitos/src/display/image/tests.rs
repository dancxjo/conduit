use super::*;

struct Surface {
    pixels: [u32; 12],
}
impl PixelTarget for Surface {
    fn format(&self) -> DisplayFormat {
        DisplayFormat {
            width: 4,
            height: 3,
            pitch: 16,
            bits_per_pixel: 32,
            red_shift: 16,
            green_shift: 8,
            blue_shift: 0,
        }
    }
    fn write_pixel(&mut self, x: u32, y: u32, pixel: u32) -> Result<(), DisplayError> {
        if x >= 4 || y >= 3 {
            return Err(DisplayError::InvalidExtent);
        }
        self.pixels[(y * 4 + x) as usize] = pixel;
        Ok(())
    }
}
struct Gallery {
    key: GraphicsImageKey,
    pixels: [u8; 16],
    denied: bool,
}
impl ImageResolver for Gallery {
    fn resolve(&self, key: GraphicsImageKey) -> Result<OpaqueRgbaImage<'_>, DisplayError> {
        if self.denied {
            return Err(DisplayError::ImageAccessDenied);
        }
        if key.identity() == self.key.identity() && key.version() != self.key.version() {
            return Err(DisplayError::ImageStale);
        }
        if key != self.key {
            return Err(DisplayError::ImageUnavailable);
        }
        OpaqueRgbaImage::prepare(2, 2, &self.pixels)
    }
}
fn gallery() -> Gallery {
    Gallery {
        key: GraphicsImageKey::new([1; 32], [2; 32]).unwrap(),
        pixels: [
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
        ],
        denied: false,
    }
}
fn scene(key: GraphicsImageKey, bounds: LayoutRect, clip: LayoutRect) -> GraphicsScene {
    let mut scene = GraphicsScene::empty();
    scene
        .push(GraphicsCommand::image(bounds, clip, key).unwrap())
        .unwrap();
    scene
}

#[test]
fn admitted_generation_scales_and_clips_without_touching_other_pixels() {
    let gallery = gallery();
    let bounds = LayoutRect {
        x: 0,
        y: 0,
        width: 4,
        height: 2,
    };
    let clip = LayoutRect {
        x: 1,
        y: 0,
        width: 2,
        height: 2,
    };
    let scene = scene(gallery.key, bounds, clip);
    let mut target = Surface {
        pixels: [0x123456; 12],
    };
    let receipt = render_scene_with_images(&mut target, &scene, &gallery).unwrap();
    assert_eq!(
        receipt,
        DisplayReceipt {
            commands: 1,
            pixels_written: 4
        }
    );
    assert_eq!(
        target.pixels,
        [
            0x123456, 0xff0000, 0x00ff00, 0x123456, 0x123456, 0xff0000, 0x00ff00, 0x123456,
            0x123456, 0x123456, 0x123456, 0x123456
        ]
    );
}

#[test]
fn full_viewport_center_cover_preserves_pixels_on_both_crop_axes() {
    let viewport = LayoutRect {
        x: 0,
        y: 0,
        width: 4,
        height: 3,
    };
    let key = gallery().key;
    let commands = scene(key, viewport, viewport);
    let command = &commands.commands()[0];
    for (width, height, expected) in [
        (8, 3, [2, 3, 4, 5, 10, 11, 12, 13, 18, 19, 20, 21]),
        (4, 7, [8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19]),
    ] {
        // Each source pixel has its own value. Stretching the frame or
        // cropping from the origin would yield a different exact raster.
        let rgba: alloc::vec::Vec<_> = (0..width * height)
            .flat_map(|i| [0, 0, i as u8, 255])
            .collect();
        let image = OpaqueRgbaImage::prepare(width, height, &rgba).unwrap();
        let mut target = Surface {
            pixels: [u32::MAX; 12],
        };
        let mut receipt = DisplayReceipt::default();
        raster(&mut target, command, image, &mut receipt).unwrap();
        assert_eq!(receipt.pixels_written, 12);
        assert_eq!(target.pixels, expected);
    }
}

#[test]
fn absent_changed_and_denied_generations_never_render() {
    let mut gallery = gallery();
    let bounds = LayoutRect {
        x: 0,
        y: 0,
        width: 2,
        height: 2,
    };
    let current = scene(gallery.key, bounds, bounds);
    let stale = scene(
        GraphicsImageKey::new([1; 32], [3; 32]).unwrap(),
        bounds,
        bounds,
    );
    let mut target = Surface {
        pixels: [0x123456; 12],
    };
    assert_eq!(
        render_scene(&mut target, &current),
        Err(DisplayError::ImageUnsupported)
    );
    assert_eq!(
        render_scene_with_images(&mut target, &stale, &gallery),
        Err(DisplayError::ImageStale)
    );
    let absent = scene(
        GraphicsImageKey::new([9; 32], [2; 32]).unwrap(),
        bounds,
        bounds,
    );
    assert_eq!(
        render_scene_with_images(&mut target, &absent, &gallery),
        Err(DisplayError::ImageUnavailable)
    );
    gallery.denied = true;
    assert_eq!(
        render_scene_with_images(&mut target, &current, &gallery),
        Err(DisplayError::ImageAccessDenied)
    );
    assert_eq!(target.pixels, [0x123456; 12]);
}

#[test]
fn malformed_or_transparent_backing_refuses_during_preparation() {
    assert!(matches!(
        OpaqueRgbaImage::prepare(0, 2, &[]),
        Err(DisplayError::InvalidExtent)
    ));
    assert!(matches!(
        OpaqueRgbaImage::prepare(2, 2, &[255; 12]),
        Err(DisplayError::InvalidExtent)
    ));
    assert!(matches!(
        OpaqueRgbaImage::prepare(1, 1, &[1, 2, 3, 0]),
        Err(DisplayError::ImageUnsupported)
    ));
}

impl RetainedPixelTarget for Surface {
    fn read_pixel(&self, x: u32, y: u32) -> Result<u32, DisplayError> {
        if x >= 4 || y >= 3 {
            return Err(DisplayError::InvalidExtent);
        }
        Ok(self.pixels[(y * 4 + x) as usize])
    }
}
#[test]
#[cfg(feature = "native-compositor-core")]
fn selected_typography_path_renders_only_independently_resolved_images() {
    use crate::display::typography::{
        Typography, render_scene_selected, render_scene_selected_with_images,
    };
    let gallery = gallery();
    let bounds = LayoutRect {
        x: 0,
        y: 0,
        width: 4,
        height: 3,
    };
    let scene = scene(gallery.key, bounds, bounds);
    let mut target = Surface { pixels: [0; 12] };
    assert_eq!(
        render_scene_selected(
            &mut target,
            &scene,
            |_, c| c.text_role().into(),
            100,
            Typography::EDITORIAL,
            |_| 0
        ),
        Err(DisplayError::ImageUnsupported)
    );
    assert_eq!(target.pixels, [0; 12]);
    let receipt = render_scene_selected_with_images(
        &mut target,
        &scene,
        |_, c| c.text_role().into(),
        100,
        Typography::EDITORIAL,
        |_| 0,
        &gallery,
    )
    .unwrap();
    assert_eq!(receipt.commands, 1);
    assert_eq!(receipt.pixels_written, 12);
    assert!(target.pixels.iter().all(|p| *p != 0));
}
