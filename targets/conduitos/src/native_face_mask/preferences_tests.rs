//! Checked native Host/Mask and CPU presentation; no QEMU, Pi or owner authority claim.
use super::*;
use alloc::{vec, vec::Vec};
use conduit_face::{
    CrecheComponentRecipe, FaceInteractionRefusal, LocalLookContrastOutcome,
    LocalLookVisualOverride, LookVisualFacetOption,
};
use conduit_plot::rust_binding::BoundedSequence;

fn local(values: Vec<LocalLookVisualOverride>) -> LocalLookInfo {
    LocalLookInfo::new(
        LocalLookContrast::Standard,
        "personal/readable@1".into(),
        1,
        BoundedSequence::try_from_iter(values).unwrap(),
    )
    .unwrap()
}
fn configured() -> NativeFaceMask {
    let provider = crate::product_bases::fixture_surface_provider();
    NativeFaceMask::prepare_creche(
        "host/native-personal".into(),
        "boot/native-personal".into(),
        OfferGeneration(1),
        "personal-look",
        provider.entry.base_id.clone(),
        "surface/native-personal",
        &provider,
        CrecheComponentRecipe::SingleColumn,
    )
    .unwrap()
}

#[test]
fn personal_look_replacement_keeps_author_face_and_refuses_the_old_show() {
    let mut mask = configured();
    let draft = super::super::tests::draft();
    let face = draft
        .face(&mask.birth_basis("arrival/native-personal"))
        .unwrap();
    let author = mask.component_configuration().unwrap().clone();
    let mut pixels = super::super::tests::CountingDisplay::default();
    mask.present(face.clone(), 1, 1, &mut pixels).unwrap();
    let old_show = mask.show().unwrap().clone();
    let action = face
        .actions
        .iter()
        .position(|action| action.identity == "creche.suggest")
        .unwrap();
    let held = mask
        .scene()
        .unwrap()
        .interaction(
            &face.identity,
            face.revision,
            crate::native_face_scene::FaceControl {
                action,
                argument: None,
            },
            &old_show,
            vec![],
            1,
        )
        .unwrap();
    let writes = pixels.writes;
    mask.scene.as_mut().unwrap().focus_next(true);
    let focus = mask.scene().unwrap().focused_request();
    assert!(focus.is_some());
    let mut personal = configured();
    personal.set_local_visual_look(
        Some(local(vec![
            LocalLookVisualOverride::TypeBodyPx(20),
            LocalLookVisualOverride::SpacePanelPx(0),
        ])),
        LocalLookContrast::Standard,
    );
    personal.preserve_encounter_from(&mut mask).unwrap();
    mask.suspend().unwrap();
    assert!(mask.show().is_none());
    personal.present(face.clone(), 2, 2, &mut pixels).unwrap();
    let mask = personal;
    assert!(pixels.writes > writes);
    assert_eq!(mask.component_configuration(), Some(&author));
    assert_eq!(mask.scene().unwrap().face(), &face);
    assert_eq!(mask.scene().unwrap().focused_request(), focus);
    let resolved = mask.local_visual_look_resolution().unwrap();
    assert_eq!(resolved.author, author.requested_look().unwrap());
    let LookVisualFacetOption::Some(visual) = resolved.admitted.selected.visual() else {
        panic!("visual")
    };
    assert_eq!(*visual.tokens().type_body_px(), 20);
    assert_eq!(*visual.tokens().space_panel_px(), 0);
    assert_ne!(mask.show().unwrap().show_id, old_show.show_id);
    assert_eq!(
        held.validate_against(&face, mask.show().unwrap()),
        Err(FaceInteractionRefusal::StaleShow)
    );
    let mut previous = mask;
    let mut authored = configured();
    authored.set_local_visual_look(None, LocalLookContrast::Standard);
    authored.preserve_encounter_from(&mut previous).unwrap();
    previous.suspend().unwrap();
    authored.present(face.clone(), 3, 3, &mut pixels).unwrap();
    assert!(authored.local_visual_look_resolution().is_none());
    assert_eq!(authored.component_configuration(), Some(&author));
    assert_eq!(authored.scene().unwrap().face(), &face);
    assert_eq!(authored.scene().unwrap().focused_request(), focus);
}

#[test]
fn invalid_local_proposals_have_distinct_refusals_without_drawing() {
    let mut mask = configured();
    let face = super::super::tests::draft()
        .face(&mask.birth_basis("arrival/refused-local"))
        .unwrap();
    let mut pixels = super::super::tests::CountingDisplay::default();
    for (sequence, (proposal, expected)) in [
        (
            local(vec![
                LocalLookVisualOverride::TypeBodyPx(20),
                LocalLookVisualOverride::TypeBodyPx(22),
            ]),
            NativeLocalLookRefusal::DuplicateField,
        ),
        (
            local(vec![
                LocalLookVisualOverride::text_primary(20, 20, 20).unwrap(),
            ]),
            NativeLocalLookRefusal::Contrast,
        ),
        (
            local(vec![LocalLookVisualOverride::RadiusControlPx(0)]),
            NativeLocalLookRefusal::UnsupportedField("radius_control_px"),
        ),
        (
            local(vec![LocalLookVisualOverride::RadiusPanelPx(8)]),
            NativeLocalLookRefusal::UnsupportedField("radius_panel_px"),
        ),
    ]
    .into_iter()
    .enumerate()
    {
        mask.set_local_visual_look(Some(proposal), LocalLookContrast::Standard);
        assert_eq!(
            mask.present(
                face.clone(),
                sequence as u64 + 1,
                sequence as u64 + 1,
                &mut pixels
            ),
            Err(NativeFaceMaskError::LocalLook(expected))
        );
        assert_eq!(pixels.writes, 0);
        assert!(mask.show().is_none());
    }
}

#[test]
fn independent_accessibility_floor_reaches_native_paint_and_read_only_show() {
    let mut mask = configured();
    let face = super::super::tests::draft()
        .face(&mask.birth_basis("arrival/accessible-local"))
        .unwrap();
    mask.set_visual_user_constraints(true, true, 200).unwrap();
    mask.set_local_visual_look(
        Some(local(vec![
            LocalLookVisualOverride::TextScalePercent(100),
            LocalLookVisualOverride::text_primary(20, 20, 20).unwrap(),
        ])),
        LocalLookContrast::Enhanced,
    );
    let mut pixels = super::super::tests::CountingDisplay::default();
    mask.present_read_only(face.clone(), 1, 1, &mut pixels)
        .unwrap();
    assert!(pixels.writes > 0);
    let resolved = mask.local_visual_look_resolution().unwrap();
    assert_eq!(resolved.required_contrast, LocalLookContrast::Enhanced);
    assert_eq!(
        resolved.contrast,
        LocalLookContrastOutcome::PlatformColorsRequired
    );
    assert_eq!(resolved.admitted.visual.expression.text_scale_percent, 200);
    assert!(resolved.admitted.visual.platform_forced_colors);
    assert_eq!(mask.scene().unwrap().face(), &face);
    assert!(mask.scene().unwrap().frame().unwrap().hits.is_empty());
}

#[test]
fn unconfigured_mask_explicitly_refuses_personal_look() {
    let (mut mask, draft) = super::super::tests::fixture();
    let face = draft
        .face(&mask.birth_basis("arrival/no-author-local"))
        .unwrap();
    mask.set_local_visual_look(Some(local(vec![])), LocalLookContrast::Standard);
    let mut pixels = super::super::tests::CountingDisplay::default();
    assert_eq!(
        mask.present(face, 1, 1, &mut pixels),
        Err(NativeFaceMaskError::LocalLookUnsupported)
    );
    assert_eq!(pixels.writes, 0);
    assert!(mask.show().is_none());
}

#[test]
#[ignore = "requires retained real Faces and their checked personal browser views"]
fn identical_personal_resources_select_identical_native_and_browser_looks() {
    use conduit_conduitos_mask_offer::{
        NativeMaskStorageProfile, native_layout_host_advertisement,
        prepare_layout_component_document,
    };
    use conduit_plot::rust_binding::NativeRustBinding;
    #[derive(serde::Deserialize)]
    struct Export {
        face: Face,
    }
    let counterpart = std::path::PathBuf::from(
        std::env::var_os("CONDUIT_FIELD_BROWSER_PERSONAL_VIEWS")
            .expect("explicit checked browser capture directory"),
    );
    let source = super::super::configured::field_look_reuse_tests::source();
    let provider = crate::product_bases::fixture_surface_provider();
    let host = native_layout_host_advertisement(
        &"host/native-paired-look".into(),
        &"boot/native-paired-look".into(),
        1,
        &provider.entry,
        NativeMaskStorageProfile::Standard,
    );
    let preferences = NativeLookPreferences {
        local: Some(local(vec![
            LocalLookVisualOverride::TypeBodyPx(20),
            LocalLookVisualOverride::SpacePanelPx(0),
        ])),
        minimum_contrast: LocalLookContrast::Standard,
    };
    for component in ["todo-component-flat", "creche-component-columns"] {
        for theme in ["day", "night"] {
            let (_, configuration) = prepare_layout_component_document(
                &host,
                &source,
                &alloc::format!("{component}-{theme}"),
                NativeMaskStorageProfile::Standard,
            )
            .unwrap();
            for (name, key) in [
                ("environment", "CONDUIT_FIELD_ENVIRONMENT_FACE"),
                ("todo", "CONDUIT_FIELD_TODO_FACE"),
            ] {
                let bytes =
                    std::fs::read(std::env::var_os(key).expect("explicit retained Face export"))
                        .unwrap();
                let export: Export = serde_json::from_slice(&bytes).unwrap();
                let (mut scene, selection, _) =
                    NativeFaceMask::prepare_component_scene_with_preferences(
                        &configuration,
                        &export.face,
                        1024,
                        600,
                        false,
                        VisualUserConstraints {
                            forced_colors: false,
                            reduced_motion: true,
                            minimum_text_scale_percent: 100,
                            maximum_supported_text_scale_percent: 200,
                        },
                        &preferences,
                    )
                    .unwrap();
                assert_eq!(scene.face(), &export.face);
                let NativeVisualSelection::Local(resolved) = &selection else {
                    panic!("local resolution")
                };
                assert_eq!(resolved.author, configuration.requested_look().unwrap());
                let selected = selection.admitted().selected.clone().encode().unwrap();
                for browser in [
                    "todo-mask-flat-field-instrument",
                    "patchbay-mask-field-instrument",
                ] {
                    let bytes = std::fs::read(
                        counterpart
                            .join(name)
                            .join(alloc::format!("{browser}-{theme}-personal.json")),
                    )
                    .unwrap();
                    let view: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
                    assert_eq!(view["face_id"], export.face.identity.as_str());
                    assert_eq!(view["face_revision"], export.face.revision.to_string());
                    let browser_selected: Vec<u8> = serde_json::from_value(
                        view["component"]["selected_look"]["canonical_value"].clone(),
                    )
                    .unwrap();
                    assert_eq!(
                        selected, browser_selected,
                        "{name}/{component}/{browser}/{theme}"
                    );
                }
                for details in [false, true] {
                    scene.show_details(details);
                    for _ in 0..scene.pages() {
                        assert!(scene.frame().unwrap().hits.is_empty());
                        scene.turn_page(true);
                    }
                }
                assert_eq!(scene.face(), &export.face);
            }
        }
    }
}
