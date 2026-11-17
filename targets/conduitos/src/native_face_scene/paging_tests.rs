//! Local pager CPU proof through the existing sampled tap recognizer.
use super::tests::fixture;
use super::*;
use alloc::{format, vec};
use conduit_body::{Body, BodyWorkset, ResidentPlot};
use conduit_face::*;

fn library() -> Face {
    let plots = (0..12).map(|index| {
        ResidentPlot::new(
            format!("source/library-{index}").into(),
            format!("checked/library-{index}").into(),
        )
    });
    let body = Body::born_with_plots(
        BodyWorkset::from_plots(plots).unwrap(),
        1,
        "sign/born".into(),
    )
    .unwrap();
    let labels: Vec<_> = (0..12)
        .map(|index| {
            let name = ["House clock", "House notebook", "House thermostat"][index % 3];
            format!("{name} {index}: an ordinary resident plot with a complete descriptive name that remains available throughout local reading")
        })
        .collect();
    let names: Vec<_> = body
        .workset
        .plots()
        .iter()
        .zip(&labels)
        .map(|(plot, name)| FaceResidentPlotName {
            source_document_id: &plot.source_document_id,
            checked_plot_id: &plot.checked_plot_id,
            name,
        })
        .collect();
    let face = BodyFace::project_with_names(
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
    .face;
    // Exercise a complete primary document independently of the owner's
    // current policy for placing resident plots in selected details.
    Face::new_with_semantics(
        face.revision,
        face.basis,
        face.subjects,
        face.relationships,
        face.properties,
        face.text,
        face.actions,
        vec![],
    )
    .unwrap()
}
fn tap(
    scene: &mut NativeFaceScene,
    x: i16,
    y: i16,
    show: &MaskShow,
    sequence: u64,
) -> Result<FaceSceneInput, FaceSceneError> {
    let page = scene.page();
    scene.pointer_sample(x, y, true, true, show, sequence * 2)?;
    assert_eq!(scene.page(), page, "press alone cannot turn a page");
    scene.pointer_sample(x, y, false, false, show, sequence * 2 + 1)
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
        let controls: NativePageControls = scene.page_controls().unwrap().unwrap();
        assert!(!controls.can_previous);
        let (x, y) = center(controls.previous);
        assert!(matches!(
            tap(&mut scene, x, y, &show, 1),
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
            let outcome = tap(&mut scene, x, y, &show, index as u64 + 2).unwrap();
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
                tap(&mut scene, x, y, &show, 100 + index as u64),
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
        tap(&mut scene, x, y, &show, 1),
        Err(FaceSceneError::StaleFace)
    ));
    assert_eq!(scene.page(), 0);
}

#[test]
fn sampled_page_tap_commits_only_on_release_and_cancellation_requires_a_new_edge() {
    let face = library();
    let show = fixture::show(&face);
    let mut scene = NativeFaceScene::prepare(face, 1024, 600).unwrap();
    let (x, y) = center(scene.page_controls().unwrap().unwrap().next);
    scene.pointer_sample(x, y, true, true, &show, 1).unwrap();
    assert_eq!(scene.page(), 0);
    assert!(
        scene
            .frame()
            .unwrap()
            .scene
            .commands()
            .iter()
            .any(|command| {
                command.payload() == "Next page" && command.paint == GraphicsPaintRole::Focus
            })
    );
    scene.pointer_sample(x, y, true, false, &show, 2).unwrap();
    assert_eq!(scene.page(), 0);
    // Exceed the existing recognizer's movement threshold without leaving the
    // large footer target, then return while the same physical press is held.
    scene
        .pointer_sample(x + 9, y, true, false, &show, 3)
        .unwrap();
    scene.pointer_sample(x, y, true, false, &show, 4).unwrap();
    scene.pointer_sample(x, y, false, false, &show, 5).unwrap();
    assert_eq!(scene.page(), 0);
    assert!(matches!(
        tap(&mut scene, x, y, &show, 6).unwrap(),
        FaceSceneInput::Changed
    ));
    assert_eq!(scene.page(), 1);
    scene.pointer_sample(x, y, false, false, &show, 14).unwrap();
    assert_eq!(
        scene.page(),
        1,
        "duplicate release cannot turn another page"
    );
    assert!(!scene.has_pending_input());
}

#[test]
fn restored_reading_and_stale_show_cannot_reactivate_a_held_page_tap() {
    let face = library();
    let show = fixture::show(&face);
    let mut scene = NativeFaceScene::prepare(face, 1024, 600).unwrap();
    let position = scene.reading_position().unwrap();
    let (x, y) = center(scene.page_controls().unwrap().unwrap().next);
    scene.pointer_sample(x, y, true, true, &show, 1).unwrap();
    assert!(scene.restore_reading_position(position));
    scene.pointer_sample(x, y, true, false, &show, 2).unwrap();
    scene.pointer_sample(x, y, false, false, &show, 3).unwrap();
    assert_eq!(scene.page(), 0);
    scene.pointer_sample(x, y, true, true, &show, 4).unwrap();
    let mut stale = show.clone();
    stale.face_revision += 1;
    assert!(matches!(
        scene.pointer_sample(x, y, false, false, &stale, 5),
        Err(FaceSceneError::StaleFace)
    ));
    scene.pointer_sample(x, y, true, false, &show, 6).unwrap();
    scene.pointer_sample(x, y, false, false, &show, 7).unwrap();
    assert_eq!(scene.page(), 0);
    tap(&mut scene, x, y, &show, 8).unwrap();
    assert_eq!(scene.page(), 1);
}

#[test]
fn pager_painting_keeps_the_existing_document_command_capacity() {
    let mut scene = NativeFaceScene::prepare(library(), 640, 480).unwrap();
    let rows: Vec<_> = scene
        .primary
        .iter()
        .map(|row| (row.page, row.bounds, row.text.clone()))
        .collect();
    for page in [0, scene.pages() - 1] {
        scene.page = page;
        let mut graphics = GraphicsScene::empty();
        scene
            .append_page_controls(
                &mut graphics,
                scene.page_controls().unwrap().unwrap(),
                false,
                false,
            )
            .unwrap();
        assert_eq!(4 + graphics.commands().len(), chrome::COMMANDS);
        assert!(scene.frame().unwrap().scene.commands().len() <= MAX_GRAPHICS_COMMANDS);
    }
    for (row, original) in scene.primary.iter().zip(rows) {
        assert_eq!(
            (row.page, row.bounds, row.text.as_str()),
            (original.0, original.1, original.2.as_str())
        );
    }
}

#[test]
fn paged_local_look_guidance_keeps_controls_and_the_original_command_bound() {
    let mut scene = NativeFaceScene::prepare(library(), 640, 480).unwrap();
    let controls = scene.page_controls().unwrap().unwrap();
    let before = scene.frame().unwrap().scene.commands().len();
    scene.set_local_creche_preferences(true);
    let frame = scene.frame().unwrap();
    assert!(
        frame
            .scene
            .commands()
            .last()
            .unwrap()
            .payload()
            .contains("F6")
    );
    assert_eq!(frame.scene.commands().len(), before);
    assert_eq!(scene.page_controls().unwrap().unwrap(), controls);
}
