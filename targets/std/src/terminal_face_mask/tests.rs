//! Renderer/input conformance. Planned Show acknowledgement here is explicitly a
//! fixture; the concrete hosted kernel bridge is tested by its integration owner.
use super::*;
use conduit_presentation::*;
#[path = "fixture.rs"]
mod fixture;

fn face(revision: u64) -> Presentation {
    let producer = fixture::producer_plan();
    Presentation::new_with_semantics(
        revision,
        PresentationBasis {
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
            PresentationSubject {
                identity: "z-first".into(),
                role: PresentationRole::Document,
                name: "A readable encounter".into(),
            },
            PresentationSubject {
                identity: "a-field".into(),
                role: PresentationRole::TextEntry,
                name: "Friendly name".into(),
            },
        ],
        vec![PresentationRelationship {
            source: "z-first".into(),
            target: "a-field".into(),
            kind: PresentationRelationshipKind::Contains,
        }],
        vec![PresentationProperty {
            subject: "z-first".into(),
            name: "producer".into(),
            value: PresentationPropertyValue::Identity("opaque/sha256:producer".into()),
        }],
        vec![PresentationText {
            subject: "a-field".into(),
            text: "Give this encounter a name.".into(),
        }],
        vec![
            PresentationAction {
                identity: "edit".into(),
                intent: "test/edit".into(),
                target: "a-field".into(),
                name: "Change name".into(),
                arguments: vec![
                    FaceActionArgument::text("value".into(), "Name".into(), 0, 64).unwrap(),
                ],
                disclosure: PresentationDisclosureLevel::CurrentAction,
                availability: PresentationActionAvailability::Available,
            },
            PresentationAction {
                identity: "finish".into(),
                intent: "test/finish".into(),
                target: "z-first".into(),
                name: "Finish".into(),
                arguments: vec![],
                disclosure: PresentationDisclosureLevel::CurrentAction,
                availability: PresentationActionAvailability::Available,
            },
        ],
        vec![],
    )
    .unwrap()
}

fn bind(mask: &mut TerminalFaceMask) -> MaskShow {
    let prepared = fixture::prepared(mask.presentation());
    let mut output = Vec::new();
    let receipt = mask.render(&mut output, &prepared).unwrap();
    assert_eq!(receipt.bytes_written(), output.len());
    let available = prepared
        .transition(
            ManifestationLifecycle::Available,
            "fixture/terminal-flushed".into(),
        )
        .unwrap();
    mask.bind_show(receipt, available.clone()).unwrap();
    available
}
#[test]
fn full_document_preserves_words_relationships_and_inspection() {
    let mut mask = TerminalFaceMask::prepare(face(1), 60, 12).unwrap();
    let document = mask
        .document
        .iter()
        .map(|r| r.text.as_str())
        .collect::<String>();
    for wording in [
        "A readable encounter",
        "Friendly name",
        "Give this encounter",
        "Change name",
        "Finish",
    ] {
        assert!(document.contains(wording), "{wording}");
    }
    assert!(mask.document.iter().any(|r| r.text.contains("contains")));
    let show = bind(&mut mask);
    assert_eq!(
        mask.input(TerminalInput::Inspect, &show, 0).unwrap(),
        TerminalInputOutcome::Redraw
    );
    assert!(mask.inspecting());
    let exact = mask
        .inspect_document
        .iter()
        .map(|r| r.text.as_str())
        .collect::<String>();
    assert!(exact.contains(mask.presentation().identity.as_str()));
    assert!(exact.contains("opaque/sha256:producer"));
    assert!(mask.show().is_none());
}

#[test]
fn todo_default_is_a_checklist_with_exact_actions_and_full_inspection() {
    let base = face(1);
    let mut subjects = base.subjects;
    let mut properties = base.properties;
    let mut disclosures = Vec::new();
    subjects.push(PresentationSubject {
        identity: "todo/list".into(),
        role: PresentationRole::Collection,
        name: "Groceries".into(),
    });
    disclosures.push(PresentationDisclosure {
        subject: "todo/list".into(),
        level: PresentationDisclosureLevel::Primary,
    });
    subjects.push(PresentationSubject {
        identity: "todo/status".into(),
        role: PresentationRole::Status,
        name: "Progress".into(),
    });
    disclosures.push(PresentationDisclosure {
        subject: "todo/status".into(),
        level: PresentationDisclosureLevel::Primary,
    });
    for index in 0..20 {
        let identity = format!("todo/item/{index}");
        subjects.push(PresentationSubject {
            identity: identity.clone(),
            role: PresentationRole::Item,
            name: if index < 3 {
                format!("Open item {index}")
            } else {
                format!("Completed item {index}")
            },
        });
        properties.push(PresentationProperty {
            subject: identity.clone(),
            name: "complete".into(),
            value: PresentationPropertyValue::Flag(index >= 3),
        });
        properties.push(PresentationProperty {
            subject: identity.clone(),
            name: "order".into(),
            value: PresentationPropertyValue::Count(index as u64),
        });
        disclosures.push(PresentationDisclosure {
            subject: identity,
            level: if index < 3 {
                PresentationDisclosureLevel::Primary
            } else {
                PresentationDisclosureLevel::SelectedDetail
            },
        });
    }
    let actions = vec![
        PresentationAction {
            identity: "todo/add".into(),
            intent: "todo/add".into(),
            target: "todo/list".into(),
            name: "Add item".into(),
            arguments: vec![
                FaceActionArgument::text("text".into(), "New item".into(), 1, 256).unwrap(),
            ],
            disclosure: PresentationDisclosureLevel::CurrentAction,
            availability: PresentationActionAvailability::Available,
        },
        PresentationAction {
            identity: "todo/complete/0".into(),
            intent: "todo/complete".into(),
            target: "todo/item/0".into(),
            name: "Complete item".into(),
            arguments: vec![],
            disclosure: PresentationDisclosureLevel::CurrentAction,
            availability: PresentationActionAvailability::Available,
        },
        PresentationAction {
            identity: "todo/reopen/3".into(),
            intent: "todo/reopen".into(),
            target: "todo/item/3".into(),
            name: "Reopen item".into(),
            arguments: vec![],
            disclosure: PresentationDisclosureLevel::CurrentAction,
            availability: PresentationActionAvailability::Available,
        },
        PresentationAction {
            identity: "todo/unavailable/0".into(),
            intent: "todo/remove".into(),
            target: "todo/item/0".into(),
            name: "Unavailable removal".into(),
            arguments: vec![],
            disclosure: PresentationDisclosureLevel::CurrentAction,
            availability: PresentationActionAvailability::Unavailable {
                reason_code: "not-now".into(),
                explanation: "Not available now".into(),
            },
        },
    ];
    let todo = Presentation::new_with_semantics(
        2,
        base.basis,
        subjects,
        base.relationships,
        properties,
        vec![
            PresentationText {
                subject: "todo/list".into(),
                text: "Groceries".into(),
            },
            PresentationText {
                subject: "todo/status".into(),
                text: "3 things left · 17 completed".into(),
            },
        ],
        actions,
        disclosures,
    )
    .unwrap();
    let mask = TerminalFaceMask::prepare(todo, 80, 14).unwrap();
    let default = mask
        .document
        .iter()
        .map(|row| row.text.as_str())
        .collect::<String>();
    assert!(default.starts_with("Groceries3 things left · 17 completed1. [ ] Open item 0"));
    assert!(!default.contains("Progress"));
    assert!(document::frame(&mask).starts_with("\x1b[2J\x1b[H\x1b[1mGroceries"));
    assert!(default.contains("3. [ ] Open item 2"));
    assert!(default.contains("Add item"));
    assert!(default.contains("Complete item · Open item 0"));
    assert!(!default.contains("Completed item"));
    assert!(!default.contains("opaque/sha256:producer"));
    assert!(!default.contains("Reopen item"));
    assert!(!default.contains("Unavailable removal"));
    assert!(mask.document.iter().any(|row| row.control
        == Some(TerminalControl {
            action: 0,
            argument: Some(0)
        })));
    assert!(mask.document.iter().any(|row| row.control
        == Some(TerminalControl {
            action: 1,
            argument: None
        })));
    let inspection = mask
        .inspect_document
        .iter()
        .map(|row| row.text.as_str())
        .collect::<String>();
    assert!(inspection.contains("Completed item 17"));
    assert!(inspection.contains("opaque/sha256:producer"));
    assert!(inspection.contains("Unavailable removal"));
}

#[test]
fn terminal_face_names_small_exact_text_choices_without_changing_the_contract() {
    let base = face(1);
    let mut actions = base.actions.clone();
    actions[0].arguments[0].contract = conduit_core::CheckedValueContract::new(
        conduit_core::kind_id(UTF8_TEXT_VALUE_KIND),
        4,
        vec![conduit_core::ValueConstraint::CanonicalMembership {
            members: ["1000", "2000", "250", "500"]
                .map(|value| value.as_bytes().to_vec())
                .into(),
            negated: false,
        }],
    )
    .unwrap();
    let changed = Presentation::new_with_semantics(
        base.revision + 1,
        base.basis,
        base.subjects,
        base.relationships,
        base.properties,
        base.text,
        actions,
        base.disclosures,
    )
    .unwrap();
    let mask = TerminalFaceMask::prepare_read_only(changed, 60, 12).unwrap();
    let document = mask
        .document
        .iter()
        .map(|row| row.text.as_str())
        .collect::<String>();
    assert!(document.contains("For Change name, choose Name: 1000, 2000, 250, 500."));
    assert!(!document.contains("canonical values 0x"));
    let inspection = mask
        .inspect_document
        .iter()
        .map(|row| row.text.as_str())
        .collect::<String>();
    assert!(inspection.contains("CanonicalMembership"));
    assert!(mask.face.actions[0].arguments[0]
        .contract
        .validate(b"500")
        .is_ok());
    assert!(mask.face.actions[0].arguments[0]
        .contract
        .validate(b"750")
        .is_err());
}

#[test]
fn read_only_owner_face_can_be_read_but_never_submitted() {
    let mut mask = TerminalFaceMask::prepare_read_only(face(7), 60, 12).unwrap();
    let show = bind(&mut mask);
    let frame = document::frame(&mask);
    assert!(frame.contains("Read only"));
    assert!(!frame.contains("Enter apply"));
    assert_eq!(
        mask.input(TerminalInput::NextControl, &show, 1).unwrap(),
        TerminalInputOutcome::Unchanged
    );
    assert_eq!(mask.focused(), None);
    assert_eq!(
        mask.input(TerminalInput::Apply, &show, 2),
        Err(TerminalError::UnsupportedInput)
    );
    assert_eq!(
        mask.input(TerminalInput::Text('x'), &show, 2),
        Err(TerminalError::UnsupportedInput)
    );
    assert_eq!(mask.show(), Some(&show));
    assert_eq!(
        mask.input(TerminalInput::NextClause, &show, 3).unwrap(),
        TerminalInputOutcome::Redraw
    );
    assert_eq!(mask.focused(), None);
    assert!(mask.show().is_none());
}
#[test]
fn flush_failure_cannot_produce_receipt_or_leave_input_active() {
    struct FailFlush;
    impl Write for FailFlush {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::ErrorKind::BrokenPipe.into())
        }
    }
    let mut mask = TerminalFaceMask::prepare(face(1), 60, 12).unwrap();
    let old = bind(&mut mask);
    let prepared = fixture::prepared(mask.presentation());
    assert!(matches!(
        mask.render(&mut FailFlush, &prepared),
        Err(TerminalError::Io(std::io::ErrorKind::BrokenPipe))
    ));
    assert!(matches!(
        mask.input(TerminalInput::Apply, &old, 0),
        Err(TerminalError::StaleShow)
    ));
}
#[test]
fn only_latest_effect_receipt_and_available_exact_show_bind() {
    let mut mask = TerminalFaceMask::prepare(face(1), 60, 12).unwrap();
    let prepared = fixture::prepared(mask.presentation());
    let first = mask.render(&mut Vec::new(), &prepared).unwrap();
    let _second = mask.render(&mut Vec::new(), &prepared).unwrap();
    let show = fixture::show(mask.presentation());
    assert_eq!(mask.bind_show(first, show), Err(TerminalError::StaleShow));
    let receipt = mask.render(&mut Vec::new(), &prepared).unwrap();
    assert_eq!(
        mask.bind_show(receipt, prepared.clone()),
        Err(TerminalError::UnacknowledgedShow)
    );
    let stale = fixture::prepared(&face(2));
    assert!(matches!(
        mask.render(&mut Vec::new(), &stale),
        Err(TerminalError::StaleFace)
    ));
}
#[test]
fn typed_edits_preserve_unicode_and_exact_action_correlation() {
    let mut mask = TerminalFaceMask::prepare(face(1), 60, 12).unwrap();
    // Direct semantic controls are resolved from the shared reading plan.
    mask.focus = Some(TerminalControl {
        action: 0,
        argument: Some(0),
    });
    for key in [
        TerminalInput::Text('é'),
        TerminalInput::Text('界'),
        TerminalInput::Backspace,
    ] {
        let show = bind(&mut mask);
        mask.input(key, &show, 7).unwrap();
    }
    let show = bind(&mut mask);
    let TerminalInputOutcome::Interaction(interaction) =
        mask.input(TerminalInput::Apply, &show, 7).unwrap()
    else {
        panic!("expected typed interaction")
    };
    interaction
        .validate_against(mask.presentation(), &show)
        .unwrap();
    assert_eq!(interaction.arguments[0].value, "é".as_bytes());
    assert_eq!(interaction.action_id, "edit");
    assert_eq!(interaction.target, "a-field");
    assert!(matches!(
        mask.input(TerminalInput::Apply, &show, 8),
        Err(TerminalError::StaleShow)
    ));
}
#[test]
fn controls_follow_canonical_clause_order_and_page_navigation_has_no_action() {
    let mut mask = TerminalFaceMask::prepare(face(1), 40, 8).unwrap();
    let mut expected = Vec::new();
    for row in &mask.document {
        if let Some(c) = row.control {
            if !expected.contains(&c) {
                expected.push(c);
            }
        }
    }
    for control in expected {
        let show = bind(&mut mask);
        assert_eq!(
            mask.input(TerminalInput::NextControl, &show, 0).unwrap(),
            TerminalInputOutcome::Redraw
        );
        assert_eq!(mask.focused(), Some(control));
    }
    let show = bind(&mut mask);
    assert_eq!(
        mask.input(TerminalInput::PageDown, &show, 0).unwrap(),
        TerminalInputOutcome::Redraw
    );
    assert!(mask.focused().is_none());
    assert!(mask.reading_clause_index().is_some());
}
#[test]
fn missing_or_oversized_input_is_refused_without_submission() {
    let mut mask = TerminalFaceMask::prepare(face(1), 60, 12).unwrap();
    mask.focus = Some(TerminalControl {
        action: 0,
        argument: Some(0),
    });
    let show = bind(&mut mask);
    assert!(matches!(
        mask.input(TerminalInput::Apply, &show, 0),
        Err(TerminalError::Interaction(
            FaceInteractionRefusal::MissingArgument
        ))
    ));
    mask.drafts[0][0] = Some(vec![b'x'; 64]);
    assert!(matches!(
        mask.input(TerminalInput::Text('x'), &show, 0),
        Err(TerminalError::InputPressure)
    ));
    assert_eq!(mask.drafts[0][0].as_ref().unwrap().len(), 64);
}
#[test]
fn key_decoder_is_incremental_bounded_and_distinguishes_escape() {
    let mut keys = TerminalKeyboard::default();
    for (bytes, expected) in [
        (b"\x1b[Z".as_slice(), TerminalInput::PreviousControl),
        (b"\x1b[B".as_slice(), TerminalInput::NextClause),
        (b"\x1bOQ".as_slice(), TerminalInput::Inspect),
        ("é".as_bytes(), TerminalInput::Text('é')),
    ] {
        let mut last = None;
        for b in bytes {
            last = keys.push(*b).unwrap();
        }
        assert_eq!(last, Some(expected));
    }
    assert_eq!(keys.push(27).unwrap(), None);
    assert_eq!(keys.escape_timeout(), Some(TerminalInput::Escape));
    assert!(keys.push(255).is_err());
    assert_eq!(keys.push(3).unwrap(), Some(TerminalInput::Quit));
}
#[test]
fn document_escapes_terminal_control_bytes_in_face_wording() {
    let face = face(1);
    // Rebuild immutable identity through the canonical constructor.
    let mut subjects = face.subjects.clone();
    subjects[0].name = "hostile\u{1b}[2J".into();
    let changed = Presentation::new_with_semantics(
        face.revision,
        face.basis.clone(),
        subjects,
        face.relationships.clone(),
        face.properties.clone(),
        face.text.clone(),
        face.actions.clone(),
        face.disclosures.clone(),
    )
    .unwrap();
    let mask = TerminalFaceMask::prepare(changed, 60, 12).unwrap();
    assert!(mask.document.iter().all(|r| !r.text.contains('\u{1b}')));
    assert!(mask.document.iter().any(|r| r.text.contains("\\u{1b}")));
}
