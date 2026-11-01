//! Checked Source/Back preparation of reusable Looks, without a Show or input grant.
use super::*;
use alloc::{format, string::String, vec::Vec};
use conduit_conduitos_mask_offer::{
    NativeMaskStorageProfile, native_layout_host_advertisement, prepare_layout_component_document,
};
use conduit_face::{
    CRECHE_LAYOUT_MASK_SOURCE, FIELD_INSTRUMENT_LOOKS_SOURCE, TODO_LAYOUT_MASK_SOURCE,
    VisualUserConstraints, field_instrument_day_look, field_instrument_night_look,
};
use conduit_plot::rust_binding::NativeRustBinding;

// Bind the exact authored expression, rather than independently reconstructing
// its tokens. All roots share one Source document; no renderer config is forged.
fn source() -> String {
    let authored = conduit_plot::parse_syntax_document(FIELD_INSTRUMENT_LOOKS_SOURCE);
    let mut source = format!("{TODO_LAYOUT_MASK_SOURCE}\n{CRECHE_LAYOUT_MASK_SOURCE}\n");
    for theme in ["day", "night"] {
        let plot = authored
            .plots
            .iter()
            .find(|plot| plot.name.text == format!("look/field-instrument-{theme}"))
            .unwrap();
        let [parameter] = plot.front.startup_parameters.as_slice() else {
            panic!("one ordinary authored Look parameter");
        };
        assert_eq!(parameter.name.text, "look");
        let literal = &parameter.default.as_ref().unwrap().text;
        for component in ["todo-component-flat", "creche-component-columns"] {
            source.push_str(&format!(
                "plot {component}-{theme} (\n >> face: Face\n show: Show >>\n \
                 interaction: FaceInteraction...| >>\n) {{\n \
                 component: {component}(look = {literal})\n \
                 face >> component.face\n component.show >> show\n \
                 component.interaction >> interaction\n}}\n"
            ));
        }
    }
    source
}

fn qualify(face: &Face) {
    let source = source();
    let provider = crate::product_bases::fixture_surface_provider();
    let host = native_layout_host_advertisement(
        &"host/field-reuse".into(),
        &"boot/field-reuse".into(),
        1,
        &provider.entry,
        NativeMaskStorageProfile::Standard,
    );
    let mut document = None;
    let mut structured = false;
    for component in ["todo-component-flat", "creche-component-columns"] {
        let mut reference = None;
        for (theme, expected) in [
            ("day", field_instrument_day_look().unwrap()),
            ("night", field_instrument_night_look().unwrap()),
        ] {
            let (_, configuration) = prepare_layout_component_document(
                &host,
                &source,
                &format!("{component}-{theme}"),
                NativeMaskStorageProfile::Standard,
            )
            .unwrap();
            assert_eq!(
                configuration.look.canonical_value(),
                expected.clone().encode().unwrap()
            );
            assert_eq!(configuration.requested_look().unwrap(), expected);
            if let Some(previous) = &document {
                assert_eq!(&configuration.component.source_document_id, previous);
            }
            document = Some(configuration.component.source_document_id.clone());
            let mut encounters = Vec::new();
            for scale in [100, 200] {
                let constraints = VisualUserConstraints {
                    forced_colors: false,
                    reduced_motion: true,
                    minimum_text_scale_percent: scale,
                    maximum_supported_text_scale_percent: 200,
                };
                let (mut scene, admitted, fallback) = NativeFaceMask::prepare_component_scene(
                    &configuration,
                    face,
                    1024,
                    600,
                    false,
                    constraints,
                )
                .unwrap();
                structured |= fallback.is_none();
                assert_eq!(scene.face(), face);
                assert_eq!(admitted.requested, expected);
                assert_eq!(
                    admitted,
                    conduit_face::admit_visual_look(&expected, constraints).unwrap()
                );
                let mut rendered_pages = Vec::new();
                for details in [false, true] {
                    scene.show_details(details);
                    for _ in 0..scene.pages() {
                        let frame = scene.frame().unwrap();
                        // Preparation is not surface acknowledgement or input admission.
                        assert!(frame.hits.is_empty());
                        let text: Vec<_> = frame
                            .scene
                            .commands()
                            .iter()
                            .filter(|command| {
                                command.kind == conduit_face::GraphicsCommandKind::Text
                            })
                            .map(|command| String::from(command.payload()))
                            .collect();
                        assert!(!text.is_empty());
                        rendered_pages.push(text);
                        scene.turn_page(true);
                    }
                }
                // Palette replacement must preserve content, paging and fallback.
                encounters.push((rendered_pages, scene.pages(), fallback));
                assert_eq!(scene.face(), face);
            }
            if let Some(previous) = &reference {
                assert_eq!(&encounters, previous);
            }
            reference = Some(encounters);
        }
    }
    assert!(
        structured,
        "at least one checked layout must render without generic fallback"
    );
}

#[test]
fn authored_field_looks_reach_both_checked_layouts_without_changing_face() {
    let (service, draft) = super::super::tests::fixture();
    let face = draft
        .face(&service.birth_basis("arrival/field-reuse"))
        .unwrap();
    qualify(&face);
}

#[test]
#[ignore = "requires explicitly supplied retained real environmental and Todo Face exports"]
fn retained_real_faces_use_the_same_authored_looks_and_checked_layouts() {
    #[derive(serde::Deserialize)]
    struct RetainedExport {
        face: Face,
    }
    for key in ["CONDUIT_FIELD_ENVIRONMENT_FACE", "CONDUIT_FIELD_TODO_FACE"] {
        let path = std::env::var_os(key).expect("explicit retained Face path");
        let bytes = std::fs::read(path).unwrap();
        let export: RetainedExport = serde_json::from_slice(&bytes).unwrap();
        qualify(&export.face);
    }
}
