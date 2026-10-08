use conduit_core::kind_id;
use conduit_presentation::{
    plan_face_utterances, FaceReadingCommand, FaceReadingCursor, FaceReadingRefusal, Presentation,
    PresentationAction, PresentationActionAvailability, PresentationBasis,
    PresentationDisclosureLevel, PresentationProperty, PresentationPropertyValue,
    PresentationRelationship, PresentationRelationshipKind, PresentationRole, PresentationSubject,
    PresentationText,
};

fn basis() -> PresentationBasis {
    PresentationBasis {
        body_id: None,
        wake_id: None,
        source_document_id: None,
        checked_plot_id: None,
        expanded_plot_id: None,
        plan_id: None,
        active_play_id: None,
        sign_ids: vec![],
    }
}

fn face(revision: u64, result: &str) -> Presentation {
    Presentation::new_with_semantics(
        revision,
        basis(),
        vec![
            PresentationSubject {
                identity: "main".into(),
                role: PresentationRole::Semantic(kind_id("document/main")),
                name: "Field clock".into(),
            },
            PresentationSubject {
                identity: "article".into(),
                role: PresentationRole::Semantic(kind_id("document/article")),
                name: "Current reading".into(),
            },
            PresentationSubject {
                identity: "nav".into(),
                role: PresentationRole::Semantic(kind_id("document/navigation")),
                name: "Controls".into(),
            },
        ],
        vec![
            PresentationRelationship {
                source: "main".into(),
                target: "article".into(),
                kind: PresentationRelationshipKind::Contains,
            },
            PresentationRelationship {
                source: "main".into(),
                target: "nav".into(),
                kind: PresentationRelationshipKind::Contains,
            },
        ],
        vec![PresentationProperty {
            subject: "article".into(),
            name: "interval-seconds".into(),
            value: PresentationPropertyValue::Count(15),
        }],
        vec![PresentationText {
            subject: "article".into(),
            text: result.into(),
        }],
        vec![PresentationAction {
            identity: "clock/change-interval".into(),
            intent: "clock/change-interval".into(),
            target: "nav".into(),
            name: "Change interval".into(),
            arguments: vec![],
            disclosure: PresentationDisclosureLevel::CurrentAction,
            availability: PresentationActionAvailability::Unavailable {
                reason_code: "clock/lulled".into(),
                explanation: "Wake the Body first.".into(),
            },
        }],
        vec![],
    )
    .unwrap()
}

#[test]
fn reads_every_face_clause_independent_of_viewport() {
    let face = face(1, "The last tick completed.");
    let expected = plan_face_utterances(&face).unwrap();
    let mut cursor = FaceReadingCursor::new(&face).unwrap();
    cursor.command(&face, FaceReadingCommand::ReadAll).unwrap();
    let mut actual = vec![];
    while let Some(clause) = cursor.next_read_clause(&face).unwrap() {
        actual.push(clause.clone());
    }
    assert_eq!(actual, expected.clauses);
    let whole_view = actual
        .iter()
        .map(|item| item.text.as_str())
        .collect::<Vec<_>>();
    for expected in [
        "Role document/main: Field clock.",
        "Role document/article: Current reading.",
        "Role document/navigation: Controls.",
        "Field clock contains Current reading.",
        "Current reading has property interval-seconds: count 15.",
        "The last tick completed.",
        "Unavailable action: Change interval, for Controls. Reason clock/lulled: Wake the Body first.",
    ] {
        assert!(whole_view.contains(&expected), "missing {expected}");
    }
    assert!(cursor.next_read_clause(&face).unwrap().is_none());
}

#[test]
fn read_all_reaches_the_last_item_of_a_long_view() {
    let subjects = (0..128)
        .map(|number| PresentationSubject {
            identity: format!("item/{number:03}"),
            role: PresentationRole::Item,
            name: format!("Reading item {number}"),
        })
        .collect();
    let face = Presentation::new(1, basis(), subjects, vec![], vec![], vec![]).unwrap();
    let mut cursor = FaceReadingCursor::new(&face).unwrap();
    cursor.command(&face, FaceReadingCommand::ReadAll).unwrap();
    let mut count = 0;
    let mut last = String::new();
    while let Some(clause) = cursor.next_read_clause(&face).unwrap() {
        count += 1;
        last = clause.text.clone();
    }
    assert_eq!(count, 128);
    assert_eq!(last, "Item: Reading item 127.");
}

#[test]
fn reads_only_primary_items_from_one_exact_face_and_keeps_provenance() {
    let subjects = (0..20)
        .map(|number| PresentationSubject {
            identity: format!("item/{number}"),
            role: PresentationRole::Item,
            name: format!("Task {number}"),
        })
        .collect::<Vec<_>>();
    let disclosures = (0..20)
        .map(|number| conduit_presentation::PresentationDisclosure {
            subject: format!("item/{number}"),
            level: if number < 3 {
                PresentationDisclosureLevel::Primary
            } else {
                PresentationDisclosureLevel::SelectedDetail
            },
        })
        .collect();
    let face = Presentation::new_with_semantics(
        1,
        basis(),
        subjects,
        vec![],
        vec![],
        vec![],
        vec![],
        disclosures,
    )
    .unwrap();
    let mut cursor = FaceReadingCursor::new(&face).unwrap();
    cursor
        .command(
            &face,
            FaceReadingCommand::ReadRoleAtDisclosure(
                PresentationRole::Item,
                PresentationDisclosureLevel::Primary,
            ),
        )
        .unwrap();
    let mut names = vec![];
    let first = cursor.next_read_clause(&face).unwrap().unwrap();
    let conduit_presentation::FaceUtteranceProvenance::Subject(source) = &first.provenance else {
        panic!("selection must retain subject provenance");
    };
    names.push(source.identity().clone());
    let mut stale = face.clone();
    stale.revision += 1;
    assert_eq!(
        cursor.next_read_clause(&stale),
        Err(FaceReadingRefusal::StaleFace)
    );
    while let Some(clause) = cursor.next_read_clause(&face).unwrap() {
        let conduit_presentation::FaceUtteranceProvenance::Subject(source) = &clause.provenance
        else {
            panic!("selection must retain subject provenance");
        };
        names.push(source.identity().clone());
    }
    assert_eq!(names, ["item/0", "item/1", "item/2"]);
    assert!(!cursor.has_pending());
    let no_match = cursor
        .command(
            &face,
            FaceReadingCommand::ReadRoleAtDisclosure(
                PresentationRole::Status,
                PresentationDisclosureLevel::Primary,
            ),
        )
        .unwrap();
    assert!(no_match.at_boundary);
    assert!(!no_match.reading);
    assert_eq!(cursor.focused_clause().text, "Item: Task 2.");
}

#[test]
fn moves_by_semantic_subject_and_action_with_repeat_and_stop() {
    let face = face(2, "Tick.");
    let mut cursor = FaceReadingCursor::new(&face).unwrap();
    let outcome = cursor
        .command(&face, FaceReadingCommand::NextSubject)
        .unwrap();
    assert!(!outcome.at_boundary);
    assert_eq!(
        cursor.next_read_clause(&face).unwrap().unwrap().text,
        "Role document/article: Current reading."
    );
    cursor
        .command(&face, FaceReadingCommand::FocusSubject("main".into()))
        .unwrap();
    cursor
        .command(
            &face,
            FaceReadingCommand::NextRole(PresentationRole::Semantic(kind_id("document/article"))),
        )
        .unwrap();
    assert_eq!(
        cursor.next_read_clause(&face).unwrap().unwrap().text,
        "Role document/article: Current reading."
    );
    cursor.command(&face, FaceReadingCommand::Next).unwrap();
    assert_eq!(
        cursor.next_read_clause(&face).unwrap().unwrap().text,
        "Role document/navigation: Controls."
    );
    cursor.command(&face, FaceReadingCommand::Previous).unwrap();
    assert_eq!(
        cursor.next_read_clause(&face).unwrap().unwrap().text,
        "Role document/article: Current reading."
    );

    cursor
        .command(
            &face,
            FaceReadingCommand::FocusAction("clock/change-interval".into()),
        )
        .unwrap();
    let first = cursor.next_read_clause(&face).unwrap().unwrap().clone();
    assert!(first.text.starts_with("Unavailable action:"));
    cursor.command(&face, FaceReadingCommand::Repeat).unwrap();
    assert_eq!(cursor.next_read_clause(&face).unwrap(), Some(&first));
    assert!(
        cursor
            .command(&face, FaceReadingCommand::NextAction)
            .unwrap()
            .at_boundary
    );

    cursor.command(&face, FaceReadingCommand::ReadAll).unwrap();
    assert!(cursor.next_read_clause(&face).unwrap().is_some());
    let stopped = cursor.command(&face, FaceReadingCommand::Stop).unwrap();
    assert!(stopped.interrupted);
    assert!(!stopped.reading);
    assert!(cursor.next_read_clause(&face).unwrap().is_none());
}

#[test]
fn refuses_stale_revision_and_preserves_pending_read_on_unknown_focus() {
    let first = face(3, "Tick.");
    let later = face(4, "A new tick completed.");
    let mut cursor = FaceReadingCursor::new(&first).unwrap();
    cursor.command(&first, FaceReadingCommand::ReadAll).unwrap();
    assert_eq!(
        cursor.command(&first, FaceReadingCommand::FocusAction("absent".into())),
        Err(FaceReadingRefusal::UnknownAction)
    );
    assert_eq!(
        cursor.command(&later, FaceReadingCommand::Next),
        Err(FaceReadingRefusal::StaleFace)
    );
    assert_eq!(
        cursor.next_read_clause(&later),
        Err(FaceReadingRefusal::StaleFace)
    );
    assert!(cursor.next_read_clause(&first).unwrap().is_some());
    let stopped = cursor.command(&later, FaceReadingCommand::Stop).unwrap();
    assert!(stopped.interrupted);
}

#[test]
fn refresh_keeps_named_subject_but_never_carries_old_reading_or_indexed_fact() {
    let first = face(5, "Old result.");
    let later = face(6, "New result.");
    let mut cursor = FaceReadingCursor::new(&first).unwrap();
    cursor
        .command(&first, FaceReadingCommand::FocusSubject("article".into()))
        .unwrap();
    let refreshed = cursor.refresh(&later).unwrap();
    assert!(refreshed.retained_focus);
    assert!(refreshed.interrupted);
    assert_eq!(
        cursor.focused_clause().text,
        "Role document/article: Current reading."
    );
    assert!(cursor.next_read_clause(&later).unwrap().is_none());

    // Move to the new indexed text clause, whose index alone is not a stable
    // semantic focus across a later revision.
    cursor.command(&later, FaceReadingCommand::ReadAll).unwrap();
    while let Some(clause) = cursor.next_read_clause(&later).unwrap() {
        if clause.text == "New result." {
            break;
        }
    }
    let newest = face(7, "Another result.");
    let refreshed = cursor.refresh(&newest).unwrap();
    assert!(!refreshed.retained_focus);
    assert_eq!(refreshed.focused_clause, 0);
    assert_eq!(
        cursor.focused_clause().text,
        "Role document/main: Field clock."
    );
}
