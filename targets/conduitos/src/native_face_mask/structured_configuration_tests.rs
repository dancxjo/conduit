//! Exact Source/Back custody through native preparation; CPU only.
use super::*;
use conduit_conduitos_mask_offer::{
    NativeMaskStorageProfile, layout_resource_renderer_offer, prepare_layout_component_document,
};
use conduit_face::{CRECHE_LAYOUT_MASK_SOURCE, VisualUserConstraints};

#[test]
fn explicit_layout_configuration_reaches_consumer_and_preserves_complete_fallback() {
    let (service, draft) = fixture();
    let face = draft.face(&service.birth_basis("arrival/layout")).unwrap();
    let provider = crate::product_bases::fixture_surface_provider();
    let mut host = conduit_conduitos_mask_offer::native_host_advertisement(
        &"host/native".into(),
        &"boot/native".into(),
        1,
        &provider.entry,
    );
    let profile = NativeMaskStorageProfile::Standard;
    // This supplied offer is a CPU fixture, not a runtime observation or Show.
    assert!(
        prepare_layout_component_document(
            &host,
            CRECHE_LAYOUT_MASK_SOURCE,
            "creche-mask-columns",
            profile,
        )
        .is_err()
    );
    host.capabilities
        .push(layout_resource_renderer_offer(profile));
    let constraints = VisualUserConstraints {
        forced_colors: false,
        reduced_motion: true,
        minimum_text_scale_percent: 100,
        maximum_supported_text_scale_percent: 200,
    };
    let expression = conduit_face::admit_visual_expression(
        Ok(conduit_face::VisualExpression::phosphor()),
        constraints,
    )
    .unwrap();
    let baseline = crate::native_face_scene::NativeFaceScene::prepare_visual_fallback(
        face.clone(),
        1280,
        800,
        true,
        &expression,
    )
    .unwrap();
    let expected = observe(baseline);
    let nonvisual_look = r#"{generative: none(empty), identity: "conduit.look/nonvisual@1", spoken: none(empty), version: 2, visual: none(empty)}"#;
    let nonvisual_source = CRECHE_LAYOUT_MASK_SOURCE
        .lines()
        .map(|line| {
            if line.trim_start().starts_with("component: ") {
                let (component, _) = line.split_once("(look = ").unwrap();
                alloc::format!("{component}(look = {nonvisual_look})")
            } else {
                alloc::string::String::from(line)
            }
        })
        .collect::<alloc::vec::Vec<_>>()
        .join("\n");
    for (source, expected_fallback, nonvisual) in [
        (
            alloc::string::String::from(CRECHE_LAYOUT_MASK_SOURCE),
            false,
            false,
        ),
        (
            CRECHE_LAYOUT_MASK_SOURCE.replace("inline_shrink: 1", "inline_shrink: 2"),
            true,
            false,
        ),
        (nonvisual_source, false, true),
    ] {
        let (stage, configuration) =
            prepare_layout_component_document(&host, &source, "creche-mask-columns", profile)
                .unwrap();
        assert_eq!(stage.planned_mask.mask.plot_name, "creche-mask-columns");
        assert!(configuration.layout.is_some());
        let (scene, admitted, fallback) = NativeFaceMask::prepare_component_scene(
            &configuration,
            &face,
            1280,
            800,
            true,
            constraints,
        )
        .unwrap();
        assert_eq!(scene.face(), &face);
        if nonvisual {
            assert!(matches!(
                admitted.requested.visual(),
                conduit_face::LookVisualFacetOption::None
            ));
            assert_eq!(admitted.selected.identity(), conduit_face::PHOSPHOR_LOOK_ID);
            assert!(matches!(
                admitted.visual.outcome,
                conduit_face::VisualExpressionOutcome::PhosphorFallback {
                    refusal: conduit_face::VisualExpressionRefusal::MissingVisualFacet
                }
            ));
        }
        if expected_fallback {
            assert_eq!(
                fallback.as_deref(),
                Some("ConfiguredLayout(UnsupportedStructure)")
            );
        } else {
            assert!(fallback.is_none(), "{fallback:?}");
        }
        let observed = observe(scene);
        // Preserve every exact action/argument control, not merely the set of
        // actions. The complete generic route is the established baseline.
        assert_eq!(observed.0, expected.0);
        if expected_fallback {
            // This includes non-action headings, help, review and other meaning,
            // across both primary and Details views and every continuation.
            assert_eq!(observed.1, expected.1);
        }
    }
}

fn observe(
    mut scene: crate::native_face_scene::NativeFaceScene,
) -> (
    alloc::collections::BTreeSet<(usize, Option<usize>)>,
    alloc::vec::Vec<alloc::string::String>,
) {
    let mut controls = alloc::collections::BTreeSet::new();
    let mut text = alloc::vec::Vec::new();
    for details in [false, true] {
        scene.show_details(details);
        for page in 0..scene.pages() {
            let frame = scene.frame().unwrap();
            assert!(frame.scene.commands().len() <= conduit_face::MAX_GRAPHICS_COMMANDS);
            for hit in &frame.hits {
                scene
                    .resolve(frame.face_id, frame.revision, hit.control)
                    .unwrap();
                controls.insert((hit.control.action, hit.control.argument));
            }
            text.extend(
                frame
                    .scene
                    .commands()
                    .iter()
                    .filter(|command| command.kind == conduit_face::GraphicsCommandKind::Text)
                    .map(|command| alloc::string::String::from(command.payload())),
            );
            if page + 1 < scene.pages() {
                scene.turn_page(true);
            }
        }
    }
    (controls, text)
}
