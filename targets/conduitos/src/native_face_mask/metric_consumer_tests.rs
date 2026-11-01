//! Checked Source reaches ordinary Mask preparation, not just scene helpers.
use super::*;
use crate::display::typography::{TextLayout, Typography};
use conduit_conduitos_mask_offer::{
    NativeMaskStorageProfile, layout_resource_renderer_offer, prepare_layout_component_document,
};
use conduit_face::{CRECHE_LAYOUT_MASK_SOURCE, VisualUserConstraints};

#[test]
fn checked_source_metrics_reach_structured_and_capacity_fallback_geometry() {
    let (service, draft) = super::super::tests::fixture();
    let face = draft.face(&service.birth_basis("arrival/metrics")).unwrap();
    let provider = crate::product_bases::fixture_surface_provider();
    let mut host = conduit_conduitos_mask_offer::native_host_advertisement(
        &"host/native".into(),
        &"boot/native".into(),
        1,
        &provider.entry,
    );
    host.capabilities.push(layout_resource_renderer_offer(
        NativeMaskStorageProfile::Standard,
    ));
    let source = CRECHE_LAYOUT_MASK_SOURCE
        .replace("type_body_px: 16", "type_body_px: 18")
        .replace("type_small_px: 14", "type_small_px: 16")
        .replace("line_height_percent: 150", "line_height_percent: 160");
    let mut reference = None;
    for fallback in [false, true] {
        let source = if fallback {
            source.replace("inline_shrink: 1", "inline_shrink: 2")
        } else {
            source.clone()
        };
        let (_, configuration) = prepare_layout_component_document(
            &host,
            &source,
            "creche-mask-columns",
            NativeMaskStorageProfile::Standard,
        )
        .unwrap();
        for scale in [100, 200] {
            let (mut scene, admitted, reason) = NativeFaceMask::prepare_component_scene(
                &configuration,
                &face,
                800,
                600,
                true,
                VisualUserConstraints {
                    forced_colors: false,
                    reduced_motion: true,
                    minimum_text_scale_percent: scale,
                    maximum_supported_text_scale_percent: 200,
                },
            )
            .unwrap();
            assert_eq!(scene.face(), &face);
            assert_eq!(reason.is_some(), fallback);
            let typography = Typography::with_visual_look(
                admitted.visual.expression.typography,
                &admitted.selected,
                200,
            )
            .unwrap();
            let mut controls = alloc::collections::BTreeSet::new();
            for details in [false, true] {
                scene.show_details(details);
                for _ in 0..scene.pages() {
                    let frame = scene.frame().unwrap();
                    for hit in frame.hits {
                        controls.insert((hit.control.action, hit.control.argument));
                    }
                    for command in frame.scene.commands() {
                        if command.kind == conduit_face::GraphicsCommandKind::Text {
                            let measured = TextLayout::new_selected(
                                command.payload(),
                                typography.style(command.text_role().into()),
                                command.bounds.width,
                                scale,
                            )
                            .unwrap()
                            .finish_height();
                            assert!(
                                measured <= u32::from(command.bounds.height),
                                "{}",
                                command.payload()
                            );
                        }
                    }
                    scene.turn_page(true);
                }
            }
            assert!(!controls.is_empty());
            if let Some(previous) = &reference {
                assert_eq!(&controls, previous);
            }
            reference = Some(controls);
        }
    }
}
