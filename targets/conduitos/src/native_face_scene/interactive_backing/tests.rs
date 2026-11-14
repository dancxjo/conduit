use super::*;
#[test]
fn ordinary_editing_rows_and_maximum_draft_fit_qualified_retained_bound() {
    let face = super::super::tests::face(1);
    let bounds = NativeFaceScene::standard_interactive_memory_bounds(&face).unwrap();
    let show = super::super::tests::fixture::show(&face);
    let mut scene = NativeFaceScene::prepare(face, 640, 480).unwrap();
    assert!(scene.standard_interactive_retained_backing_bytes().unwrap() <= bounds.retained_bytes);
    // The exact Source fixture permits 64 bytes on one active argument.
    // Exercise that ordinary retained state, plus the complete local notice.

    scene
        .focus_named(&FaceFocusRequest {
            action_id: "edit".into(),
            argument_name: Some("value".into()),
        })
        .unwrap();
    let key = conduit_human::KeyEvent::new(
        4,
        conduit_human::KeyTransition::Pressed,
        conduit_human::KeyModifiers::NONE,
    )
    .unwrap();
    for sequence in 1..=64 {
        assert!(matches!(
            scene.key(key, &show, sequence).unwrap(),
            FaceSceneInput::Changed
        ));
    }
    assert!(matches!(
        scene.key(key, &show, 65),
        Err(FaceSceneError::InputBound)
    ));
    scene.set_local_notice("A bounded local notice").unwrap();
    assert!(scene.standard_interactive_retained_backing_bytes().unwrap() <= bounds.retained_bytes);
    assert!(
        scene.frame().unwrap().retained_backing_bytes().unwrap() <= bounds.frame_transient_bytes
    );
    assert!(bounds.preparation_peak_bytes >= bounds.retained_bytes);
    let mut graph = scene.face().clone();
    graph.subjects[0].role = FaceRole::Gear;
    // Use the same semantic constructor so the refusal is for genuine graph
    // projection demand rather than a stale content identity.
    let graph = Face::new_with_semantics(
        graph.revision,
        graph.basis,
        graph.subjects,
        graph.relationships,
        graph.properties,
        graph.text,
        graph.actions,
        graph.disclosures,
    )
    .unwrap();
    assert_eq!(
        NativeFaceScene::standard_interactive_memory_bounds(&graph),
        Err(FaceSceneError::UnsupportedInput)
    );
}

#[test]
fn administrative_face_uses_linear_document_bounds_within_original_display_grant() {
    use conduit_face::*;
    let fixture = super::super::tests::face(1);
    let subjects = [
        FaceRole::Body,
        FaceRole::Plot,
        FaceRole::Status,
        FaceRole::Part,
        FaceRole::Part,
    ]
    .into_iter()
    .enumerate()
    .map(|(index, role)| FaceSubject {
        identity: format!("subject/{index}"),
        role,
        name: "n".repeat([16, 24, 15, 78, 78][index]),
    })
    .collect::<Vec<_>>();
    let relationships = (1..5)
        .map(|index| FaceRelationship {
            source: "subject/0".into(),
            target: format!("subject/{index}"),
            kind: FaceRelationshipKind::Contains,
        })
        .collect();
    let properties = (0..14)
        .map(|index| FaceProperty {
            subject: format!("subject/{}", index % 5),
            name: format!("fact/{index}"),
            value: if index < 7 {
                FacePropertyValue::Identity(format!("identity/{}", "a".repeat(64)))
            } else if index == 7 {
                FacePropertyValue::Count(4)
            } else {
                FacePropertyValue::Text("present".into())
            },
        })
        .collect();
    let actions = (0..9)
        .map(|index| FaceAction {
            identity: format!("action/{index}"),
            intent: "test/administrative-action".into(),
            target: format!("subject/{}", index % 5),
            name: "ordinary operation".into(),
            arguments: Vec::new(),
            disclosure: FaceDisclosureLevel::CurrentAction,
            availability: FaceActionAvailability::Available,
        })
        .collect();
    let disclosures = (0..5)
        .map(|index| FaceDisclosure {
            subject: format!("subject/{index}"),
            level: FaceDisclosureLevel::Primary,
        })
        .collect();
    let face = Face::new_with_semantics(
        1,
        fixture.basis,
        subjects,
        relationships,
        properties,
        alloc::vec![FaceText {
            subject: "subject/0".into(),
            text: "s".repeat(49)
        }],
        actions,
        disclosures,
    )
    .unwrap();
    let bounds = NativeFaceScene::standard_interactive_memory_bounds(&face).unwrap();
    let (primary, details) = document::prepare(&face, true).unwrap();
    let text_bound = document_text_bytes(&face).unwrap();
    assert!(primary.iter().map(|item| item.text.len()).sum::<usize>() <= text_bound.0);
    assert!(details.iter().map(|item| item.text.len()).sum::<usize>() <= text_bound.1);
    let utterance = conduit_face::plan_face_utterances(&face).unwrap();
    assert!(
        utterance
            .clauses
            .iter()
            .map(|c| c.text.len())
            .sum::<usize>()
            <= text_bound.1
    );
    let scene = NativeFaceScene::prepare(face, 1024, 600).unwrap();
    assert!(scene.standard_interactive_retained_backing_bytes().unwrap() <= bounds.retained_bytes);
    // Leaves most of the original 32MiB grant for fixed kernel/raster/metadata;
    // this is a regression ceiling, not a new capacity granted by the helper.
    assert!(
        bounds.lifetime_peak_bytes().unwrap() < 8 * 1024 * 1024,
        "{bounds:?}"
    );
}

#[test]
fn visual_fallback_funds_default_document_overlap_and_selected_scale_density() {
    use conduit_face::{
        VisualDensity, VisualExpression, VisualTypography, VisualUserConstraints,
        admit_visual_expression,
    };
    let face = super::super::tests::face(1);
    let standard = NativeFaceScene::standard_interactive_memory_bounds(&face).unwrap();
    let mut bound = None;
    assert_eq!(
        crate::test_allocations::allocations(|| {
            bound = Some(NativeFaceScene::visual_fallback_memory_bounds(&face).unwrap());
        }),
        0,
        "preflight precedes the expression/Scene allocation lease"
    );
    let bound = bound.unwrap();
    assert_eq!(bound.retained_bytes, standard.retained_bytes);
    assert_eq!(bound.input_transient_bytes, standard.input_transient_bytes);
    assert_eq!(bound.frame_transient_bytes, standard.frame_transient_bytes);
    let original = NativeFaceScene::prepare(face.clone(), 1024, 600).unwrap();
    let mut actual_documents = 0;
    for rows in [&original.primary, &original.details] {
        actual_documents += rows.capacity() * size_of::<layout::Row>();
        actual_documents += rows.iter().map(|row| row.text.capacity()).sum::<usize>();
    }
    assert!(bound.preparation_peak_bytes >= standard.preparation_peak_bytes + actual_documents);
    for scale in [100, 150, 200] {
        for density in [VisualDensity::Compact, VisualDensity::Spacious] {
            for typography in [VisualTypography::Editorial, VisualTypography::Engineering] {
                let mut value = VisualExpression::phosphor();
                value.text_scale_percent = scale;
                value.density = density;
                value.typography = typography;
                let expression = admit_visual_expression(
                    Ok(value),
                    VisualUserConstraints {
                        forced_colors: false,
                        reduced_motion: true,
                        minimum_text_scale_percent: 100,
                        maximum_supported_text_scale_percent: 200,
                    },
                )
                .unwrap();
                let mut scene = NativeFaceScene::prepare_visual_fallback(
                    face.clone(),
                    1024,
                    600,
                    true,
                    &expression,
                )
                .unwrap();
                assert_eq!(scene.face(), &face);
                assert_eq!(scene.text_scale_percent, scale);
                assert!(
                    scene.standard_interactive_retained_backing_bytes().unwrap()
                        <= bound.retained_bytes
                );
                assert!(
                    scene.frame().unwrap().retained_backing_bytes().unwrap()
                        <= bound.frame_transient_bytes
                );
                scene
                    .focus_named(&FaceFocusRequest {
                        action_id: "edit".into(),
                        argument_name: Some("value".into()),
                    })
                    .unwrap();
                let show = super::super::tests::fixture::show(&face);
                let key = conduit_human::KeyEvent::new(
                    4,
                    conduit_human::KeyTransition::Pressed,
                    conduit_human::KeyModifiers::NONE,
                )
                .unwrap();
                assert!(matches!(
                    scene.key(key, &show, 1).unwrap(),
                    FaceSceneInput::Changed
                ));
                assert!(
                    scene.standard_interactive_retained_backing_bytes().unwrap()
                        <= bound.retained_bytes
                );
                assert!(
                    scene.frame().unwrap().retained_backing_bytes().unwrap()
                        <= bound.frame_transient_bytes
                );
            }
        }
    }
    assert_eq!(
        NativeFaceScene::standard_interactive_memory_bounds(&face).unwrap(),
        standard
    );
}
