use super::*;
use conduit_face::{FaceIconKey, GraphicsPaintRole as Paint};

fn rect(x: i16, y: i16, width: u16, height: u16) -> LayoutRect {
    LayoutRect {
        x,
        y,
        width,
        height,
    }
}
fn draw(buffer: &mut SurfaceBuffer, scene: &GraphicsScene) -> Result<DisplayReceipt, DisplayError> {
    #[cfg(feature = "native-compositor")]
    {
        crate::display::typography::render_scene(buffer, scene, |_, command| {
            command.text_role().into()
        })
    }
    #[cfg(not(feature = "native-compositor"))]
    {
        crate::display::render_scene(buffer, scene)
    }
}
fn scene(variant: usize) -> GraphicsScene {
    let extent = rect(0, 0, 96, 64);
    let mut scene = GraphicsScene::empty();
    if !variant.is_multiple_of(9) {
        scene
            .push(
                GraphicsCommand::rect(extent, extent, Paint::Background, GraphicsShapeStyle::Fill)
                    .unwrap(),
            )
            .unwrap();
    }
    let commands = [
        GraphicsCommand::text(
            rect(2, 3, 28, 25),
            extent,
            Paint::Foreground,
            if variant.is_multiple_of(2) {
                "Wi"
            } else {
                "ề\ni"
            },
        )
        .unwrap(),
        GraphicsCommand::rect(
            rect(20, 20, 24, 20),
            rect(25, 23, 32, 23),
            Paint::Accent,
            [
                GraphicsShapeStyle::Fill,
                GraphicsShapeStyle::Stroke,
                GraphicsShapeStyle::RoundedFill,
                GraphicsShapeStyle::RoundedStroke,
            ][variant % 4],
        )
        .unwrap(),
        GraphicsCommand::icon(
            rect(55, 6, 24, 24),
            extent,
            Paint::Warning,
            FaceIconKey::Clock,
        )
        .unwrap(),
        GraphicsCommand::rect(
            rect(-8, 40, 20, 12),
            extent,
            Paint::Success,
            GraphicsShapeStyle::Fill,
        )
        .unwrap(),
        GraphicsCommand::text(
            rect(76, 45, 20, 19),
            rect(82, 47, 12, 12),
            Paint::Accent,
            "xy",
        )
        .unwrap(),
    ];
    scene
        .push(
            GraphicsCommand::path(
                conduit_face::GraphicsPath::new(&[
                    conduit_face::GraphicsPoint::new(40, 40).unwrap(),
                    conduit_face::GraphicsPoint::new(70, 40).unwrap(),
                    conduit_face::GraphicsPoint::new(70, 60).unwrap(),
                ])
                .unwrap(),
                rect(42, 41, 28, 20),
                Paint::Muted,
            )
            .unwrap(),
        )
        .unwrap();
    for offset in 0..commands.len() {
        let index = (offset + variant / 4) % commands.len();
        if variant % 7 != index {
            scene.push(commands[index]).unwrap();
        }
    }
    scene
}
#[test]
fn incremental_pixels_match_fresh_raster_across_edits_order_clips_and_blending() {
    // Every pair starts from a real successful old raster, including deleted
    // backgrounds, text edits, command insertion/removal and z-order changes.
    for before in 0..24 {
        let old = scene(before);
        assert!(canonical(&old));
        let mut prepared = SurfaceBuffer::new(96, 64).unwrap();
        raster(&mut prepared, &old, None, draw).unwrap();
        for after in 0..24 {
            let new = scene(after);
            assert!(canonical(&new));
            let mut actual = SurfaceBuffer::new(96, 64).unwrap();
            actual.pixels.copy_from_slice(&prepared.pixels);
            let mut expected = SurfaceBuffer::new(96, 64).unwrap();
            raster(&mut expected, &new, None, draw).unwrap();
            raster(&mut actual, &new, Some(&old), draw).unwrap();
            assert_eq!(
                actual.pixels, expected.pixels,
                "before={before}, after={after}"
            );
        }
    }
}
#[test]
fn unchanged_scene_does_no_raster_work_and_local_edit_keeps_distant_pixels() {
    let extent = rect(0, 0, 96, 64);
    let mut old = GraphicsScene::empty();
    old.push(
        GraphicsCommand::rect(extent, extent, Paint::Background, GraphicsShapeStyle::Fill).unwrap(),
    )
    .unwrap();
    old.push(GraphicsCommand::text(rect(2, 2, 25, 20), extent, Paint::Foreground, "i").unwrap())
        .unwrap();
    old.push(
        GraphicsCommand::icon(
            rect(64, 32, 24, 24),
            extent,
            Paint::Accent,
            FaceIconKey::Clock,
        )
        .unwrap(),
    )
    .unwrap();
    let mut actual = SurfaceBuffer::new(96, 64).unwrap();
    let full = raster(&mut actual, &old, None, draw).unwrap();
    let pixels = actual.pixels.clone();
    let identical = raster(&mut actual, &old, Some(&old), |_, _| {
        panic!("unchanged scene must reuse pixels")
    })
    .unwrap();
    assert_eq!(identical, DisplayReceipt::default());
    assert_eq!(actual.pixels, pixels);
    let mut new = GraphicsScene::empty();
    for (index, command) in old.commands().iter().enumerate() {
        new.push(if index == 1 {
            GraphicsCommand::text(command.bounds, command.clip, Paint::Foreground, "W").unwrap()
        } else {
            *command
        })
        .unwrap();
    }
    let partial = raster(&mut actual, &new, Some(&old), draw).unwrap();
    assert_eq!(partial.commands, 2);
    assert!(partial.pixels_written < full.pixels_written);
    for y in 0..64 {
        for x in 28..96 {
            assert_eq!(actual.pixels[y * 96 + x], pixels[y * 96 + x]);
        }
    }
    let mut expected = SurfaceBuffer::new(96, 64).unwrap();
    raster(&mut expected, &new, None, draw).unwrap();
    assert_eq!(actual.pixels, expected.pixels);
}
#[test]
fn edited_public_fields_that_break_canonical_scene_refuse_reuse() {
    let mut command = GraphicsCommand::rect(
        rect(1, 1, 3, 3),
        rect(0, 0, 10, 10),
        Paint::Accent,
        GraphicsShapeStyle::Fill,
    )
    .unwrap();
    command.bounds.width = 0;
    let mut scene = GraphicsScene::empty();
    scene.push(command).unwrap();
    assert!(!canonical(&scene));
}
#[test]
fn failed_render_does_not_produce_success_receipt() {
    let mut buffer = SurfaceBuffer::new(96, 64).unwrap();
    let old = scene(1);
    raster(&mut buffer, &old, None, draw).unwrap();
    let result = raster(&mut buffer, &scene(2), Some(&old), |_, _| {
        Err(DisplayError::Lost)
    });
    assert_eq!(result, Err(DisplayError::Lost));
}

#[allow(dead_code)]
mod fixture {
    use crate as conduitos;
    include!("../../../tests/common/native_compositor_fixture.rs");
}

#[test]
fn reuse_keeps_revision_identity_and_resize_fences() {
    use crate::native_compositor::NativeCompositorError;
    use conduit_core::HostBaseId;
    let (face, plan, main, _, mut compositor) = fixture::fixture(1);
    let (next_face, next_plan, next_main, _, _) = fixture::fixture(2);
    let base = HostBaseId::from(fixture::BASE);
    let scene = scene(3);
    let first = compositor
        .update_surface(&face, &main, &plan, fixture::MAIN, &base, &scene)
        .unwrap()
        .clone();
    assert!(first.display.commands > 0);
    let second = compositor
        .update_surface(
            &next_face,
            &next_main,
            &next_plan,
            fixture::MAIN,
            &base,
            &scene,
        )
        .unwrap();
    assert_eq!(second.display, DisplayReceipt::default());
    assert_eq!(second.face_id, next_face.identity);
    assert_eq!(second.manifestation_id, next_main.manifestation_id);
    assert_eq!(second.active_play_id, next_main.active_play_id);
    assert_eq!(
        compositor.update_surface(
            &next_face,
            &next_main,
            &next_plan,
            fixture::MAIN,
            &base,
            &scene
        ),
        Err(NativeCompositorError::StaleSurfaceRevision)
    );
    compositor
        .place_surface(fixture::MAIN, rect(1, 1, 14, 9), 0)
        .unwrap();
    let (third_face, third_plan, third_main, _, _) = fixture::fixture(3);
    let third = compositor
        .update_surface(
            &third_face,
            &third_main,
            &third_plan,
            fixture::MAIN,
            &base,
            &scene,
        )
        .unwrap();
    assert!(
        third.display.commands > 0,
        "replacement extent must be fully rasterized"
    );
}

#[test]
fn raster_reuse_and_scene_eligibility_allocate_no_storage() {
    let old = scene(3);
    let new = scene(4);
    let mut buffer = SurfaceBuffer::new(96, 64).unwrap();
    raster(&mut buffer, &old, None, draw).unwrap();
    let mut result = None;
    let allocations = crate::test_allocations::allocations(|| {
        assert!(canonical(&new));
        result = Some(raster(&mut buffer, &new, Some(&old), draw));
    });
    result.unwrap().unwrap();
    assert_eq!(allocations, 0);
}

#[test]
fn failed_scene_invalidates_old_pixels_and_cannot_advance_revision() {
    use crate::native_compositor::NativeCompositorError;
    use conduit_core::HostBaseId;
    let (face, plan, main, _, mut compositor) = fixture::fixture(1);
    fixture::update(&mut compositor, &face, &plan, &main, fixture::MAIN);
    let (next_face, next_plan, next_main, _, _) = fixture::fixture(2);
    let base = HostBaseId::from(fixture::BASE);
    let bounds = rect(0, 0, 4, 4);
    let mut invalid =
        GraphicsCommand::rect(bounds, bounds, Paint::Accent, GraphicsShapeStyle::Fill).unwrap();
    invalid.kind = GraphicsCommandKind::OrthogonalPath;
    let mut scene = GraphicsScene::empty();
    scene.push(invalid).unwrap();
    assert!(!canonical(&scene));
    assert_eq!(
        compositor.update_surface(
            &next_face,
            &next_main,
            &next_plan,
            fixture::MAIN,
            &base,
            &scene
        ),
        Err(NativeCompositorError::Display(DisplayError::InvalidExtent))
    );
    let surface = &compositor.surfaces[0];
    assert!(surface.receipt.is_none());
    assert!(surface.raster_scene.is_none());
    assert!(!surface.is_ready());
    assert_eq!(
        surface.binding.as_ref().unwrap().last_revision,
        face.revision
    );
    let scene = super::tests::scene(3);
    let receipt = compositor
        .update_surface(
            &next_face,
            &next_main,
            &next_plan,
            fixture::MAIN,
            &base,
            &scene,
        )
        .unwrap();
    assert!(
        receipt.display.commands > 0,
        "failure must force a fresh raster"
    );
}

#[cfg(feature = "native-compositor")]
#[test]
fn palette_and_scale_change_force_full_raster_under_new_exact_receipt() {
    use conduit_core::HostBaseId;
    use conduit_face::{
        VisualExpression, VisualPalette, VisualUserConstraints, admit_visual_expression,
    };
    let (face, plan, main, _, mut compositor) = fixture::fixture(1);
    let scene = scene(3);
    let base = HostBaseId::from(fixture::BASE);
    compositor
        .update_surface(&face, &main, &plan, fixture::MAIN, &base, &scene)
        .unwrap();
    let original = compositor.surfaces[0].buffer.pixels.clone();
    let mut expression = VisualExpression::phosphor();
    expression.palette = VisualPalette::Editorial;
    expression.text_scale_percent = 125;
    compositor.surfaces[0].visual_expression = Some(
        admit_visual_expression(
            Ok(expression),
            VisualUserConstraints {
                forced_colors: false,
                reduced_motion: false,
                minimum_text_scale_percent: 100,
                maximum_supported_text_scale_percent: 200,
            },
        )
        .unwrap(),
    );
    let (next_face, next_plan, next_main, _, _) = fixture::fixture(2);
    let receipt = compositor
        .update_surface(
            &next_face,
            &next_main,
            &next_plan,
            fixture::MAIN,
            &base,
            &scene,
        )
        .unwrap();
    assert_eq!(
        usize::from(receipt.display.commands),
        scene.commands().len()
    );
    assert_eq!(
        receipt
            .visual_expression
            .as_ref()
            .unwrap()
            .expression
            .palette,
        VisualPalette::Editorial
    );
    assert_ne!(compositor.surfaces[0].buffer.pixels, original);
}

#[cfg(feature = "native-compositor")]
#[test]
fn typography_alone_invalidates_retained_raster_without_changing_paint_or_scale() {
    use conduit_core::HostBaseId;
    use conduit_face::{
        VisualExpression, VisualTypography, VisualUserConstraints, admit_visual_expression,
    };
    let (face, plan, main, _, mut compositor) = fixture::fixture(1);
    let extent = rect(0, 0, 96, 64);
    let mut scene = GraphicsScene::empty();
    scene
        .push(
            GraphicsCommand::rect(extent, extent, Paint::Background, GraphicsShapeStyle::Fill)
                .unwrap(),
        )
        .unwrap();
    scene
        .push(
            GraphicsCommand::text(rect(4, 4, 80, 48), extent, Paint::Foreground, "WiJ")
                .unwrap()
                .with_text_role(conduit_face::GraphicsTextRole::Title)
                .unwrap(),
        )
        .unwrap();
    let constraints = VisualUserConstraints {
        forced_colors: false,
        reduced_motion: false,
        minimum_text_scale_percent: 100,
        maximum_supported_text_scale_percent: 200,
    };
    compositor.surfaces[0].visual_expression =
        Some(admit_visual_expression(Ok(VisualExpression::phosphor()), constraints).unwrap());
    let base = HostBaseId::from(fixture::BASE);
    compositor
        .update_surface(&face, &main, &plan, fixture::MAIN, &base, &scene)
        .unwrap();
    let original = compositor.surfaces[0].buffer.pixels.clone();
    let mut expression = VisualExpression::phosphor();
    expression.typography = VisualTypography::Engineering;
    compositor.surfaces[0].visual_expression = Some(
        admit_visual_expression(
            Ok(expression),
            VisualUserConstraints {
                forced_colors: false,
                reduced_motion: false,
                minimum_text_scale_percent: 100,
                maximum_supported_text_scale_percent: 200,
            },
        )
        .unwrap(),
    );
    let (next_face, next_plan, next_main, _, _) = fixture::fixture(2);
    let receipt = compositor
        .update_surface(
            &next_face,
            &next_main,
            &next_plan,
            fixture::MAIN,
            &base,
            &scene,
        )
        .unwrap();
    assert_eq!(
        usize::from(receipt.display.commands),
        scene.commands().len()
    );
    assert_eq!(
        receipt
            .visual_expression
            .as_ref()
            .unwrap()
            .expression
            .typography,
        VisualTypography::Engineering
    );
    assert_ne!(compositor.surfaces[0].buffer.pixels, original);
}
