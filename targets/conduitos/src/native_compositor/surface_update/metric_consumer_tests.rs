//! CPU retained pixels and refusal custody; not physical scanout or Show ACK.
use super::*;
use crate::display::{
    PixelTarget,
    typography::{Typography, render_scene_selected},
};
use conduit_face::{
    GraphicsCommand, GraphicsPaintRole, GraphicsShapeStyle, MaskShow, VisualUserConstraints,
};

fn setup(
    body: u16,
) -> (
    Face,
    MaskShow,
    conduit_face::AdmittedVisualLook,
    NativeCompositor,
) {
    let provider = crate::product_bases::fixture_surface_provider();
    let mut host = conduit_conduitos_mask_offer::native_host_advertisement(
        &"host/metrics".into(),
        &"boot/metrics".into(),
        1,
        &provider.entry,
    );
    host.capabilities.push(
        conduit_conduitos_mask_offer::layout_resource_renderer_offer(
            conduit_conduitos_mask_offer::NativeMaskStorageProfile::Standard,
        ),
    );
    let source = conduit_face::TODO_LAYOUT_MASK_SOURCE
        .replace("type_body_px: 16", &alloc::format!("type_body_px: {body}"))
        .replace("type_small_px: 14", "type_small_px: 16")
        .replace("line_height_percent: 150", "line_height_percent: 160");
    let (stage, configuration) = conduit_conduitos_mask_offer::prepare_layout_component_document(
        &host,
        &source,
        "todo-mask-flat-compact",
        conduit_conduitos_mask_offer::NativeMaskStorageProfile::Standard,
    )
    .unwrap();
    let face = crate::native_face_scene::tests::face(2);
    let placement = stage.planned_mask.show_placement();
    let show = MaskShow::prepared(
        &stage.planned_mask,
        &face,
        conduit_core::bind_active_play(
            &stage.planned_mask.plan.plan_id,
            &placement.host_id,
            &placement.boot_id,
            1,
        ),
        face.subjects[0].identity.clone(),
        "surface/metrics".into(),
        "sign/prepared".into(),
    )
    .unwrap();
    let admission = CompositorAdmission::new(
        placement.host_id.clone(),
        placement.boot_id.clone(),
        placement.offer_generation,
        placement.implementation_id.clone(),
        provider.entry.base_id.clone(),
        alloc::vec![placement.placement_id.clone()],
        alloc::vec!["surface/metrics".into()],
    )
    .unwrap();
    let mut compositor = NativeCompositor::admitted(admission);
    compositor
        .admit_surface("surface/metrics", extent(), 0)
        .unwrap();
    let admitted = conduit_face::admit_visual_look(
        &configuration.requested_look().unwrap(),
        VisualUserConstraints {
            forced_colors: false,
            reduced_motion: true,
            minimum_text_scale_percent: 100,
            maximum_supported_text_scale_percent: 200,
        },
    )
    .unwrap();
    (face, show, admitted, compositor)
}
fn extent() -> LayoutRect {
    LayoutRect {
        x: 0,
        y: 0,
        width: 256,
        height: 128,
    }
}
fn scene() -> GraphicsScene {
    let mut scene = GraphicsScene::empty();
    scene
        .push(
            GraphicsCommand::rect(
                extent(),
                extent(),
                GraphicsPaintRole::Background,
                GraphicsShapeStyle::Fill,
            )
            .unwrap(),
        )
        .unwrap();
    scene
        .push(
            GraphicsCommand::text(extent(), extent(), GraphicsPaintRole::Foreground, "AéJp\nW")
                .unwrap(),
        )
        .unwrap();
    scene
}

#[test]
fn retained_surface_paints_selected_metrics_and_repaint_reuses_exact_pixels() {
    let (face, show, admitted, mut compositor) = setup(18);
    let base = compositor.admission.display_base_id.clone();
    let scene = scene();
    let receipt = compositor
        .update_pending_look_mask_surface(&face, &show, &base, &scene, &admitted)
        .unwrap()
        .clone();
    assert_eq!(receipt.visual_look.as_ref(), Some(&admitted.selected));
    let mut expected = SurfaceBuffer::new(256, 128).unwrap();
    let format = expected.format();
    let typography = Typography::with_visual_look(
        admitted.visual.expression.typography,
        &admitted.selected,
        200,
    )
    .unwrap();
    render_scene_selected(
        &mut expected,
        &scene,
        |_, c| c.text_role().into(),
        100,
        typography,
        |role| {
            super::super::paint::visual_look_paint(
                format,
                role,
                Some(&admitted.visual),
                Some(&admitted.selected),
            )
        },
    )
    .unwrap();
    assert_eq!(compositor.surfaces[0].buffer.pixels, expected.pixels);
    let mut old = SurfaceBuffer::new(256, 128).unwrap();
    render_scene_selected(
        &mut old,
        &scene,
        |_, c| c.text_role().into(),
        100,
        Typography::new(admitted.visual.expression.typography),
        |role| {
            super::super::paint::visual_look_paint(
                format,
                role,
                Some(&admitted.visual),
                Some(&admitted.selected),
            )
        },
    )
    .unwrap();
    assert_ne!(
        expected.pixels, old.pixels,
        "test must detect fixed-profile paint"
    );
    let available = show
        .transition(ManifestationLifecycle::Available, "sign/available".into())
        .unwrap();
    let repaint = compositor
        .repaint_mask_surface(&face, &available, &base, &scene)
        .unwrap();
    assert_eq!(repaint.display.pixels_written, 0);
    assert_eq!(compositor.surfaces[0].buffer.pixels, expected.pixels);
}

#[test]
fn oversized_selected_metrics_refuse_before_retained_surface_mutation() {
    let (face, show, admitted, mut compositor) = setup(64);
    let base = compositor.admission.display_base_id.clone();
    compositor.surfaces[0].buffer.pixels.fill(0x123456);
    assert_eq!(
        compositor.update_pending_look_mask_surface(&face, &show, &base, &scene(), &admitted),
        Err(NativeCompositorError::Display(DisplayError::InvalidExtent))
    );
    let surface = &compositor.surfaces[0];
    assert!(surface.buffer.pixels.iter().all(|pixel| *pixel == 0x123456));
    assert!(
        surface.binding.is_none()
            && surface.receipt.is_none()
            && surface.visual_look.is_none()
            && surface.visual_expression.is_none()
    );
}
