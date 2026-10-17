use super::*;
use alloc::{string::String, vec};
use conduit_core::{CheckedValueContract, ValueConstraint, kind_id};
use conduit_face::*;
use conduit_human::{KeyEvent, KeyModifiers, KeyTransition};
#[path = "fixture.rs"]
pub(crate) mod fixture;

pub(crate) fn face(revision: u64) -> Face {
    let producer = fixture::producer_plan();
    Face::new_with_semantics(
        revision,
        FaceBasis {
            body_id: Some(serde_json::from_str("\"body/native-face-fixture\"").unwrap()),
            wake_id: None,
            source_document_id: Some(producer.source_document_id),
            checked_plot_id: Some(producer.checked_plot_id),
            expanded_plot_id: Some(producer.expanded_plot_id),
            plan_id: Some(producer.plan_id),
            active_play_id: None,
            sign_ids: vec![],
        },
        vec![
            FaceSubject {
                identity: "z-first".into(),
                role: FaceRole::Document,
                name: "A readable encounter".into(),
            },
            FaceSubject {
                identity: "a-field".into(),
                role: FaceRole::TextEntry,
                name: "Friendly name".into(),
            },
        ],
        vec![FaceRelationship {
            source: "z-first".into(),
            target: "a-field".into(),
            kind: FaceRelationshipKind::Contains,
        }],
        vec![FaceProperty {
            subject: "z-first".into(),
            name: "producer".into(),
            value: FacePropertyValue::Identity("opaque/sha256:producer".into()),
        }],
        vec![FaceText {
            subject: "a-field".into(),
            text: "Give this encounter a name.".into(),
        }],
        vec![
            FaceAction {
                identity: "edit".into(),
                intent: "test/edit".into(),
                target: "a-field".into(),
                name: "Change name".into(),
                arguments: vec![
                    FaceActionArgument::text("value".into(), "Name".into(), 0, 64).unwrap(),
                ],
                disclosure: FaceDisclosureLevel::CurrentAction,
                availability: FaceActionAvailability::Available,
            },
            FaceAction {
                identity: "finish".into(),
                intent: "test/finish".into(),
                target: "z-first".into(),
                name: "Finish".into(),
                arguments: vec![],
                disclosure: FaceDisclosureLevel::CurrentAction,
                availability: FaceActionAvailability::Available,
            },
        ],
        vec![],
    )
    .unwrap()
}

fn press(usage: u8) -> KeyEvent {
    KeyEvent::new(usage, KeyTransition::Pressed, KeyModifiers::NONE).unwrap()
}

#[test]
fn bounded_local_return_notice_does_not_mutate_owner_face() {
    let owner_face = face(7);
    let mut scene = NativeFaceScene::prepare(owner_face.clone(), 640, 480).unwrap();
    scene
        .set_local_notice("Owner accepted: Face refreshed")
        .unwrap();
    assert_eq!(scene.face(), &owner_face);
    assert!(matches!(
        scene.set_local_notice(&"x".repeat(97)),
        Err(FaceSceneError::InputBound)
    ));
    assert_eq!(scene.face(), &owner_face);
}

#[test]
fn incomplete_local_input_can_be_corrected_on_the_same_show() {
    let owner_face = face(9);
    let show = fixture::show(&owner_face);
    let mut scene = NativeFaceScene::prepare(owner_face.clone(), 640, 480).unwrap();
    focus_input(&mut scene);
    assert!(matches!(
        scene.key(press(40), &show, 1),
        Err(FaceSceneError::IncompleteInput)
    ));
    scene.set_local_notice("Input refused: incomplete").unwrap();
    assert!(scene.clear_local_notice());
    assert!(matches!(
        scene.key(press(4), &show, 2).unwrap(),
        FaceSceneInput::Changed
    ));
    let FaceSceneInput::Submitted { interaction, .. } = scene.key(press(40), &show, 3).unwrap()
    else {
        panic!("corrected input must submit on the original Show");
    };
    assert_eq!(interaction.arguments[0].value, b"a");
    assert_eq!(scene.face(), &owner_face);
}

#[test]
fn checked_mask_show_survives_compact_bounded_wire_roundtrip() {
    let owner_face = face(10);
    let show = fixture::show(&owner_face);
    let mut compact = vec![0u8; 65_536];
    let length = serde_json_core::to_slice(&show, &mut compact).unwrap();
    let decoded: MaskShow = serde_json::from_slice(&compact[..length]).unwrap();
    assert_eq!(decoded, show);
    assert!(length <= 65_536);
}

#[test]
fn creche_name_argument_reverses_to_birth_without_inventory_index_guessing() {
    use conduit_birth_plot::{BirthDraft, BirthFaceBasis, BirthPlotChoice};
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
    let producer = fixture::producer_plan();
    let face = draft
        .face(&BirthFaceBasis {
            host_id: "host/birth".into(),
            boot_id: "boot/birth".into(),
            encounter_id: "arrival/one".into(),
            producer_plot: conduit_core::PlotIdentity {
                source_document_id: producer.source_document_id,
                checked_plot_id: producer.checked_plot_id,
                expanded_plot_id: producer.expanded_plot_id,
            },
            producer_plan_id: producer.plan_id,
        })
        .unwrap();
    let mut scene = NativeFaceScene::prepare(face, 1280, 800).unwrap();
    scene
        .focus_named(&FaceFocusRequest {
            action_id: "creche.name".into(),
            argument_name: Some("value".into()),
        })
        .unwrap();
    for expected in ["creche.name", "creche.suggest", "creche.birth"] {
        let control = scene.focus_next(false).unwrap();
        assert_eq!(scene.face().actions[control.action].identity, expected);
        assert_eq!(control.argument, None);
    }
}

#[test]
fn modifier_press_on_argument_free_action_does_not_submit_or_refuse() {
    let original = face(2);
    let show = fixture::show(&original);
    let mut scene = NativeFaceScene::prepare(original, 640, 480).unwrap();
    scene
        .focus_named(&FaceFocusRequest {
            action_id: "finish".into(),
            argument_name: None,
        })
        .unwrap();
    let shift = KeyEvent::new(225, KeyTransition::Pressed, KeyModifiers::LEFT_SHIFT).unwrap();
    assert!(matches!(
        scene.key(shift, &show, 1),
        Ok(FaceSceneInput::Unchanged)
    ));
}

fn focus_input(scene: &mut NativeFaceScene) {
    scene
        .focus_named(&FaceFocusRequest {
            action_id: "edit".into(),
            argument_name: Some("value".into()),
        })
        .unwrap();
}

#[test]
fn full_face_retained_and_semantic_order_is_not_action_index_order() {
    let original = face(1);
    let mut scene = NativeFaceScene::prepare(original.clone(), 640, 480).unwrap();
    let frame = scene.frame().unwrap();
    assert_eq!(frame.face_id, &original.identity);
    assert_eq!(frame.revision, 1);
    assert_eq!(frame.page, scene.page());
    assert_eq!(frame.pages, scene.pages());
    assert!(!scene.showing_details());
    assert!(scene.focused().is_none());
    assert!(
        frame
            .scene
            .commands()
            .iter()
            .any(|command| command.payload() == "A readable encounter")
    );
    assert!(
        !scene
            .primary
            .iter()
            .any(|row| row.text.contains("opaque/sha256"))
    );
    assert!(
        scene
            .details
            .iter()
            .any(|row| row.text.contains("opaque/sha256"))
    );
    assert_eq!(
        scene.focus_next(true),
        Some(FaceControl {
            action: 1,
            argument: None
        })
    );
    assert_eq!(
        scene.focus_next(true),
        Some(FaceControl {
            action: 0,
            argument: None
        })
    );
    assert_eq!(
        scene.focus_next(true),
        Some(FaceControl {
            action: 0,
            argument: Some(0)
        })
    );
    assert_eq!(
        scene.focus_next(false),
        Some(FaceControl {
            action: 0,
            argument: None
        })
    );
    scene.show_details(true);
    scene.turn_page(true);
    assert_eq!(scene.face(), &original);
}

#[test]
fn primary_view_keeps_relationships_and_composition_in_inspection() {
    let original = face(10)
        .with_composition(vec![FaceCompositionRelation {
            identity: "name-after-document".into(),
            source: "a-field".into(),
            target: "z-first".into(),
            kind: FaceCompositionKind::RevealAfter,
        }])
        .unwrap();
    let scene = NativeFaceScene::prepare(original.clone(), 640, 480).unwrap();
    let spoken = plan_face_utterances(&original).unwrap();
    assert!(
        !scene
            .primary
            .iter()
            .any(|row| row.text.contains("Contains ·"))
    );
    for clause in spoken
        .clauses
        .iter()
        .filter(|clause| matches!(clause.provenance, FaceUtteranceProvenance::Composition(_)))
    {
        assert!(scene.details.iter().any(|row| row.text == clause.text));
    }
    assert!(
        spoken
            .clauses
            .iter()
            .filter(|clause| {
                matches!(clause.provenance, FaceUtteranceProvenance::Relationship(_))
            })
            .all(|clause| scene.details.iter().any(|row| row.text == clause.text))
    );
    assert!(scene.primary.iter().any(|row| row.text == "Change name"));
    assert!(
        scene
            .details
            .iter()
            .any(|row| row.text.contains("opaque/sha256"))
    );
}

#[test]
fn primary_view_leaves_properties_in_details() {
    let mut original = face(32);
    original.properties.extend([
        FaceProperty {
            subject: "z-first".into(),
            name: "lifecycle-state".into(),
            value: FacePropertyValue::Text("lulled".into()),
        },
        FaceProperty {
            subject: "z-first".into(),
            name: "workload-revision".into(),
            value: FacePropertyValue::Count(2),
        },
    ]);
    let original = Face::new_with_semantics(
        33,
        original.basis,
        original.subjects,
        original.relationships,
        original.properties,
        original.text,
        original.actions,
        original.disclosures,
    )
    .unwrap();
    let scene = NativeFaceScene::prepare(original.clone(), 640, 480).unwrap();
    assert!(
        !scene
            .primary
            .iter()
            .any(|row| row.text.contains("workload revision"))
    );
    assert!(
        !scene
            .primary
            .iter()
            .any(|row| row.text.contains("lifecycle state"))
    );
    for clause in plan_face_utterances(&original)
        .unwrap()
        .clauses
        .iter()
        .filter(|clause| matches!(clause.provenance, FaceUtteranceProvenance::Property(_)))
    {
        assert!(scene.details.iter().any(|row| row.text == clause.text));
    }
    let frame = scene.frame().unwrap();
    assert!(frame.scene.commands().iter().any(|command| {
        command.kind == GraphicsCommandKind::Rect
            && command.paint == GraphicsPaintRole::Accent
            && command.bounds.width == 3
    }));
}

#[test]
fn ordinary_graphics_lead_with_primary_content_and_keep_context_inspectable() {
    let mut original = face(34);
    original.subjects[0].role = FaceRole::Body;
    original.text.push(FaceText {
        subject: "z-first".into(),
        text: "Groceries is lulled with one resident plot.".into(),
    });
    original.subjects.push(FaceSubject {
        identity: "todo-list".into(),
        role: FaceRole::Collection,
        name: "Groceries".into(),
    });
    original.disclosures.push(FaceDisclosure {
        subject: "todo-list".into(),
        level: FaceDisclosureLevel::Context,
    });
    original.subjects.push(FaceSubject {
        identity: "todo-summary".into(),
        role: FaceRole::Status,
        name: "Remaining".into(),
    });
    original.text.push(FaceText {
        subject: "todo-summary".into(),
        text: "Three things remain.".into(),
    });
    original.disclosures.push(FaceDisclosure {
        subject: "todo-summary".into(),
        level: FaceDisclosureLevel::Primary,
    });
    original.subjects.push(FaceSubject {
        identity: "context".into(),
        role: FaceRole::Region,
        name: "Current overview context".into(),
    });
    original.text.push(FaceText {
        subject: "context".into(),
        text: "Workload revision 18.".into(),
    });
    original.disclosures.push(FaceDisclosure {
        subject: "context".into(),
        level: FaceDisclosureLevel::Context,
    });
    let current = Face::new_with_semantics(
        original.revision + 1,
        original.basis,
        original.subjects,
        original.relationships,
        original.properties,
        original.text,
        original.actions,
        original.disclosures,
    )
    .unwrap();
    let scene = NativeFaceScene::prepare(current, 640, 480).unwrap();
    assert!(scene.primary.iter().any(|row| row.text == "Groceries"));
    assert!(
        scene
            .primary
            .iter()
            .any(|row| row.text == "Three things remain.")
    );
    assert!(
        !scene
            .primary
            .iter()
            .any(|row| row.text.contains("resident plot"))
    );
    assert!(
        !scene
            .primary
            .iter()
            .any(|row| row.text == "Current overview context")
    );
    assert!(
        !scene
            .primary
            .iter()
            .any(|row| row.text == "Workload revision 18.")
    );
    assert!(
        scene
            .details
            .iter()
            .any(|row| row.text.contains("Current overview context"))
    );
    assert!(
        scene
            .details
            .iter()
            .any(|row| row.text == "Workload revision 18.")
    );
    assert!(
        scene
            .details
            .iter()
            .any(|row| row.text.contains("resident plot"))
    );
}

#[test]
fn primary_action_names_bounded_text_choices_from_current_face_contract() {
    let mut current = face(31);
    current.actions[0].arguments[0].value_name = "Interval in milliseconds".into();
    current.actions[0].arguments[0].contract = CheckedValueContract::new(
        kind_id(UTF8_TEXT_VALUE_KIND),
        4,
        vec![ValueConstraint::CanonicalMembership {
            members: ["1000", "2000", "250", "500"]
                .map(|value| value.as_bytes().to_vec())
                .into(),
            negated: false,
        }],
    )
    .unwrap();
    let current = Face::new_with_semantics(
        current.revision + 1,
        current.basis,
        current.subjects,
        current.relationships,
        current.properties,
        current.text,
        current.actions,
        current.disclosures,
    )
    .unwrap();
    let scene = NativeFaceScene::prepare(current.clone(), 640, 480).unwrap();
    assert!(scene.primary.iter().any(|row| {
        row.text
            .contains("Interval in milliseconds · Choose 1000, 2000, 250, 500")
    }));
    let show = fixture::show(&current);
    let interaction = scene
        .interaction(
            &current.identity,
            current.revision,
            FaceControl {
                action: 0,
                argument: Some(0),
            },
            &show,
            vec![FaceInteractionArgument {
                name: "value".into(),
                value_kind: UTF8_TEXT_VALUE_KIND.into(),
                value: b"500".to_vec(),
            }],
            1,
        )
        .unwrap();
    assert_eq!(interaction.arguments[0].value, b"500");

    let mut many = current.actions.clone();
    many[0].arguments[0].contract = CheckedValueContract::new(
        kind_id(UTF8_TEXT_VALUE_KIND),
        2,
        vec![ValueConstraint::CanonicalMembership {
            members: (0u8..9).map(|value| vec![b'0' + value]).collect(),
            negated: false,
        }],
    )
    .unwrap();
    let crowded = Face::new_with_semantics(
        current.revision + 1,
        current.basis,
        current.subjects,
        current.relationships,
        current.properties,
        current.text,
        many,
        current.disclosures,
    )
    .unwrap();
    let crowded_scene = NativeFaceScene::prepare(crowded, 640, 480).unwrap();
    assert!(
        crowded_scene
            .primary
            .iter()
            .any(|row| row.text.contains("Enter a value"))
    );
}

#[test]
fn focused_action_has_visible_focus_and_unavailable_action_has_no_hit() {
    let mut original = face(11);
    original.actions[1].availability = FaceActionAvailability::Unavailable {
        reason_code: "test/not-ready".into(),
        explanation: "Waiting for a name".into(),
    };
    let original = Face::new_with_semantics(
        11,
        original.basis,
        original.subjects,
        original.relationships,
        original.properties,
        original.text,
        original.actions,
        original.disclosures,
    )
    .unwrap();
    let mut scene = NativeFaceScene::prepare(original, 640, 480).unwrap();
    scene
        .focus_named(&FaceFocusRequest {
            action_id: "edit".into(),
            argument_name: None,
        })
        .unwrap();
    let frame = scene.frame().unwrap();
    assert!(frame.scene.commands().iter().any(|command| {
        command.paint == GraphicsPaintRole::Focus && command.kind == GraphicsCommandKind::Rect
    }));
    assert!(
        !scene
            .primary
            .iter()
            .any(|row| row.text.contains("Waiting for a name"))
    );
    assert!(
        scene
            .details
            .iter()
            .any(|row| row.text.contains("Waiting for a name"))
    );
    assert!(frame.hits.iter().all(|hit| hit.action.identity != "finish"));
}

#[test]
fn patchbay_diagram_uses_exact_face_gears_ports_and_cord_endpoints() {
    let mut original = face(12);
    for (identity, role, name) in [
        ("gear/a", FaceRole::Gear, "Source Gear"),
        ("gear/b", FaceRole::Gear, "Sink Gear"),
        ("port/a", FaceRole::Port, "Out · text"),
        ("port/b", FaceRole::Port, "In · text"),
        ("cord/a-b", FaceRole::Cord, "Text to display"),
    ] {
        original.subjects.push(FaceSubject {
            identity: identity.into(),
            role,
            name: name.into(),
        });
    }
    for (source, target, kind) in [
        ("gear/a", "port/a", FaceRelationshipKind::Contains),
        ("gear/b", "port/b", FaceRelationshipKind::Contains),
        ("cord/a-b", "port/a", FaceRelationshipKind::Connects),
        ("cord/a-b", "port/b", FaceRelationshipKind::Connects),
    ] {
        original.relationships.push(FaceRelationship {
            source: source.into(),
            target: target.into(),
            kind,
        });
    }
    for (subject, name, value) in [
        (
            "port/a",
            "semantic-id",
            FacePropertyValue::Identity("out".into()),
        ),
        (
            "port/b",
            "semantic-id",
            FacePropertyValue::Identity("in".into()),
        ),
        (
            "port/a",
            "direction",
            FacePropertyValue::Text("outgoing".into()),
        ),
        (
            "port/b",
            "direction",
            FacePropertyValue::Text("receiving".into()),
        ),
        (
            "cord/a-b",
            "source-port",
            FacePropertyValue::Identity("out".into()),
        ),
        (
            "cord/a-b",
            "sink-port",
            FacePropertyValue::Identity("in".into()),
        ),
    ] {
        original.properties.push(FaceProperty {
            subject: subject.into(),
            name: name.into(),
            value,
        });
    }
    let original = Face::new_with_semantics(
        12,
        original.basis,
        original.subjects,
        original.relationships,
        original.properties,
        original.text,
        original.actions,
        original.disclosures,
    )
    .unwrap();
    let show = fixture::show(&original);
    let mut scene = NativeFaceScene::prepare(original.clone(), 640, 480).unwrap();
    assert!(scene.has_diagram());
    assert!(matches!(
        scene.key(press(60), &show, 1),
        Ok(FaceSceneInput::Changed)
    ));
    assert!(scene.showing_diagram());
    let frame = scene.frame().unwrap();
    assert!(frame.scene.commands().iter().any(|command| {
        command.kind == GraphicsCommandKind::OrthogonalPath && command.path_geometry().is_some()
    }));
    assert!(
        frame
            .scene
            .commands()
            .iter()
            .any(|command| command.payload() == "1 · Source Gear")
    );
    assert!(
        frame
            .scene
            .commands()
            .iter()
            .any(|command| command.payload() == "2 · Sink Gear")
    );
    assert!(frame.hits.is_empty());
    scene.show_details(true);
    assert!(
        scene
            .details
            .iter()
            .any(|row| row.text.contains("Text to display"))
    );
    assert_eq!(scene.face(), &original);
    let mut compact = NativeFaceScene::prepare(original, 320, 240).unwrap();
    compact.show_diagram(true);
    assert_eq!(compact.pages(), 2);
    for _ in 0..compact.pages() {
        let frame = compact.frame().unwrap();
        assert!(frame.scene.commands().len() <= MAX_GRAPHICS_COMMANDS);
        compact.turn_page(true);
    }
}

#[test]
fn every_utf8_byte_survives_layout_pages_with_bounded_commands() {
    let mut original = face(2);
    let long = "é中 long readable words ".repeat(30);
    original.text = vec![FaceText {
        subject: "a-field".into(),
        text: long.clone(),
    }];
    // Use the canonical constructor to recompute identity after replacing text.
    original = Face::new_with_semantics(
        2,
        original.basis,
        original.subjects,
        original.relationships,
        original.properties,
        original.text,
        original.actions,
        original.disclosures,
    )
    .unwrap();
    let mut scene = NativeFaceScene::prepare(original.clone(), 320, 240).unwrap();
    let joined: String = scene
        .primary
        .iter()
        .filter(|row| {
            row.control.is_none()
                && row.role == GraphicsTextRole::Body
                && row.paint == GraphicsPaintRole::Foreground
        })
        .map(|row| row.text.as_str())
        .collect();
    assert_eq!(joined, long);
    for _ in 0..scene.pages() {
        let frame = scene.frame().unwrap();
        assert!(frame.scene.commands().len() <= MAX_GRAPHICS_COMMANDS);
        assert!(
            frame
                .scene
                .commands()
                .iter()
                .all(|command| command.clip_class() == GraphicsClipClass::FullyVisible)
        );
        scene.turn_page(true);
    }
    assert_eq!(scene.face(), &original);
}

#[test]
fn typing_and_tab_commit_exact_argument_then_refuse_more_input_on_old_show() {
    let original = face(3);
    let show = fixture::show(&original);
    let mut scene = NativeFaceScene::prepare(original.clone(), 640, 480).unwrap();
    focus_input(&mut scene);
    for usage in [11, 12] {
        assert!(matches!(
            scene.key(press(usage), &show, 1).unwrap(),
            FaceSceneInput::Changed
        ));
    }
    assert!(
        scene
            .frame()
            .unwrap()
            .scene
            .commands()
            .iter()
            .any(|command| command.payload().contains("Draft: hi"))
    );
    let FaceSceneInput::Submitted {
        interaction,
        next_focus,
    } = scene.key(press(43), &show, 7).unwrap()
    else {
        panic!("expected commit");
    };
    assert_eq!(interaction.action_id, "edit");
    assert_eq!(interaction.face_revision, 3);
    assert_eq!(interaction.arguments[0].value, b"hi");
    assert!(next_focus.is_some());
    assert!(matches!(
        scene.key(press(40), &show, 8),
        Err(FaceSceneError::StaleFace)
    ));
    assert_eq!(scene.face(), &original);
}

#[test]
fn stale_show_unacknowledged_show_and_stale_mapping_refuse() {
    let original = face(4);
    let show = fixture::show(&original);
    let mut scene = NativeFaceScene::prepare(face(5), 640, 480).unwrap();
    assert!(matches!(
        scene.key(press(43), &show, 1),
        Err(FaceSceneError::StaleFace)
    ));
    assert_eq!(
        scene
            .resolve(
                &original.identity,
                4,
                FaceControl {
                    action: 0,
                    argument: Some(0)
                }
            )
            .unwrap_err(),
        FaceSceneError::StaleFace
    );
    scene = NativeFaceScene::prepare(original.clone(), 640, 480).unwrap();
    let prepared = fixture::prepared(&original);
    assert!(matches!(
        scene.key(press(43), &prepared, 1),
        Err(FaceSceneError::StaleFace)
    ));
}

#[test]
fn pointer_mapping_retains_contract_and_never_invokes_an_unavailable_action() {
    let mut original = face(6);
    original.actions[1].availability = FaceActionAvailability::Unavailable {
        reason_code: "test/no".into(),
        explanation: "Not ready".into(),
    };
    original = Face::new_with_semantics(
        6,
        original.basis,
        original.subjects,
        original.relationships,
        original.properties,
        original.text,
        original.actions,
        original.disclosures,
    )
    .unwrap();
    let show = fixture::show(&original);
    let mut scene = NativeFaceScene::prepare(original, 640, 480).unwrap();
    focus_input(&mut scene);
    let frame = scene.frame().unwrap();
    let hit = frame
        .hits
        .iter()
        .find(|hit| hit.argument.is_some())
        .unwrap();
    assert_eq!(hit.argument.unwrap().contract.maximum_bytes, 64);
    assert!(frame.hits.iter().all(|hit| hit.action.identity != "finish"));
    let (x, y) = (hit.bounds.x + 1, hit.bounds.y + 1);
    assert!(matches!(
        scene.pointer(x, y, &show, 1).unwrap(),
        FaceSceneInput::Changed
    ));
}

#[test]
fn text_bound_and_backspace_preserve_valid_replacement() {
    let original = face(8);
    let show = fixture::show(&original);
    let mut scene = NativeFaceScene::prepare(original, 640, 480).unwrap();
    focus_input(&mut scene);
    for _ in 0..64 {
        scene.key(press(4), &show, 1).unwrap();
    }
    assert!(matches!(
        scene.key(press(4), &show, 1),
        Err(FaceSceneError::InputBound)
    ));
    scene.key(press(42), &show, 1).unwrap();
    let FaceSceneInput::Submitted { interaction, .. } = scene.key(press(40), &show, 1).unwrap()
    else {
        panic!()
    };
    assert_eq!(interaction.arguments[0].value.len(), 63);
}

#[test]
fn face_words_render_as_nonuniform_scene_pixels() {
    use crate::display::{DisplayError, DisplayFormat, PixelTarget, RetainedPixelTarget};
    struct Pixels(Vec<u32>);
    impl PixelTarget for Pixels {
        fn format(&self) -> DisplayFormat {
            DisplayFormat {
                width: 640,
                height: 480,
                pitch: 2560,
                bits_per_pixel: 32,
                red_shift: 16,
                green_shift: 8,
                blue_shift: 0,
            }
        }
        fn write_pixel(&mut self, x: u32, y: u32, value: u32) -> Result<(), DisplayError> {
            self.0[(y * 640 + x) as usize] = value;
            Ok(())
        }
    }
    impl RetainedPixelTarget for Pixels {
        fn read_pixel(&self, x: u32, y: u32) -> Result<u32, DisplayError> {
            Ok(self.0[(y * 640 + x) as usize])
        }
    }
    let mut pixels = Pixels(vec![0; 640 * 480]);
    let scene = NativeFaceScene::prepare(face(9), 640, 480).unwrap();
    crate::display::typography::render_scene(
        &mut pixels,
        &scene.frame().unwrap().scene,
        |_, command| command.text_role().into(),
    )
    .unwrap();
    assert!(pixels.0.windows(2).any(|pair| pair[0] != pair[1]));
}

#[test]
fn checked_native_recipe_marks_contiguous_runs_without_reordering_controls() {
    let mut original = face(7);
    original.subjects[0].role = FaceRole::Collection;
    original.subjects[1].role = FaceRole::Item;
    let original = Face::new_with_semantics(
        original.revision,
        original.basis,
        original.subjects,
        original.relationships,
        original.properties,
        original.text,
        original.actions,
        original.disclosures,
    )
    .unwrap();
    let provider = fixture::framebuffer_provider_entry();
    let host = conduit_conduitos_mask_offer::native_host_advertisement(
        &"host/native".into(),
        &"boot/native".into(),
        1,
        &provider,
    );
    let mut scenes = Vec::new();
    for recipe in [TodoComponentRecipe::Flat, TodoComponentRecipe::Grouped] {
        let (_, configuration) = conduit_conduitos_mask_offer::prepare_component_stage_from_offer(
            &host,
            ComponentMaskLook::Spacious,
            recipe,
        )
        .unwrap();
        let recipe = prepare_component_recipe(
            conduit_core::StructuredInfoValue::from_canonical_bytes(
                configuration.recipe.canonical_value(),
            )
            .unwrap(),
            &original,
        )
        .unwrap();
        let expression = admit_visual_expression(
            Ok(visual_expression_from_info(
                conduit_core::StructuredInfoValue::from_canonical_bytes(
                    configuration.expression.canonical_value(),
                )
                .unwrap(),
            )
            .unwrap()),
            VisualUserConstraints {
                forced_colors: false,
                reduced_motion: true,
                minimum_text_scale_percent: 100,
                maximum_supported_text_scale_percent: 200,
            },
        )
        .unwrap();
        let fallback = NativeFaceScene::prepare_visual_fallback(
            original.clone(),
            1000,
            480,
            true,
            &expression,
        )
        .unwrap();
        assert_eq!(fallback.face(), &original);
        assert_eq!(fallback.text_scale_percent(), 125);
        assert!(fallback.frame().is_ok());
        scenes.push(
            NativeFaceScene::prepare_configured(
                original.clone(),
                1000,
                480,
                true,
                &recipe,
                &expression,
            )
            .unwrap(),
        );
    }
    assert_eq!(scenes[0].face(), &original);
    assert_eq!(scenes[1].face(), &original);
    let controls = |scene: &NativeFaceScene| {
        scene
            .primary
            .iter()
            .filter_map(|row| row.control)
            .collect::<Vec<_>>()
    };
    assert_eq!(controls(&scenes[0]), controls(&scenes[1]));
    assert_eq!(
        scenes[0]
            .primary
            .iter()
            .filter(|row| row.region_start)
            .count(),
        1
    );
    assert_eq!(
        scenes[1]
            .primary
            .iter()
            .filter(|row| row.region_start)
            .count(),
        2
    );
    assert_eq!(scenes[0].text_scale_percent(), 125);
    assert_eq!(scenes[1].text_scale_percent(), 125);
}

#[test]
fn maximum_todo_projection_keeps_all_controls_and_detail_pages_in_both_native_looks() {
    use conduit_todo_face::todo_fragment;
    use conduit_todo_plot::{TodoItem, TodoState};
    let state = TodoState {
        title: "Things".into(),
        revision: 20,
        next_id: 21,
        items: (1..=20)
            .map(|number| TodoItem {
                id: alloc::format!("task-{number}"),
                text: alloc::format!("{number:02}{}", "é".repeat(35)),
                complete: number % 2 == 0,
            })
            .collect(),
    };
    state.validate().unwrap();
    let fragment = todo_fragment(
        &state,
        FaceContributionBasis {
            checked_plot_id: "checked/todo".into(),
            plan_id: "plan/todo".into(),
            active_play_id: "play/todo".into(),
            required_interaction_context: None,
        },
        true,
    )
    .unwrap();
    let original = Face::new_with_semantics(
        7,
        face(7).basis,
        fragment.subjects,
        fragment.relationships,
        fragment.properties,
        fragment.text,
        fragment.actions,
        fragment.disclosures,
    )
    .unwrap();
    assert!(!original.actions[0].availability.is_available());
    let provider = fixture::framebuffer_provider_entry();
    let host = conduit_conduitos_mask_offer::native_host_advertisement(
        &"host/native".into(),
        &"boot/native".into(),
        1,
        &provider,
    );
    for look in [ComponentMaskLook::Compact, ComponentMaskLook::Spacious] {
        for recipe in [TodoComponentRecipe::Flat, TodoComponentRecipe::Grouped] {
            let (_, configuration) =
                conduit_conduitos_mask_offer::prepare_component_stage_from_offer(
                    &host, look, recipe,
                )
                .unwrap();
            let recipe = prepare_component_recipe(
                conduit_core::StructuredInfoValue::from_canonical_bytes(
                    configuration.recipe.canonical_value(),
                )
                .unwrap(),
                &original,
            )
            .unwrap();
            let expression = admit_visual_expression(
                visual_expression_from_info(
                    conduit_core::StructuredInfoValue::from_canonical_bytes(
                        configuration.expression.canonical_value(),
                    )
                    .unwrap(),
                ),
                VisualUserConstraints {
                    forced_colors: true,
                    reduced_motion: true,
                    minimum_text_scale_percent: 200,
                    maximum_supported_text_scale_percent: 200,
                },
            )
            .unwrap();
            let mut scene = NativeFaceScene::prepare_configured(
                original.clone(),
                320,
                240,
                true,
                &recipe,
                &expression,
            )
            .unwrap();
            assert_eq!(scene.face(), &original);
            assert_eq!(scene.text_scale_percent(), 200);
            assert!(expression.platform_forced_colors);
            scene.show_details(true);
            assert!(scene.pages() > 1);
            let text: alloc::string::String =
                scene.details.iter().map(|row| row.text.as_str()).collect();
            for item in &state.items {
                assert!(text.contains(&item.text));
            }
            let mut expected = Vec::new();
            for row in &scene.details {
                if let Some(control) = row.control
                    && original.actions[control.action].availability.is_available()
                    && !expected.contains(&control)
                {
                    expected.push(control);
                }
            }
            assert_eq!(expected.len(), 40);
            let mut reached = Vec::new();
            for _ in 0..expected.len() {
                let control = scene.focus_next(true).unwrap();
                assert!(!reached.contains(&control));
                assert!(original.actions[control.action].availability.is_available());
                scene.frame().unwrap();
                reached.push(control);
            }
            assert_eq!(reached, expected);
            assert_eq!(scene.focus_next(true), expected.first().copied());
            assert_eq!(scene.face(), &original);
        }
    }
}

#[test]
fn native_density_changes_measured_padding_without_changing_source_geometry() {
    let original = face(7);
    let provider = fixture::framebuffer_provider_entry();
    let host = conduit_conduitos_mask_offer::native_host_advertisement(
        &"host/native".into(),
        &"boot/native".into(),
        1,
        &provider,
    );
    let mut scenes = Vec::new();
    for look in [ComponentMaskLook::Compact, ComponentMaskLook::Spacious] {
        let (_, configuration) = conduit_conduitos_mask_offer::prepare_component_stage_from_offer(
            &host,
            look,
            TodoComponentRecipe::Flat,
        )
        .unwrap();
        let recipe = prepare_component_recipe(
            conduit_core::StructuredInfoValue::from_canonical_bytes(
                configuration.recipe.canonical_value(),
            )
            .unwrap(),
            &original,
        )
        .unwrap();
        let expression = admit_visual_expression(
            visual_expression_from_info(
                conduit_core::StructuredInfoValue::from_canonical_bytes(
                    configuration.expression.canonical_value(),
                )
                .unwrap(),
            ),
            VisualUserConstraints {
                forced_colors: false,
                reduced_motion: true,
                minimum_text_scale_percent: 200,
                maximum_supported_text_scale_percent: 200,
            },
        )
        .unwrap();
        scenes.push(
            NativeFaceScene::prepare_configured(
                original.clone(),
                1000,
                480,
                true,
                &recipe,
                &expression,
            )
            .unwrap(),
        );
    }
    assert_eq!(scenes[0].primary[0].padding, SPACE_SM);
    assert_eq!(scenes[1].primary[0].padding, SPACE_SM * 2);
    assert_eq!(scenes[0].primary[0].bounds.x, scenes[1].primary[0].bounds.x);
    assert_eq!(
        scenes[0].primary[0].bounds.width,
        scenes[1].primary[0].bounds.width
    );
    assert_eq!(
        scenes[1].primary[0].bounds.height,
        scenes[0].primary[0].bounds.height + SPACE_SM * 2
    );
    assert_eq!(
        scenes[0].primary[0].text_bounds().height,
        scenes[1].primary[0].text_bounds().height
    );
    let controls = |scene: &NativeFaceScene| {
        scene
            .primary
            .iter()
            .filter_map(|row| row.control)
            .collect::<Vec<_>>()
    };
    assert_eq!(controls(&scenes[0]), controls(&scenes[1]));
    assert_eq!(scenes[0].face(), &original);
    assert_eq!(scenes[1].face(), &original);
}
