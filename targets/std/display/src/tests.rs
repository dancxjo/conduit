use super::*;
use conduit_face::{GraphicsCommand, GraphicsPaintRole, LayoutRect};
fn scene() -> GraphicsScene {
    let bounds = LayoutRect {
        x: 0,
        y: 0,
        width: 32,
        height: 24,
    };
    let mut scene = GraphicsScene::empty();
    scene
        .push(
            GraphicsCommand::text(bounds, bounds, GraphicsPaintRole::Foreground, "67.6F").unwrap(),
        )
        .unwrap();
    scene
}
#[test]
fn finite_surface_uses_ordinary_graphics_and_revocation_erases_pixels() {
    for (w, h) in [(0, 24), (32, 0), (u32::MAX, 24), (4096, 4096)] {
        assert!(Surface::prepare(w, h).is_err());
    }
    let mut surface = Surface::prepare(32, 24).unwrap();
    assert_eq!(surface.bytes().len(), 32 * 24 * 4);
    let before = surface.bytes().as_ptr();
    let receipt = surface.render(&scene()).unwrap();
    assert_eq!(receipt.commands, 1);
    assert!(receipt.pixels_written > 0);
    assert!(surface.bytes().iter().any(|b| *b != 0));
    surface.render(&GraphicsScene::empty()).unwrap();
    assert!(surface
        .bytes()
        .as_chunks::<4>()
        .0
        .iter()
        .all(|p| *p == 0xff00_0000_u32.to_ne_bytes()));
    assert_eq!(surface.bytes().as_ptr(), before);
    surface.lose();
    assert_eq!(surface.render(&scene()), Err("display-custody-lost"));
    assert!(surface.bytes().iter().all(|b| *b == 0));
}
#[test]
fn actual_sdl_software_texture_accepts_scene_and_refuses_after_loss() {
    // Explicit CPU diagnostic backend; this makes no DRM/HDMI acceptance claim.
    assert!(sdl2::hint::set("SDL_VIDEODRIVER", "dummy"));
    let sdl = sdl2::init().unwrap();
    let video = sdl.video().unwrap();
    assert_eq!(video.current_video_driver(), "dummy");
    let window = video
        .window("CPU fixture", 32, 24)
        .hidden()
        .build()
        .unwrap();
    let mut canvas = window.into_canvas().software().build().unwrap();
    let textures = canvas.texture_creator();
    let mut scanout = Scanout::prepare(&mut canvas, &textures, 32, 24).unwrap();
    scanout.present(&scene()).unwrap();
    let pixels = scanout
        .canvas
        .read_pixels(None, PixelFormatEnum::ARGB8888)
        .unwrap();
    assert_eq!(pixels, scanout.surface.bytes());
    scanout.lose();
    assert_eq!(
        scanout.present(&scene()),
        Err("display-custody-lost".into())
    );
}
