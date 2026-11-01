//! Actual checked Mask preparation with independent Host-local proposal bytes.
use super::*;
use crate::owner_face_mask::{component, Acknowledgement, HostBasis, OwnerBrowserMask};
use conduit_face::{Face, LocalLookVisualOverride, LookVisualFacetOption};

fn face() -> Face {
    serde_json::from_str(include_str!(
        "../../../../../../proof/conformance/face-waist/domain-derived/open-domain-face.json"
    ))
    .unwrap()
}
fn proposal(values: Vec<LocalLookVisualOverride>) -> Vec<u8> {
    LocalLookInfo::new(
        LocalLookContrast::Standard,
        "personal/readable@1".into(),
        1,
        BoundedSequence::try_from_iter(values).unwrap(),
    )
    .unwrap()
    .encode()
    .unwrap()
}
fn prepare(
    face: &Face,
    constraints: component::VisualConstraints,
) -> Result<OwnerBrowserMask, String> {
    prepare_source(
        face,
        constraints,
        conduit_face::TODO_COMPONENT_MASK_SOURCE,
        "todo-mask-flat-compact",
    )
}
fn prepare_source(
    face: &Face,
    constraints: component::VisualConstraints,
    source: &str,
    root: &str,
) -> Result<OwnerBrowserMask, String> {
    let host = crate::installed_browser::membership_advertisement(
        "host/local-look".into(),
        "boot/local-look".into(),
    );
    let owner = crate::installed_browser::membership_advertisement(
        "owner/local-look".into(),
        "owner/local-look/boot".into(),
    );
    let line = crate::owner_face_mask::field_look_reuse_tests::line;
    let sent = line("face", &owner, &host);
    let returned = line("show", &host, &owner);
    let interaction = line("interaction", &host, &owner);
    let planned = conduit_browser_mask_offer::planned_owner_face_show_interaction_mask(
        &host,
        &owner,
        &sent,
        &returned,
        &interaction,
        source,
        root,
    )?;
    OwnerBrowserMask::prepare_planned(
        HostBasis {
            body_id: face.basis.body_id.clone().unwrap(),
            host_id: "host/local-look".into(),
            boot_id: "boot/local-look".into(),
            visual_constraints: Some(constraints),
        },
        face.clone(),
        planned,
        None,
        Some((
            sent.admitted_line(),
            returned.admitted_line(),
            Some(interaction.admitted_line()),
        )),
        1,
        false,
    )
}
fn user() -> component::VisualConstraints {
    component::VisualConstraints {
        minimum_text_scale_percent: 100,
        ..Default::default()
    }
}
fn selected(mask: &OwnerBrowserMask) -> LookInfo {
    LookInfo::decode(
        mask.component
            .as_ref()
            .unwrap()
            .selected_look
            .canonical_value(),
    )
    .unwrap()
}

#[test]
fn local_metrics_change_selected_resource_without_rewriting_planned_author_or_face() {
    let face = face();
    let baseline = prepare(&face, user()).unwrap();
    let mut mask = prepare(
        &face,
        component::VisualConstraints {
            local_look: Some(proposal(vec![
                LocalLookVisualOverride::TypeBodyPx(20),
                LocalLookVisualOverride::SpacePanelPx(0),
                LocalLookVisualOverride::RadiusControlPx(20),
                LocalLookVisualOverride::RadiusPanelPx(12),
            ])),
            ..user()
        },
    )
    .unwrap();
    assert_eq!(mask.planned, baseline.planned);
    assert_eq!(
        mask.component.as_ref().unwrap().configuration,
        baseline.component.as_ref().unwrap().configuration
    );
    let look = selected(&mask);
    let LookVisualFacetOption::Some(visual) = look.visual() else {
        panic!("visual")
    };
    assert_eq!(*visual.tokens().type_body_px(), 20);
    assert_eq!(*visual.tokens().space_panel_px(), 0);
    assert_eq!(*visual.tokens().radius_control_px(), 20);
    assert_eq!(*visual.tokens().radius_panel_px(), 12);
    assert_ne!(look.identity(), selected(&baseline).identity());
    let view = mask.view();
    assert_eq!(
        serde_json::to_value(&view.actions).unwrap(),
        serde_json::to_value(baseline.view().actions).unwrap()
    );
    mask.acknowledge(Acknowledgement {
        show_id: view.show_id,
        face_id: view.face_id,
        face_revision: view.face_revision,
    })
    .unwrap();
    let view = mask.view();
    assert_eq!(view.show_state, "available");
    assert!(!view.interactions_admitted);
    assert_eq!(mask.face, face);
    let provenance = serde_json::to_value(&mask.component.as_ref().unwrap().local_look).unwrap();
    assert_eq!(
        provenance["overridden_fields"],
        serde_json::json!([
            "type_body_px",
            "space_panel_px",
            "radius_control_px",
            "radius_panel_px"
        ])
    );
    assert_eq!(provenance["contrast"], "checked-nominal-tokens");
    assert!(
        serde_json::to_vec(&view).unwrap().len()
            <= crate::owner_face_mask::abi::conduit_browser_owner_face_output_capacity()
    );
}

#[test]
fn invalid_duplicate_and_insufficient_contrast_proposals_refuse_preparation() {
    let face = face();
    for bytes in [
        vec![0],
        vec![0; MAX_LOOK_BYTES + 1],
        proposal(vec![
            LocalLookVisualOverride::TypeBodyPx(20),
            LocalLookVisualOverride::TypeBodyPx(22),
        ]),
        proposal(vec![
            LocalLookVisualOverride::text_primary(20, 20, 20).unwrap()
        ]),
    ] {
        let result = prepare(
            &face,
            component::VisualConstraints {
                local_look: Some(bytes),
                ..user()
            },
        );
        let error = result.err().expect("invalid local proposal refuses");
        assert!(error.starts_with("browser local Look"), "{error}");
    }
}

#[test]
fn accessibility_floor_and_platform_colors_outrank_weaker_personal_values() {
    let face = face();
    let mask = prepare(
        &face,
        component::VisualConstraints {
            local_look: Some(proposal(vec![
                LocalLookVisualOverride::TextScalePercent(100),
                LocalLookVisualOverride::text_primary(20, 20, 20).unwrap(),
            ])),
            minimum_text_scale_percent: 200,
            forced_colors: true,
            enhanced_contrast: true,
            ..Default::default()
        },
    )
    .unwrap();
    let look = selected(&mask);
    let LookVisualFacetOption::Some(visual) = look.visual() else {
        panic!("visual")
    };
    assert_eq!(*visual.expression().text_scale_percent(), 100);
    let admitted = &mask.component.as_ref().unwrap().expression;
    assert_eq!(admitted.expression.text_scale_percent, 200);
    assert!(admitted.accessibility_overrides.text_scale);
    assert!(admitted.platform_forced_colors);
    let metadata = serde_json::to_value(&mask.component.as_ref().unwrap().local_look).unwrap();
    assert_eq!(metadata["required_contrast"], "enhanced");
    assert_eq!(metadata["contrast"], "platform-colors-required");
    assert_eq!(mask.face, face);
    assert!(!mask.view().interactions_admitted);
}

#[test]
fn unconfigured_mask_refuses_local_preferences_instead_of_ignoring_them() {
    let face = face();
    let result = OwnerBrowserMask::prepare_component_fixture(
        HostBasis {
            body_id: face.basis.body_id.clone().unwrap(),
            host_id: "host/no-author-look".into(),
            boot_id: "boot/no-author-look".into(),
            visual_constraints: Some(component::VisualConstraints {
                local_look: Some(proposal(vec![])),
                ..user()
            }),
        },
        face,
        1,
        false,
    );
    assert_eq!(
        result.err().as_deref(),
        Some("browser selected Mask has no configured author Look for local preferences")
    );
}

#[test]
#[ignore = "requires explicitly supplied retained real environmental and Todo Face exports"]
fn personal_field_looks_prepare_real_faces_without_changing_planned_author() {
    qualify_real_faces(false);
}

#[test]
#[ignore = "requires explicitly supplied retained real environmental and Todo Face exports"]
fn personal_corner_looks_prepare_real_faces_without_changing_planned_author() {
    qualify_real_faces(true);
}

fn qualify_real_faces(corners: bool) {
    #[derive(serde::Deserialize)]
    struct Export {
        face: Face,
    }
    let capture =
        std::env::var_os("CONDUIT_FIELD_BROWSER_CAPTURE_DIR").map(std::path::PathBuf::from);
    for (name, key) in [
        ("environment", "CONDUIT_FIELD_ENVIRONMENT_FACE"),
        ("todo", "CONDUIT_FIELD_TODO_FACE"),
    ] {
        let bytes =
            std::fs::read(std::env::var_os(key).expect("explicit retained Face export")).unwrap();
        let export: Export = serde_json::from_slice(&bytes).unwrap();
        for (source, root) in [
            (
                conduit_face::FIELD_INSTRUMENT_TODO_MASK_SOURCE,
                "todo-mask-flat-field-instrument",
            ),
            (
                conduit_face::FIELD_INSTRUMENT_PATCHBAY_MASK_SOURCE,
                "patchbay-mask-field-instrument",
            ),
        ] {
            for theme in ["day", "night"] {
                let root = format!("{root}-{theme}");
                let baseline = prepare_source(&export.face, user(), source, &root).unwrap();
                let mask = prepare_source(
                    &export.face,
                    component::VisualConstraints {
                        local_look: Some(proposal(if corners {
                            vec![
                                LocalLookVisualOverride::TypeBodyPx(20),
                                LocalLookVisualOverride::TypeSmallPx(18),
                                LocalLookVisualOverride::RadiusControlPx(20),
                                LocalLookVisualOverride::RadiusPanelPx(20),
                            ]
                        } else {
                            vec![
                                LocalLookVisualOverride::TypeBodyPx(20),
                                LocalLookVisualOverride::SpacePanelPx(0),
                            ]
                        })),
                        ..user()
                    },
                    source,
                    &root,
                )
                .unwrap();
                assert_eq!(mask.planned, baseline.planned);
                assert_eq!(
                    mask.component.as_ref().unwrap().configuration,
                    baseline.component.as_ref().unwrap().configuration
                );
                assert_eq!(mask.face, export.face);
                assert_eq!(
                    serde_json::to_value(mask.view().actions).unwrap(),
                    serde_json::to_value(baseline.view().actions).unwrap()
                );
                let look = selected(&mask);
                let LookVisualFacetOption::Some(visual) = look.visual() else {
                    panic!("visual")
                };
                assert_eq!(*visual.tokens().type_body_px(), 20);
                if corners {
                    assert_eq!(*visual.tokens().type_small_px(), 18);
                    assert_eq!(*visual.tokens().radius_control_px(), 20);
                    assert_eq!(*visual.tokens().radius_panel_px(), 20);
                } else {
                    assert_eq!(*visual.tokens().space_panel_px(), 0);
                }
                assert!(!mask.view().interactions_admitted);
                let view = serde_json::to_vec(&mask.view()).unwrap();
                assert!(
                    view.len()
                        <= crate::owner_face_mask::abi::conduit_browser_owner_face_output_capacity(
                        )
                );
                if let Some(directory) = &capture {
                    let directory = directory.join(name);
                    std::fs::create_dir_all(&directory).unwrap();
                    std::fs::write(
                        directory.join(format!(
                            "{root}-personal{}.json",
                            if corners { "-corners" } else { "" }
                        )),
                        view,
                    )
                    .unwrap();
                }
            }
        }
    }
}
