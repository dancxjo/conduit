//! CPU metric/geometry conformance. These fixtures confer no owner authority.
use super::*;
use alloc::{format, string::String, vec};
use conduit_face::*;
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};

fn specimens() -> Vec<Face> {
    let mut faces: Vec<Face> = [
        include_str!("../../../../proof/conformance/face-waist/domain-derived/prebirth-face.json"),
        include_str!(
            "../../../../proof/conformance/face-waist/domain-derived/lulled-body-face.json"
        ),
        include_str!(
            "../../../../proof/conformance/face-waist/domain-derived/awake-body-face.json"
        ),
        include_str!(
            "../../../../proof/conformance/face-waist/domain-derived/open-domain-face.json"
        ),
    ]
    .into_iter()
    .map(|value| serde_json::from_str(value).unwrap())
    .collect();
    // Historical canonical Home projection; launcher-action convergence is F2,
    // not a fact the layout can manufacture from its older HomeModel scene.
    let audit: serde_json::Value =
        serde_json::from_str(include_str!("../../../../proof/native-shell/fixtures.json")).unwrap();
    let home = audit["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["case"] == "home")
        .unwrap();
    faces.push(serde_json::from_value(home["face"].clone()).unwrap());
    faces
}
fn expression(scale: u16, density: VisualDensity) -> AdmittedVisualExpression {
    let mut expression = VisualExpression::phosphor();
    expression.density = density;
    admit_visual_expression(
        Ok(expression),
        VisualUserConstraints {
            forced_colors: false,
            reduced_motion: true,
            minimum_text_scale_percent: scale,
            maximum_supported_text_scale_percent: 200,
        },
    )
    .unwrap()
}
fn assert_frame(scene: &NativeFaceScene) -> usize {
    let frame = scene.frame().unwrap();
    let chrome = chrome::Chrome::prepare(scene.screen, scene.text_scale_percent).unwrap();
    assert_eq!(frame.face_id, &scene.face.identity);
    assert_eq!(frame.revision, scene.face.revision);
    assert!(frame.scene.commands().len() <= MAX_GRAPHICS_COMMANDS);
    for command in frame.scene.commands() {
        assert_eq!(command.clip_class(), GraphicsClipClass::FullyVisible);
        if command.kind == GraphicsCommandKind::Text {
            assert!(command.payload().len() <= MAX_GRAPHICS_TEXT_BYTES);
            let measured = TextLayout::new_scaled(
                command.payload(),
                command.text_role().into(),
                command.bounds.width,
                scene.text_scale_percent,
            )
            .unwrap()
            .finish_height();
            assert!(
                measured <= u32::from(command.bounds.height),
                "clipped {:?}: {} > {}",
                command.payload(),
                measured,
                command.bounds.height
            );
        }
    }
    for row in scene.rows().iter().filter(|row| row.page == scene.page) {
        assert!(row.bounds.y >= chrome.top as i16);
        assert!(
            u32::from(row.bounds.y as u16) + u32::from(row.bounds.height)
                <= u32::from(chrome.bottom)
        );
    }
    for hit in &frame.hits {
        let (action, argument) = scene
            .resolve(frame.face_id, frame.revision, hit.control)
            .unwrap();
        assert_eq!(action, hit.action);
        assert_eq!(argument, hit.argument);
        assert!(action.availability.is_available());
        assert!(frame.scene.commands().iter().any(|command| command.kind
            == GraphicsCommandKind::Rect
            && command.bounds == hit.bounds));
        assert_eq!(
            frame
                .hit_test(hit.bounds.x + 1, hit.bounds.y + 1)
                .unwrap()
                .control,
            hit.control
        );
    }
    frame.scene.commands().len()
}
fn all_pages(scene: &mut NativeFaceScene) -> usize {
    let mut maximum = 0;
    for _ in 0..scene.pages() {
        maximum = maximum.max(assert_frame(scene));
        scene.turn_page(true);
    }
    maximum
}
#[test]
fn every_canonical_page_fits_measured_chrome_at_supported_extents_and_scales() {
    for face in specimens() {
        for (width, height) in [(1280, 800), (640, 480), (320, 240)] {
            for scale in [100, 200] {
                for density in [VisualDensity::Compact, VisualDensity::Spacious] {
                    let mut scene = NativeFaceScene::prepare_visual_fallback(
                        face.clone(),
                        width,
                        height,
                        true,
                        &expression(scale, density),
                    )
                    .unwrap();
                    all_pages(&mut scene);
                    scene.show_details(true);
                    all_pages(&mut scene);
                    assert_eq!(scene.face(), &face);
                    let details: String =
                        scene.details.iter().map(|row| row.text.as_str()).collect();
                    for clause in plan_face_utterances(&face).unwrap().clauses {
                        assert!(details.contains(&clause.text), "lost exact clause");
                    }
                }
            }
        }
    }
}
#[test]
fn source_body_status_is_readable_without_native_lifecycle_inference() {
    let face = specimens().remove(1);
    let body = face
        .subjects
        .iter()
        .find(|subject| subject.role == FaceRole::Body)
        .unwrap();
    let scene = NativeFaceScene::prepare(face.clone(), 640, 480).unwrap();
    let statuses: String = scene
        .primary
        .iter()
        .filter(|row| row.role == GraphicsTextRole::Status)
        .map(|row| row.text.as_str())
        .collect();
    for text in face
        .text
        .iter()
        .filter(|text| text.subject == body.identity)
    {
        assert!(statuses.contains(&text.text));
    }
}
#[test]
fn unicode_and_unbroken_names_reconstruct_exactly_in_details() {
    let original = specimens().remove(1);
    let mut subjects = original.subjects;
    let name = format!(
        "Crèche {} {}",
        "e\u{301}—界\u{10ffff}".repeat(20),
        "x".repeat(300)
    );
    subjects
        .iter_mut()
        .find(|subject| subject.role == FaceRole::Body)
        .unwrap()
        .name = name.clone();
    let face = Face::new_with_semantics(
        original.revision,
        original.basis,
        subjects,
        original.relationships,
        original.properties,
        original.text,
        original.actions,
        original.disclosures,
    )
    .unwrap();
    for (width, height) in [(1280, 800), (640, 480), (320, 240)] {
        let mut scene = NativeFaceScene::prepare_visual_fallback(
            face.clone(),
            width,
            height,
            true,
            &expression(200, VisualDensity::Spacious),
        )
        .unwrap();
        all_pages(&mut scene);
        scene.show_details(true);
        assert!(
            scene
                .details
                .iter()
                .map(|row| row.text.as_str())
                .collect::<String>()
                .contains(&name)
        );
        all_pages(&mut scene);
        assert_eq!(scene.face(), &face);
    }
}
fn dense() -> (Face, PreparedComponentRecipe) {
    let original = specimens().remove(3);
    let argument = original
        .actions
        .iter()
        .find(|action| action.identity == "lesson/answer")
        .unwrap()
        .arguments[0]
        .clone();
    let subjects: Vec<_> = (0..8)
        .map(|index| FaceSubject {
            identity: format!("dense/{index}"),
            name: format!("Document {index}"),
            role: FaceRole::Document,
        })
        .collect();
    let mut second = argument.clone();
    second.name = "dense/second".into();
    second.value_name = "Second value".into();
    let mut actions: Vec<_> = subjects
        .iter()
        .enumerate()
        .map(|(index, subject)| FaceAction {
            identity: format!("dense/action/{index}"),
            intent: "fixture/dense@1".into(),
            target: subject.identity.clone(),
            name: format!("Action {index}"),
            arguments: vec![argument.clone(), second.clone()],
            disclosure: FaceDisclosureLevel::CurrentAction,
            availability: FaceActionAvailability::Available,
        })
        .collect();
    let mut unavailable = actions[0].clone();
    unavailable.identity = "dense/unavailable".into();
    unavailable.availability = FaceActionAvailability::Unavailable {
        reason_code: "fixture/not-offered".into(),
        explanation: "Not offered.".into(),
    };
    actions.push(unavailable);
    let disclosures = subjects
        .iter()
        .map(|subject| FaceDisclosure {
            subject: subject.identity.clone(),
            level: FaceDisclosureLevel::Primary,
        })
        .collect();
    let regions = subjects.iter().enumerate().map(|(index, subject)| {
        ComponentRegionInfo::new(
            index as u8,
            format!("region/{index}"),
            1,
            ComponentSubjectSelector::subject(subject.identity.clone()).unwrap(),
        )
        .unwrap()
    });
    let info = ComponentRecipeInfo::new(
        12,
        "fixture/dense@1".into(),
        2,
        720,
        BoundedSequence::try_from_iter(regions).unwrap(),
        1,
    )
    .unwrap()
    .into_structured()
    .unwrap();
    let face = Face::new_with_semantics(
        7,
        FaceBasis {
            body_id: None,
            wake_id: None,
            source_document_id: None,
            checked_plot_id: None,
            expanded_plot_id: None,
            plan_id: None,
            active_play_id: None,
            sign_ids: vec![],
        },
        subjects,
        vec![],
        vec![],
        vec![],
        actions,
        disclosures,
    )
    .unwrap();
    let recipe = prepare_component_recipe(info, &face).unwrap();
    (face, recipe)
}
#[test]
fn dense_regions_and_multiple_typed_arguments_admit_complete_bounded_pages() {
    let (face, recipe) = dense();
    let mut maximum = 0;
    for (width, height) in [(1280, 800), (640, 480), (320, 240), (1280, 16384)] {
        for scale in [100, 200] {
            let mut scene = NativeFaceScene::prepare_configured(
                face.clone(),
                width,
                height,
                true,
                &recipe,
                &expression(scale, VisualDensity::Compact),
            )
            .unwrap();
            assert!(scene.pages() > 1);
            maximum = maximum.max(all_pages(&mut scene));
            scene.show_details(true);
            maximum = maximum.max(all_pages(&mut scene));
            assert_eq!(scene.face(), &face);
        }
    }
    assert!(maximum > 40, "exercise the actual command boundary");
}
#[test]
fn unsupported_bounds_and_unmeasurable_notice_refuse_before_replacing_frame() {
    let face = specimens().remove(0);
    for (width, height) in [(319, 240), (320, 239), (16385, 800), (1280, 16385)] {
        assert!(matches!(
            NativeFaceScene::prepare(face.clone(), width, height),
            Err(FaceSceneError::InvalidExtent)
        ));
    }
    let mut scene = NativeFaceScene::prepare_visual_fallback(
        face,
        320,
        240,
        true,
        &expression(200, VisualDensity::Compact),
    )
    .unwrap();
    let before = scene.frame().unwrap().scene;
    assert_eq!(
        scene.set_local_notice(&"W".repeat(96)),
        Err(FaceSceneError::InputBound)
    );
    assert!(scene.local_notice.is_none());
    assert_eq!(scene.frame().unwrap().scene, before);
    scene.set_local_notice("Refused").unwrap();
    assert_frame(&scene);
    assert!(scene.clear_local_notice());
    assert_frame(&scene);
}

#[test]
fn read_only_chrome_is_measured_without_exposing_action_hits() {
    for face in specimens() {
        for (width, height) in [(1280, 800), (640, 480), (320, 240)] {
            let mut scene = NativeFaceScene::prepare_visual_fallback(
                face.clone(),
                width,
                height,
                false,
                &expression(200, VisualDensity::Spacious),
            )
            .unwrap();
            for details in [false, true] {
                scene.show_details(details);
                for _ in 0..scene.pages() {
                    assert_frame(&scene);
                    assert!(scene.frame().unwrap().hits.is_empty());
                    scene.turn_page(true);
                }
            }
            assert_eq!(scene.face(), &face);
        }
    }
}
#[test]
fn independent_diagram_geometry_refuses_expanded_chrome_with_readable_facts_fallback() {
    let face: Face = serde_json::from_str(include_str!(
        "../../../../proof/conformance/face-waist/domain-derived/patchbay-face.json"
    ))
    .unwrap();
    let mut scene = NativeFaceScene::prepare_visual_fallback(
        face.clone(),
        1280,
        800,
        true,
        &expression(200, VisualDensity::Compact),
    )
    .unwrap();
    scene.show_diagram(true);
    assert!(matches!(scene.frame(), Err(FaceSceneError::InvalidExtent)));
    scene.show_details(true);
    all_pages(&mut scene);
    assert_eq!(scene.face(), &face);
}

#[test]
fn minimum_viewport_reflows_a_permitted_two_column_recipe_to_one_column() {
    let (face, recipe) = dense();
    let info = ComponentRecipeInfo::new(
        0,
        "fixture/early-reflow@1".into(),
        2,
        280,
        recipe.recipe.regions().clone(),
        1,
    )
    .unwrap()
    .into_structured()
    .unwrap();
    let recipe = prepare_component_recipe(info, &face).unwrap();
    assert_eq!(recipe.geometry_for_width(320).unwrap().columns, 2);
    let mut scene = NativeFaceScene::prepare_configured(
        face.clone(),
        320,
        240,
        true,
        &recipe,
        &expression(200, VisualDensity::Spacious),
    )
    .unwrap();
    assert!(scene.primary.iter().all(|row| row.bounds.x == 0));
    all_pages(&mut scene);
    assert_eq!(scene.face(), &face);
}

#[path = "editor_geometry_tests.rs"]
mod editor_geometry_tests;
