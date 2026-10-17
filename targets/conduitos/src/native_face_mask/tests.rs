//! Product-path Face-to-Mask scanout and interaction proof.

use super::*;
use alloc::vec;
use conduit_birth_plot::{BirthDraft, BirthPlotChoice};
use conduit_face::{BodyFace, FaceContext, FaceFocus, ManifestationLifecycle};
use conduit_human::{KeyModifiers, KeyTransition};

#[test]
fn compositor_refusal_keeps_its_specific_machine_reason() {
    assert_eq!(
        NativeFaceMaskError::Compositor(
            crate::native_compositor::NativeCompositorError::SurfaceNotAdmitted
        )
        .as_str(),
        "compositor-surface-not-admitted"
    );
}

struct CountingDisplay {
    writes: u32,
    fail: bool,
}

impl PixelTarget for CountingDisplay {
    fn format(&self) -> crate::display::DisplayFormat {
        crate::display::DisplayFormat {
            width: 640,
            height: 480,
            pitch: 2560,
            bits_per_pixel: 32,
            red_shift: 16,
            green_shift: 8,
            blue_shift: 0,
        }
    }

    fn write_pixel(&mut self, _: u32, _: u32, _: u32) -> Result<(), crate::display::DisplayError> {
        if self.fail {
            return Err(crate::display::DisplayError::Lost);
        }
        self.writes += 1;
        Ok(())
    }
}

fn fixture() -> (NativeFaceMask, BirthDraft) {
    let provider = crate::product_bases::fixture_surface_provider();
    let service = NativeFaceMask::prepare(
        "host/native".into(),
        "boot/native".into(),
        OfferGeneration(1),
        "build",
        provider.entry.base_id.clone(),
        "surface/native-face",
        &provider,
    )
    .unwrap();
    let choices = crate::native_workset::profile()
        .installed()
        .iter()
        .map(|plot| BirthPlotChoice {
            title: plot.title().into(),
            search_text: plot.title().into(),
            plot: crate::native_workset::resident(*plot).unwrap(),
            refusal: None,
            selected: true,
        })
        .collect();
    let draft = BirthDraft::new("550e8400-e29b-41d4-a716-446655440000".into(), choices).unwrap();
    (service, draft)
}

#[test]
fn birth_face_crosses_producer_mask_scanout_and_exact_interaction_fore() {
    let (mut service, mut draft) = fixture();
    let basis = service.birth_basis("arrival/one");
    let face = draft.face(&basis).unwrap();
    let mut display = CountingDisplay {
        writes: 0,
        fail: false,
    };
    let composition = service.present(face.clone(), 1, 1, &mut display).unwrap();
    let publication = service.publication().unwrap();
    assert_eq!(publication.face_id, face.identity.as_str());
    assert!(publication.kernel_signs > 0);
    assert!(display.writes > 0);
    assert_eq!(composition.face_id, face.identity);
    let show = service.show().unwrap().clone();
    assert_eq!(show.show.lifecycle, ManifestationLifecycle::Available);
    assert!(service.route_keyboard().unwrap());
    let writes = display.writes;
    let tab = KeyEvent::new(43, KeyTransition::Pressed, KeyModifiers::NONE).unwrap();
    assert!(matches!(
        service.key(tab, 1, &mut display).unwrap(),
        NativeFaceMaskInput::Redrawn(_)
    ));
    assert!(display.writes > writes);
    assert_eq!(service.show().unwrap().show_id, show.show_id);
    let action = face
        .actions
        .iter()
        .position(|action| action.identity == "creche.suggest")
        .unwrap();
    let input = service
        .scene()
        .unwrap()
        .interaction(
            &face.identity,
            face.revision,
            crate::native_face_scene::FaceControl {
                action,
                argument: None,
            },
            &show,
            vec![],
            1,
        )
        .unwrap();
    let correlation = service.submit(input.clone()).unwrap();
    assert_eq!(correlation.show_id, show.show_id);
    assert_eq!(correlation.interaction, input);
    assert!(service.show().is_none());
    assert!(matches!(
        draft.apply_face_interaction(&basis, &show, &input),
        Ok(conduit_birth_plot::BirthActionOutcome::Changed)
    ));
    let next = draft.face(&basis).unwrap();
    assert_ne!(next.revision, face.revision);
    service.present(next, 2, 2, &mut display).unwrap();
    assert!(service.show().is_some());
}

#[test]
fn failed_scanout_does_not_publish_a_mask_show() {
    let (mut service, draft) = fixture();
    let mut display = CountingDisplay {
        writes: 0,
        fail: true,
    };
    let face = draft.face(&service.birth_basis("arrival/two")).unwrap();
    assert!(matches!(
        service.present(face, 1, 1, &mut display),
        Err(NativeFaceMaskError::Compositor(_))
    ));
    assert!(service.show().is_none());
}

#[test]
fn owner_body_face_keeps_exact_identity_and_has_read_only_mask_navigation() {
    let (mut service, _) = fixture();
    let body = conduit_body::Body::born(
        "source/shared-clock".into(),
        "checked/shared-clock".into(),
        1,
        "sign/owner-born".into(),
    )
    .unwrap();
    let face = BodyFace::project(
        &body,
        None,
        7,
        FaceContext::Overview,
        FaceFocus::Body,
        vec![],
    )
    .unwrap()
    .face;
    assert!(
        face.subjects
            .iter()
            .all(|subject| subject.role != conduit_face::FaceRole::Host)
    );
    let wake_action = face
        .actions
        .iter()
        .find(|action| action.intent == "conduit.intent/wake@1")
        .unwrap();
    let mut display = CountingDisplay {
        writes: 0,
        fail: false,
    };
    let composition = service
        .present_read_only(face.clone(), 1, 1, &mut display)
        .unwrap();
    assert_eq!(composition.face_id, face.identity);
    assert_eq!(service.scene().unwrap().face(), &face);
    assert!(service.show().is_some());
    assert!(service.route_keyboard().unwrap());
    let enter = KeyEvent::new(40, KeyTransition::Pressed, KeyModifiers::NONE).unwrap();
    assert!(matches!(
        service.key(enter, 1, &mut display).unwrap(),
        NativeFaceMaskInput::Unchanged
    ));
    let details = KeyEvent::new(59, KeyTransition::Pressed, KeyModifiers::NONE).unwrap();
    assert!(matches!(
        service.key(details, 2, &mut display).unwrap(),
        NativeFaceMaskInput::Redrawn(_)
    ));
    assert!(service.scene().unwrap().showing_details());
    let show = service.show().unwrap().clone();
    assert_eq!(
        service.scene().unwrap().interaction(
            &face.identity,
            face.revision,
            crate::native_face_scene::FaceControl {
                action: 0,
                argument: None
            },
            &show,
            vec![],
            3,
        ),
        Err(crate::native_face_scene::FaceSceneError::InputUnavailable)
    );
    let interaction = FaceInteraction::new(
        &face,
        &show,
        &wake_action.identity,
        &wake_action.target,
        vec![],
        3,
    )
    .unwrap();
    assert!(matches!(
        service.submit(interaction),
        Err(NativeFaceMaskError::InputUnavailable)
    ));
}

/// Shared production fallback preparation and actual graphics rows; this does
/// not substitute for installed owner route/Show acknowledgement proof.
#[test]
fn valid_face_beyond_checked_recipe_capacity_keeps_generic_reading_and_controls() {
    use conduit_face::*;
    let mut subjects = vec![FaceSubject {
        identity: "list".into(),
        role: FaceRole::Collection,
        name: "All required things".into(),
    }];
    let mut actions = vec![];
    for number in 0..21 {
        let identity = alloc::format!("item/{number}");
        subjects.push(FaceSubject {
            identity: identity.clone(),
            role: FaceRole::Item,
            name: alloc::format!("Required thing {number}"),
        });
        actions.push(FaceAction {
            identity: alloc::format!("action/{number}"),
            intent: "test/inspect@1".into(),
            target: identity,
            name: alloc::format!("Inspect {number}"),
            arguments: vec![],
            disclosure: FaceDisclosureLevel::CurrentAction,
            availability: FaceActionAvailability::Available,
        });
    }
    subjects.push(FaceSubject {
        identity: "unknown".into(),
        role: FaceRole::Semantic(conduit_core::kind_id("biology/cell@1")),
        name: "Unmatched meaning remains readable".into(),
    });
    let face = Face::new_with_semantics(
        7,
        crate::native_face_scene::tests::face(7).basis,
        subjects,
        vec![],
        vec![FaceProperty {
            subject: "list".into(),
            name: "required fact".into(),
            value: FacePropertyValue::Text("Exact fact remains inspectable".into()),
        }],
        vec![],
        actions,
        vec![],
    )
    .unwrap();
    face.validate().unwrap();
    let provider = crate::product_bases::fixture_surface_provider();
    let host = conduit_conduitos_mask_offer::native_host_advertisement(
        &"host/native".into(),
        &"boot/native".into(),
        1,
        &provider.entry,
    );
    let (_, configuration) = conduit_conduitos_mask_offer::prepare_component_stage_from_offer(
        &host,
        ComponentMaskLook::Spacious,
        TodoComponentRecipe::Grouped,
    )
    .unwrap();
    for input in [true, false] {
        let (mut scene, expression, fallback) = NativeFaceMask::prepare_component_scene(
            &configuration,
            &face,
            320,
            240,
            input,
            VisualUserConstraints {
                forced_colors: true,
                reduced_motion: true,
                minimum_text_scale_percent: 200,
                maximum_supported_text_scale_percent: 200,
            },
        )
        .unwrap();
        assert!(fallback.unwrap().contains("SlotCapacity"));
        assert_eq!(scene.face(), &face);
        assert_eq!(scene.text_scale_percent(), 200);
        assert!(expression.platform_forced_colors);
        scene.show_details(true);
        let mut inspected = alloc::string::String::new();
        for page in 0..scene.pages() {
            let frame = scene.frame().unwrap();
            for command in frame.scene.commands() {
                // Exclude repeating page header/footer chrome; concatenate every
                // actual document text command across all continuation pages.
                if command.kind == GraphicsCommandKind::Text
                    && command.bounds.y >= 56
                    && i32::from(command.bounds.y) + i32::from(command.bounds.height) <= 184
                {
                    inspected.push_str(command.payload());
                }
            }
            if page + 1 < scene.pages() {
                scene.turn_page(true);
            }
        }
        let expected: alloc::string::String = plan_face_utterances(&face)
            .unwrap()
            .clauses
            .iter()
            .map(|clause| clause.text.as_str())
            .collect();
        assert_eq!(inspected, expected);
        assert!(inspected.contains("Exact fact remains inspectable"));
        assert!(inspected.contains("Unmatched meaning remains readable"));
        for number in 0..21 {
            assert!(inspected.contains(&alloc::format!("Required thing {number}")));
        }
        if input {
            let mut reached = vec![];
            for _ in 0..21 {
                let control = scene.focus_next(true).unwrap();
                assert!(!reached.contains(&control));
                reached.push(control);
                scene.frame().unwrap();
            }
            assert_eq!(reached.len(), 21);
        } else {
            assert_eq!(scene.focus_next(true), None);
        }
        assert_eq!(scene.face(), &face);
    }
    assert!(
        NativeFaceMask::prepare_component_scene(
            &configuration,
            &face,
            319,
            240,
            true,
            VisualUserConstraints {
                forced_colors: false,
                reduced_motion: true,
                minimum_text_scale_percent: 200,
                maximum_supported_text_scale_percent: 200
            }
        )
        .is_err()
    );
}
