//! Changing only resolved geometry must invalidate retained raster work.
use super::*;
use conduit_face::GraphicsPaintRole as Paint;
fn draw(buffer: &mut SurfaceBuffer, scene: &GraphicsScene) -> Result<DisplayReceipt, DisplayError> {
    crate::display::typography::render_scene(buffer, scene, |_, command| command.text_role().into())
}
fn scene(metrics: Option<(u8, u8)>) -> GraphicsScene {
    let extent = LayoutRect {
        x: 0,
        y: 0,
        width: 48,
        height: 40,
    };
    let bounds = LayoutRect {
        x: 7,
        y: 5,
        width: 28,
        height: 26,
    };
    let mut scene = GraphicsScene::empty();
    scene
        .push(
            GraphicsCommand::rect(extent, extent, Paint::Background, GraphicsShapeStyle::Fill)
                .unwrap(),
        )
        .unwrap();
    for (paint, style) in [
        (Paint::Surface, GraphicsShapeStyle::RoundedFill),
        (Paint::Border, GraphicsShapeStyle::RoundedStroke),
    ] {
        let mut command = GraphicsCommand::rect(bounds, extent, paint, style).unwrap();
        if let Some((radius, stroke)) = metrics {
            command = command.with_rect_metrics(radius, stroke).unwrap();
        }
        scene.push(command).unwrap();
    }
    scene
}
#[test]
fn changed_metrics_and_legacy_transitions_match_fresh_pixels_then_reuse_exactly() {
    let cases = [
        None,
        Some((0, 1)),
        Some((2, 1)),
        Some((6, 3)),
        Some((32, 8)),
    ];
    for before in cases {
        for after in cases {
            let old = scene(before);
            let new = scene(after);
            assert!(canonical(&old) && canonical(&new));
            let mut actual = SurfaceBuffer::new(48, 40).unwrap();
            raster(&mut actual, &old, None, draw).unwrap();
            let mut expected = SurfaceBuffer::new(48, 40).unwrap();
            raster(&mut expected, &new, None, draw).unwrap();
            raster(&mut actual, &new, Some(&old), draw).unwrap();
            assert_eq!(
                actual.pixels, expected.pixels,
                "before={before:?}, after={after:?}"
            );
            let reused = raster(&mut actual, &new, Some(&new), |_, _| {
                panic!("unchanged metrics must reuse raster")
            })
            .unwrap();
            assert_eq!(reused, DisplayReceipt::default());
        }
    }
}

#[test]
fn shared_union_packet_renders_and_reuses_one_native_surface() {
    let hex = include_str!("../../../../../proof/fixtures/graphics-scene-union.hex").trim();
    let packet: alloc::vec::Vec<u8> = hex
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(core::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect();
    let scene = GraphicsScene::decode(&packet).unwrap();
    let mut surface = SurfaceBuffer::new(64, 48).unwrap();
    let receipt = raster(&mut surface, &scene, None, draw).unwrap();
    assert!(receipt.pixels_written > 0);
    let before = surface.pixels.clone();
    let reused = raster(&mut surface, &scene, Some(&scene), |_, _| {
        panic!("exact union packet must reuse retained pixels")
    })
    .unwrap();
    assert_eq!(reused, DisplayReceipt::default());
    assert_eq!(surface.pixels, before);
}
