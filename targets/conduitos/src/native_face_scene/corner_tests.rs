//! Local Look geometry reaches native drawing without moving semantic hits.
use super::*;
use conduit_face::{
    LocalLookContrast, LocalLookInfo, LocalLookVisualOverride, VisualUserConstraints,
    resolve_local_visual_look,
};
use conduit_plot::rust_binding::BoundedSequence;

fn selected(control: u16, panel: u16) -> conduit_face::AdmittedVisualLook {
    let author = authored_look_metrics_tests::metric_proposal_with_gap(18, 16, 150, 4);
    let local = LocalLookInfo::new(
        LocalLookContrast::Standard,
        "personal/corners@1".into(),
        1,
        BoundedSequence::try_from_iter([
            LocalLookVisualOverride::RadiusControlPx(control),
            LocalLookVisualOverride::RadiusPanelPx(panel),
        ])
        .unwrap(),
    )
    .unwrap();
    resolve_local_visual_look(
        &author,
        &local,
        VisualUserConstraints {
            forced_colors: false,
            reduced_motion: true,
            minimum_text_scale_percent: 100,
            maximum_supported_text_scale_percent: 200,
        },
        LocalLookContrast::Standard,
    )
    .unwrap()
    .admitted
}
#[test]
fn selected_corners_reach_generic_configured_and_checked_scenes_without_moving_hits() {
    let face = structured_source_tests::todo();
    let structure = structured_source_tests::structure(
        conduit_face::TODO_LAYOUT_MASK_SOURCE,
        "todo-component-grouped",
        &face,
    );
    let mut previous = None;
    for (control, panel) in [(0, 32), (32, 0), (4, 12)] {
        let look = selected(control, panel);
        let mut all_geometry = Vec::new();
        let mut controls = 0;
        let mut panels = 0;
        for mut scene in [
            NativeFaceScene::prepare_visual_look(face.clone(), 1024, 600, true, &look).unwrap(),
            NativeFaceScene::prepare_configured_look(
                face.clone(),
                1024,
                600,
                true,
                structure.recipe(),
                &look,
            )
            .unwrap(),
            NativeFaceScene::prepare_structured_look(&structure, 1024, 600, true, &look).unwrap(),
        ] {
            let mut geometry = Vec::new();
            for _ in 0..scene.pages() {
                let frame = scene.frame().unwrap();
                for hit in &frame.hits {
                    geometry.push((scene.page(), hit.bounds, hit.control));
                }
                for row in scene.rows().iter().filter(|row| row.page == scene.page()) {
                    if row.control.is_some() && row.checkbox.is_none() {
                        let command = frame
                            .scene
                            .commands()
                            .iter()
                            .find(|c| {
                                c.bounds == row.bounds
                                    && matches!(
                                        c.style,
                                        GraphicsShapeStyle::RoundedFill
                                            | GraphicsShapeStyle::RoundedStroke
                                    )
                            })
                            .unwrap();
                        assert_eq!(command.rect_metrics(), Some((control as u8, 1)));
                        controls += 1;
                    }
                    if let Some(bounds) = row.panel_bounds {
                        let command = frame
                            .scene
                            .commands()
                            .iter()
                            .find(|c| c.bounds == bounds && c.paint == GraphicsPaintRole::Surface)
                            .unwrap();
                        assert_eq!(command.rect_metrics(), Some((panel as u8, 1)));
                        panels += 1;
                    }
                }
                for command in frame.scene.commands().iter().filter(|c| {
                    matches!(
                        c.style,
                        GraphicsShapeStyle::RoundedFill | GraphicsShapeStyle::RoundedStroke
                    )
                }) {
                    assert!(command.rect_metrics().is_some());
                }
                let bytes = frame.scene.encode();
                assert_eq!(
                    GraphicsScene::decode(&bytes[..frame.scene.encoded_len()]).unwrap(),
                    frame.scene
                );
                assert_eq!(scene.face(), &face);
                scene.turn_page(true);
            }
            all_geometry.push(geometry);
        }
        assert!(controls > 0 && panels > 0);
        if let Some(expected) = &previous {
            assert_eq!(&all_geometry, expected);
        }
        previous = Some(all_geometry);
    }
}
#[test]
fn unselected_scenes_keep_legacy_geometry_and_diagram_panels_use_the_selected_radius() {
    let face: Face = serde_json::from_str(include_str!(
        "../../../../proof/conformance/face-waist/domain-derived/patchbay-face.json"
    ))
    .unwrap();
    let legacy = NativeFaceScene::prepare(face.clone(), 1024, 600).unwrap();
    assert!(
        legacy
            .frame()
            .unwrap()
            .scene
            .commands()
            .iter()
            .all(|c| c.rect_metrics().is_none())
    );
    let mut expected = None;
    for radius in [0, 32] {
        let mut scene = NativeFaceScene::prepare_visual_look(
            face.clone(),
            1024,
            600,
            true,
            &selected(4, radius),
        )
        .unwrap();
        scene.show_diagram(true);
        assert!(scene.showing_diagram());
        let frame = scene.frame().unwrap();
        let panels: Vec<_> = frame
            .scene
            .commands()
            .iter()
            .filter(|c| c.style == GraphicsShapeStyle::RoundedStroke)
            .collect();
        assert!(!panels.is_empty());
        assert!(
            panels
                .iter()
                .all(|c| c.rect_metrics() == Some((radius as u8, 1)))
        );
        let bounds: Vec<_> = panels.iter().map(|c| c.bounds).collect();
        if let Some(old) = &expected {
            assert_eq!(&bounds, old);
        }
        expected = Some(bounds);
        assert_eq!(scene.face(), &face);
    }
}

#[test]
fn active_document_editor_uses_control_radius_without_changing_document_or_extent() {
    let mut face = tests::face(1);
    face.properties.push(conduit_face::FaceProperty {
        subject: "z-first".into(),
        name: "text-editing-active".into(),
        value: conduit_face::FacePropertyValue::Flag(true),
    });
    let face = Face::new_with_semantics(
        face.revision,
        face.basis,
        face.subjects,
        face.relationships,
        face.properties,
        face.text,
        face.actions,
        face.disclosures,
    )
    .unwrap();
    let mut expected = None;
    for radius in [0, 32] {
        let scene = NativeFaceScene::prepare_visual_look(
            face.clone(),
            1024,
            600,
            true,
            &selected(radius, 4),
        )
        .unwrap();
        let frame = scene.frame().unwrap();
        let editor = frame
            .scene
            .commands()
            .iter()
            .find(|c| {
                c.paint == GraphicsPaintRole::Focus && c.style == GraphicsShapeStyle::RoundedStroke
            })
            .unwrap();
        assert_eq!(editor.rect_metrics(), Some((radius as u8, 1)));
        if let Some(bounds) = expected {
            assert_eq!(editor.bounds, bounds);
        }
        expected = Some(editor.bounds);
        assert_eq!(scene.face(), &face);
    }
}
