//! Software Show fixtures only; no physical input or scanout claim.
#![cfg(feature = "native-compositor")]
extern crate alloc;
#[allow(dead_code)]
#[path = "../src/native_face_scene/fixture.rs"]
mod fixture;

use conduit_body::{Body, BodyWorkset, ResidentPlot};
use conduit_face::{
    BodyFace, Face, FaceContext, FaceFocus, FaceNames, FaceResidentPlotName, LayoutRect,
    VisualDensity, VisualExpression, VisualUserConstraints, admit_visual_expression,
};
use conduitos::native_face_scene::{FaceSceneError, FaceSceneInput, NativeFaceScene};

fn library() -> Face {
    let plots = ["clock", "notebook", "thermostat"].map(|name| {
        ResidentPlot::new(
            format!("source/{name}").into(),
            format!("checked/{name}").into(),
        )
    });
    let body = Body::born_with_plots(
        BodyWorkset::from_plots(plots.to_vec()).unwrap(),
        1,
        "sign/born".into(),
    )
    .unwrap();
    let names: Vec<_> = body
        .workset
        .plots()
        .iter()
        .zip(["House clock", "House notebook", "House thermostat"])
        .map(|(plot, name)| FaceResidentPlotName {
            source_document_id: &plot.source_document_id,
            checked_plot_id: &plot.checked_plot_id,
            name,
        })
        .collect();
    BodyFace::project_with_names(
        &body,
        None,
        1,
        FaceContext::Library,
        FaceFocus::Body,
        vec![],
        FaceNames {
            body_name: Some("House"),
            resident_plots: &names,
        },
    )
    .unwrap()
    .face
}
fn center(bounds: LayoutRect) -> (i16, i16) {
    (
        bounds.x + (bounds.width / 2) as i16,
        bounds.y + (bounds.height / 2) as i16,
    )
}

#[test]
fn touch_pages_reach_named_plots_without_submitting_body_actions() {
    for (width, height, scale, interactive) in [
        (1024, 600, 150, true),
        (320, 240, 100, true),
        (320, 240, 150, true),
        (640, 480, 200, false),
    ] {
        let face = library();
        let show = fixture::show(&face);
        let mut expression = VisualExpression::phosphor();
        expression.density = VisualDensity::Spacious;
        let expression = admit_visual_expression(
            Ok(expression),
            VisualUserConstraints {
                forced_colors: false,
                reduced_motion: true,
                minimum_text_scale_percent: scale,
                maximum_supported_text_scale_percent: 200,
            },
        )
        .unwrap();
        let mut scene = NativeFaceScene::prepare_visual_fallback(
            face.clone(),
            width,
            height,
            interactive,
            &expression,
        )
        .unwrap();
        assert!(scene.pages() > 1);
        let controls = scene.page_controls().unwrap().unwrap();
        assert!(!controls.can_previous);
        let (x, y) = center(controls.previous);
        assert!(matches!(
            scene.pointer(x, y, &show, 1),
            Ok(FaceSceneInput::Unchanged)
        ));
        let mut seen = [false; 3];
        let pages = scene.pages();
        for index in 0..pages {
            let frame = scene.frame().unwrap();
            for (found, name) in
                seen.iter_mut()
                    .zip(["House clock", "House notebook", "House thermostat"])
            {
                *found |= frame
                    .scene
                    .commands()
                    .iter()
                    .any(|c| c.payload().contains(name));
            }
            let controls = scene.page_controls().unwrap().unwrap();
            let (x, y) = center(controls.next);
            assert!(
                frame.hit_test(x, y).is_none(),
                "local paging is not a Face action"
            );
            assert!(
                frame
                    .scene
                    .commands()
                    .iter()
                    .any(|c| c.payload() == "Next page" || c.payload() == "Next")
            );
            let expected = index + 1 < pages;
            assert_eq!(controls.can_next, expected);
            let outcome = scene.pointer(x, y, &show, index as u64 + 2).unwrap();
            assert!(if expected {
                matches!(outcome, FaceSceneInput::Changed)
            } else {
                matches!(outcome, FaceSceneInput::Unchanged)
            });
            assert!(!scene.has_pending_input());
            assert_eq!(scene.face(), &face);
        }
        assert!(seen.into_iter().all(|found| found));
        for index in (1..pages).rev() {
            let (x, y) = center(scene.page_controls().unwrap().unwrap().previous);
            assert!(matches!(
                scene.pointer(x, y, &show, 100 + index as u64),
                Ok(FaceSceneInput::Changed)
            ));
            assert_eq!(scene.page(), index - 1);
        }
    }
}

#[test]
fn stale_show_cannot_turn_a_local_page() {
    let face = library();
    let mut show = fixture::show(&face);
    let mut scene = NativeFaceScene::prepare(face, 1024, 600).unwrap();
    let (x, y) = center(scene.page_controls().unwrap().unwrap().next);
    show.face_revision += 1;
    assert!(matches!(
        scene.pointer(x, y, &show, 1),
        Err(FaceSceneError::StaleFace)
    ));
    assert_eq!(scene.page(), 0);
}

#[test]
fn replacement_keeps_exact_reading_page_without_retaining_old_show() {
    let face = library();
    let mut first = NativeFaceScene::prepare(face.clone(), 1024, 600).unwrap();
    first.turn_page(true);
    let page = first.page();
    assert!(page > 0);
    let position = first.reading_position().unwrap();
    assert!(!core::mem::needs_drop::<
        conduitos::native_face_scene::NativeReadingPosition,
    >());
    drop(first);
    let mut fresh = NativeFaceScene::prepare(face.clone(), 1024, 600).unwrap();
    assert!(fresh.restore_reading_position(position));
    assert_eq!(fresh.page(), page);
    assert_eq!(fresh.face(), &face);
    assert!(!fresh.has_pending_input());
    let mut changed_extent = NativeFaceScene::prepare(face.clone(), 640, 480).unwrap();
    assert!(!changed_extent.restore_reading_position(position));
    assert_eq!(changed_extent.page(), 0);
    let changed_face = Face::new_with_semantics(
        face.revision + 1,
        face.basis,
        face.subjects,
        face.relationships,
        face.properties,
        face.text,
        face.actions,
        face.disclosures,
    )
    .unwrap();
    let mut changed = NativeFaceScene::prepare(changed_face, 1024, 600).unwrap();
    assert!(!changed.restore_reading_position(position));
    assert_eq!(changed.page(), 0);
}
