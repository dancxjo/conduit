use super::*;
use alloc::vec::Vec;
use conduit_core::*;
use conduit_face::*;
use conduit_plot::rust_binding::NativeRustBinding;

fn reidentify(face: Face) -> Face {
    Face::new_with_semantics_and_temporal(
        face.revision,
        face.basis,
        face.subjects,
        face.relationships,
        face.properties,
        face.text,
        face.actions,
        face.disclosures,
        face.temporal_references,
        face.temporal_facts,
    )
    .unwrap()
    .with_composition(face.composition)
    .unwrap()
    .with_interaction_context(face.interaction_context)
    .unwrap()
}
fn passive_face(revision: u64, generation: u8) -> Face {
    let mut face = crate::native_face_scene::tests::face(revision);
    face.basis.active_play_id = Some("play/passive-reading".into());
    face.text = (0..24)
        .map(|_| FaceText {
            subject: "a-field".into(),
            text: if generation == 1 {
                "Current clock 12:34 and weather."
            } else {
                "Current clock 12:35 and weather."
            }
            .into(),
        })
        .collect();
    let image = ImageResource::new(
        ImageColorProfile::absent(),
        BoundedResourceRef {
            identity: ResourceSemanticIdentity::from_digest([generation; 32]),
            content_profile: kind_id("media/image-rgba8@1"),
            access_class: "conduit.resource/image-content@1".into(),
            extent: ResourceExtent {
                bytes: 16,
                items: Some(1),
            },
            lifetime: ResourceLifetime {
                version: ResourceVersionIdentity::from_digest([generation + 1; 32]),
                expires_at: None,
            },
        },
        ImagePixelExtent::new(2, 2).unwrap(),
        ImageFormat::rgba8(),
        "image/current".into(),
    )
    .unwrap()
    .into_structured()
    .unwrap();
    face.properties.push(FaceProperty {
        subject: "z-first".into(),
        name: "current-image".into(),
        value: FacePropertyValue::TypedValue {
            contract: CheckedValueContract::new(
                image.value_type().profile().unwrap().value_kind().clone(),
                MAX_FACE_VALUE_BYTES,
                Vec::new(),
            )
            .unwrap(),
            bytes: image.canonical_bytes().unwrap(),
        },
    });
    reidentify(face)
}
#[test]
fn passive_clock_text_and_image_refresh_preserves_page_details_and_fresh_focus_without_allocating()
{
    for details in [false, true] {
        let mut old = NativeFaceScene::prepare(passive_face(1, 1), 320, 240).unwrap();
        old.show_details(details);
        old.focus_next(false);
        if old.focus.is_none() {
            old.turn_page(true);
        }
        assert!(old.page > 0, "fixture must exercise a later page");
        let focus = old.focus.and_then(|c| control_key(&old, c));
        let mut fresh = NativeFaceScene::prepare(passive_face(2, 2), 320, 240).unwrap();
        assert_ne!(old.image, fresh.image);
        let exact = old.reading_position().unwrap();
        assert!(
            !fresh.restore_reading_position(exact),
            "strict API still rejects a new Face"
        );
        // Wrapped controls may span pages. Resolve the semantic anchor against
        // the fresh view, rather than retaining the old continuation's page.
        let expected_page = if let Some(key) = focus {
            let rows = if details {
                &fresh.details
            } else {
                &fresh.primary
            };
            rows.iter()
                .filter_map(|row| fresh.eligible(row))
                .find(|(control, _)| control_key(&fresh, *control) == Some(key))
                .unwrap()
                .1
        } else {
            old.page
        };
        let mut cookie = None;
        let mut restored = false;
        assert_eq!(
            crate::test_allocations::allocations(|| {
                cookie = old.passive_reading_position();
                restored = fresh.restore_passive_reading_position(cookie.unwrap());
            }),
            0
        );
        assert!(restored);
        assert_eq!(fresh.page, expected_page);
        assert_eq!(fresh.showing_details, details);
        assert_eq!(fresh.focus.and_then(|c| control_key(&fresh, c)), focus);
        if let Some(control) = fresh.focus {
            assert!(
                fresh
                    .resolve(&fresh.face.identity, fresh.face.revision, control)
                    .is_ok()
            );
            assert!(
                fresh
                    .resolve(&old.face.identity, old.face.revision, control)
                    .is_err()
            );
            let fresh_show = crate::native_face_scene::tests::fixture::show(&fresh.face);
            let arguments = fresh.face.actions[control.action]
                .arguments
                .iter()
                .map(|argument| FaceInteractionArgument {
                    name: argument.name.clone(),
                    value_kind: argument.contract.value_kind.as_str().into(),
                    value: b"Reading".to_vec(),
                })
                .collect();
            assert!(
                fresh
                    .interaction(
                        &fresh.face.identity,
                        fresh.face.revision,
                        control,
                        &fresh_show,
                        arguments,
                        1
                    )
                    .is_ok()
            );
            let old_show = crate::native_face_scene::tests::fixture::show(&old.face);
            assert!(
                fresh
                    .interaction(
                        &fresh.face.identity,
                        fresh.face.revision,
                        control,
                        &old_show,
                        Vec::new(),
                        1
                    )
                    .is_err()
            );
        }
    }
}
#[test]
fn passive_restore_refuses_changed_action_availability_context_plan_subject_or_geometry_purely() {
    let mut old = NativeFaceScene::prepare(passive_face(1, 1), 640, 480).unwrap();
    old.focus_next(false);
    let cookie = old.passive_reading_position().unwrap();
    for change in 0..7 {
        let mut face = passive_face(2, 2);
        match change {
            0 => {
                face.actions.remove(1);
            }
            1 => face.actions[1].intent = "test/different-meaning".into(),
            2 => {
                face.actions[1].availability = FaceActionAvailability::Unavailable {
                    reason_code: "stopped".into(),
                    explanation: "Not available".into(),
                }
            }
            3 => face.interaction_context.identity = "context/other".into(),
            4 => face.basis.plan_id = Some("plan/other".into()),
            5 => face.basis.active_play_id = Some("play/other".into()),
            _ => face.subjects[0].role = FaceRole::Collection,
        }
        let mut fresh = NativeFaceScene::prepare(reidentify(face), 640, 480).unwrap();
        let before = (fresh.page, fresh.showing_details, fresh.focus);
        assert_eq!(
            crate::test_allocations::allocations(|| assert!(
                !fresh.restore_passive_reading_position(cookie)
            )),
            0
        );
        assert_eq!((fresh.page, fresh.showing_details, fresh.focus), before);
    }
    let mut resized = NativeFaceScene::prepare(passive_face(2, 2), 320, 240).unwrap();
    assert!(!resized.restore_passive_reading_position(cookie));
    let mut draft = NativeFaceScene::prepare(passive_face(1, 1), 640, 480).unwrap();
    draft.input.submitted = true;
    assert!(draft.passive_reading_position().is_none());
}

#[test]
fn rotating_caption_reflow_resolves_focus_onto_the_new_control_page() {
    let mut old = NativeFaceScene::prepare(passive_face(1, 1), 320, 240).unwrap();
    let control = old.focus_next(false).unwrap();
    let old_page = old.page;
    let cookie = old.passive_reading_position().unwrap();
    let mut face = passive_face(2, 2);
    for text in &mut face.text {
        text.text="A photograph with a substantially longer title and photographer attribution wrapping across several measured lines on this narrow display.".into();
    }
    let mut fresh = NativeFaceScene::prepare(reidentify(face), 320, 240).unwrap();
    let fresh_page = fresh
        .primary
        .iter()
        .find(|row| row.control == Some(control))
        .unwrap()
        .page;
    assert_ne!(
        fresh_page, old_page,
        "fixture must move the stable control across pages"
    );
    assert_eq!(
        crate::test_allocations::allocations(|| assert!(
            fresh.restore_passive_reading_position(cookie)
        )),
        0
    );
    assert_eq!(fresh.focus, Some(control));
    assert_eq!(fresh.page, fresh_page);
    assert!(
        fresh
            .resolve(&fresh.face.identity, fresh.face.revision, control)
            .is_ok()
    );
}

#[test]
fn unfocused_passive_page_refuses_when_reflow_removes_that_page() {
    let mut old = NativeFaceScene::prepare(passive_face(1, 1), 320, 240).unwrap();
    while old.page + 1 < old.pages() {
        old.turn_page(true);
    }
    let cookie = old.passive_reading_position().unwrap();
    assert!(old.focus.is_none());
    assert!(old.page > 0);
    let mut face = passive_face(2, 2);
    face.text.truncate(1);
    let mut fresh = NativeFaceScene::prepare(reidentify(face), 320, 240).unwrap();
    assert!(fresh.pages() <= old.page);
    let before = (fresh.page, fresh.showing_details, fresh.focus);
    assert!(!fresh.restore_passive_reading_position(cookie));
    assert_eq!((fresh.page, fresh.showing_details, fresh.focus), before);
}
