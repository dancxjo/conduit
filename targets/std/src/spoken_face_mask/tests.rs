use super::*;

#[path = "../../../../semantics/presentation/tests/common/mod.rs"]
mod common;

use conduit_core::{kind_id, CheckedValueContract, ValueConstraint};
use conduit_presentation::{
    FaceActionArgument, ManifestationLifecycle, PresentationAction, PresentationDisclosure,
    PresentationDisclosureLevel, PresentationProperty, PresentationPropertyValue,
    PresentationRelationship, PresentationRelationshipKind, PresentationRole, PresentationSubject,
    PresentationText,
};

fn face_with_action() -> (Presentation, MaskShow) {
    let plot = common::checked_renderer_plot();
    let plan = common::plan_for(
        &plot,
        common::host(
            "linux-host",
            "linux-boot",
            "renderer-wayland",
            "presentation/renderer-wayland@1",
            "patchbay-native/wayland@1",
            "presentation/base/wayland-surface@1",
            common::WAYLAND_RESOURCE,
        ),
    );
    let base = common::presentation(&plot, &plan);
    let mut basis = base.basis;
    // This is a Host-owned Crèche Face. No Body exists at this stage.
    basis.body_id = None;
    basis.wake_id = None;
    let face = Presentation::new_with_semantics(
        1,
        basis,
        vec![
            PresentationSubject {
                identity: "patchbay/plot".into(),
                role: PresentationRole::Region,
                name: "Current view".into(),
            },
            PresentationSubject {
                identity: "arrival".into(),
                role: PresentationRole::Region,
                name: "Welcome".into(),
            },
            PresentationSubject {
                identity: "draft".into(),
                role: PresentationRole::TextEntry,
                name: "Body name".into(),
            },
        ],
        vec![],
        vec![PresentationProperty {
            subject: "draft".into(),
            name: "current name".into(),
            value: PresentationPropertyValue::Text("New Body".into()),
        }],
        vec![PresentationText {
            subject: "arrival".into(),
            text: "Choose a name, then decide when to begin.".into(),
        }],
        vec![
            PresentationAction {
                identity: "birth/set-name".into(),
                intent: "birth/set-name".into(),
                target: "draft".into(),
                name: "Set Body name".into(),
                arguments: vec![
                    FaceActionArgument::text("name".into(), "Body name".into(), 1, 32).unwrap(),
                ],
                disclosure: PresentationDisclosureLevel::CurrentAction,
                availability: PresentationActionAvailability::Available,
            },
            PresentationAction {
                identity: "birth/confirm".into(),
                intent: "birth/confirm".into(),
                target: "arrival".into(),
                name: "Create Body".into(),
                arguments: vec![],
                disclosure: PresentationDisclosureLevel::CurrentAction,
                availability: PresentationActionAvailability::Available,
            },
        ],
        vec![],
    )
    .unwrap();
    let show = common::available_mask_show(&face);
    (face, show)
}

#[test]
fn mechanical_projection_matches_interactive_wording_before_any_show() {
    let (face, show) = face_with_action();
    let projected = mechanical_face_clauses(&face).unwrap();
    let reader = SpokenFaceSession::new(face, show).unwrap();
    assert_eq!(projected, reader.voiced);
    assert!(projected
        .iter()
        .any(|clause| clause.contains("Create Body")));
}

#[test]
fn direct_opening_leads_with_context_then_result_and_leaves_detail_to_read_all() {
    let (base, _) = face_with_action();
    let mut actions = base.actions.clone();
    actions[0].availability = PresentationActionAvailability::Unavailable {
        reason_code: "not-now".into(),
        explanation: "This action cannot be used now.".into(),
    };
    let face = Presentation::new_with_semantics(
        2,
        base.basis,
        vec![
            PresentationSubject {
                identity: "body".into(),
                role: PresentationRole::Body,
                name: "Current Body".into(),
            },
            PresentationSubject {
                identity: "result".into(),
                role: PresentationRole::Status,
                name: "Result".into(),
            },
            PresentationSubject {
                identity: "context".into(),
                role: PresentationRole::Region,
                name: "Groceries".into(),
            },
            PresentationSubject {
                identity: "draft".into(),
                role: PresentationRole::TextEntry,
                name: "Draft".into(),
            },
            PresentationSubject {
                identity: "arrival".into(),
                role: PresentationRole::Region,
                name: "Arrival".into(),
            },
        ],
        vec![],
        vec![],
        vec![
            PresentationText {
                subject: "body".into(),
                text: "Lulled with one resident Plot.".into(),
            },
            PresentationText {
                subject: "result".into(),
                text: "Two things remain.".into(),
            },
            PresentationText {
                subject: "context".into(),
                text: "Groceries list.".into(),
            },
        ],
        actions,
        vec![
            PresentationDisclosure {
                subject: "body".into(),
                level: PresentationDisclosureLevel::Primary,
            },
            PresentationDisclosure {
                subject: "result".into(),
                level: PresentationDisclosureLevel::Primary,
            },
            PresentationDisclosure {
                subject: "context".into(),
                level: PresentationDisclosureLevel::Context,
            },
        ],
    )
    .unwrap();
    let opening = primary_face_clauses(&face).unwrap().join(" ");
    assert!(opening.starts_with("Groceries list. Two things remain."));
    assert!(!opening.contains("resident Plot"));
    assert!(!opening.contains("Unavailable"));
    assert!(!opening.contains("birth/"));
    let complete = mechanical_face_clauses(&face).unwrap().join(" ");
    assert!(complete.contains("Unavailable"));
}

#[test]
fn direct_opening_offers_content_action_without_generic_context_navigation() {
    let (base, _) = face_with_action();
    let make_face = |actions| {
        Presentation::new_with_semantics(
            base.revision,
            base.basis.clone(),
            base.subjects.clone(),
            base.relationships.clone(),
            base.properties.clone(),
            base.text.clone(),
            actions,
            vec![PresentationDisclosure {
                subject: "arrival".into(),
                level: PresentationDisclosureLevel::Context,
            }],
        )
        .unwrap()
    };
    let generic = base.actions[1].clone();
    let content = base.actions[0].clone();
    let opening = primary_face_clauses(&make_face(vec![generic.clone(), content])).unwrap();
    assert!(opening
        .iter()
        .any(|clause| clause == "You can Set Body name."));
    assert!(!opening
        .iter()
        .any(|clause| clause == "You can Create Body."));

    let opening = primary_face_clauses(&make_face(vec![generic])).unwrap();
    assert!(!opening.iter().any(|clause| clause.starts_with("You can ")));
}

#[test]
fn direct_opening_bounds_long_collections_without_losing_full_reading() {
    let (base, _) = face_with_action();
    let mut subjects = base.subjects;
    let mut disclosures = base.disclosures;
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
        let identity = format!("item/{index}");
        subjects.push(PresentationSubject {
            identity: identity.clone(),
            role: PresentationRole::Item,
            name: if index < 3 {
                format!("Open item {index}")
            } else {
                format!("Completed item {index}")
            },
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
    let face = Presentation::new_with_semantics(
        2,
        base.basis,
        subjects,
        base.relationships,
        base.properties,
        vec![PresentationText {
            subject: "todo/status".into(),
            text: "3 things left · 17 completed".into(),
        }],
        vec![PresentationAction {
            identity: "todo.add".into(),
            intent: "todo/add@1".into(),
            target: "todo/list".into(),
            name: "add an item".into(),
            arguments: vec![
                FaceActionArgument::text("text".into(), "Item text".into(), 1, 256).unwrap(),
            ],
            disclosure: PresentationDisclosureLevel::CurrentAction,
            availability: PresentationActionAvailability::Available,
        }],
        disclosures,
    )
    .unwrap();
    let opening = primary_face_clauses(&face).unwrap().join(" ");
    assert!(opening.starts_with("Groceries. 3 things left · 17 completed"));
    assert!(!opening.contains("Progress"));
    for index in 0..3 {
        assert!(opening.contains(&format!("Open item {index}.")));
    }
    assert!(!opening.contains("Completed item"));
    assert!(opening.contains("You can add an item."));
    let direct = crate::direct_spoken_mask_runtime::prepare_wording_items(&face).unwrap();
    let direct = std::str::from_utf8(direct.front().unwrap()).unwrap();
    assert!(direct.contains("Open item 2."));
    assert!(!direct.contains("Completed item"));
    assert!(mechanical_face_clauses(&face)
        .unwrap()
        .join(" ")
        .contains("Completed item 17."));
}

#[test]
fn read_current_items_uses_the_same_show_without_completed_detail() {
    let (base, _) = face_with_action();
    let mut subjects = base.subjects;
    let mut disclosures = base.disclosures;
    for index in 0..20 {
        subjects.push(PresentationSubject {
            identity: format!("item/{index}"),
            role: PresentationRole::Item,
            name: format!("Task {index}"),
        });
        disclosures.push(PresentationDisclosure {
            subject: format!("item/{index}"),
            level: if index < 3 {
                PresentationDisclosureLevel::Primary
            } else {
                PresentationDisclosureLevel::SelectedDetail
            },
        });
    }
    let face = Presentation::new_with_semantics(
        2,
        base.basis,
        subjects,
        base.relationships,
        base.properties,
        base.text,
        base.actions,
        disclosures,
    )
    .unwrap();
    let show = common::available_mask_show(&face);
    let mut reader = SpokenFaceSession::new(face.clone(), show.clone()).unwrap();
    reader
        .command(&face, &show, ReaderCommand::ReadCurrentItems, 1)
        .unwrap();
    let readout = reader.take_text_readout().unwrap().unwrap();
    assert_eq!(readout.face_id, face.identity.as_str());
    assert_eq!(readout.show_id, show.show_id.as_str());
    assert_eq!(
        readout.clauses,
        ["Task 0, item.", "Task 1, item.", "Task 2, item."]
    );
    reader
        .command(&face, &show, ReaderCommand::ReadAll, 2)
        .unwrap();
    let complete = reader.take_text_readout().unwrap().unwrap();
    assert!(complete.clauses.iter().any(|item| item == "Task 19, item."));

    let mut finished = face;
    finished.revision += 1;
    for disclosure in &mut finished.disclosures {
        if disclosure.subject.starts_with("item/") {
            disclosure.level = PresentationDisclosureLevel::SelectedDetail;
        }
    }
    let finished = Presentation::new_with_semantics(
        finished.revision,
        finished.basis,
        finished.subjects,
        finished.relationships,
        finished.properties,
        finished.text,
        finished.actions,
        finished.disclosures,
    )
    .unwrap();
    let finished_show = common::available_mask_show(&finished);
    let mut reader = SpokenFaceSession::new(finished.clone(), finished_show.clone()).unwrap();
    reader
        .command(
            &finished,
            &finished_show,
            ReaderCommand::ReadCurrentItems,
            3,
        )
        .unwrap();
    assert_eq!(
        reader.take_text_readout().unwrap().unwrap().clauses,
        ["No current items."]
    );
}

#[test]
fn long_primary_list_reads_bounded_items_and_offers_exact_detail_command() {
    let (base, _) = face_with_action();
    let mut subjects = base.subjects;
    let mut disclosures = base.disclosures;
    subjects.push(PresentationSubject {
        identity: "todo/status".into(),
        role: PresentationRole::Status,
        name: "4 remaining".into(),
    });
    disclosures.push(PresentationDisclosure {
        subject: "todo/status".into(),
        level: PresentationDisclosureLevel::Primary,
    });
    for index in 0..4 {
        let identity = format!("item/{index}");
        subjects.push(PresentationSubject {
            identity: identity.clone(),
            role: PresentationRole::Item,
            name: format!("Open item {index}"),
        });
        disclosures.push(PresentationDisclosure {
            subject: identity,
            level: PresentationDisclosureLevel::Primary,
        });
    }
    let face = Presentation::new_with_semantics(
        2,
        base.basis,
        subjects,
        base.relationships,
        base.properties,
        vec![PresentationText {
            subject: "todo/status".into(),
            text: "4 remaining".into(),
        }],
        base.actions,
        disclosures,
    )
    .unwrap();
    let opening = primary_face_clauses(&face).unwrap().join(" ");
    assert!(opening.starts_with("4 remaining"));
    assert!(opening.contains("Type read current items to hear what remains."));
    assert!(opening.contains("Open item 0."));
    assert!(opening.contains("Open item 2."));
    assert!(!opening.contains("Open item 3."));
    let direct = crate::direct_spoken_mask_runtime::prepare_wording_items(&face).unwrap();
    let direct = std::str::from_utf8(direct.front().unwrap()).unwrap();
    assert!(direct.contains("Type read current items to hear what remains."));
}

#[test]
fn accepted_wording_keeps_exact_text_and_current_show_in_one_bounded_flow() {
    let (face, show) = face_with_action();
    let wording = "Welcome to your Body. Its clock is ready, and the current state is retained.";
    let batch =
        SpokenBatch::from_accepted_wording(&face, &show, wording, "accepted/model/one".into())
            .unwrap();
    assert!(batch.segments.len() > 1);
    assert_eq!(
        batch
            .segments
            .iter()
            .map(|item| item.segment.text.as_str())
            .collect::<String>(),
        wording
    );
    assert_eq!(batch.source_show_id, show.show_id.as_str());
    assert_eq!(
        batch.segments.last().unwrap().segment.reason,
        SpeechCommitReason::FinalFlush
    );
    assert!(batch.validate(&face, &show).is_ok());
    assert!(SpokenBatch::from_accepted_wording(&face, &show, "", "empty".into()).is_err());
    assert!(
        SpokenBatch::from_accepted_wording(&face, &show, &"x".repeat(1025), "too-long".into())
            .is_err()
    );
}

#[test]
fn screen_free_reader_reaches_below_viewport_and_navigates_semantic_roles() {
    let (base, _) = face_with_action();
    let mut subjects = base.subjects.clone();
    for subject in &mut subjects {
        subject.role = match subject.identity.as_str() {
            "arrival" => PresentationRole::Semantic(kind_id("document/main")),
            "draft" => PresentationRole::Semantic(kind_id("document/article")),
            _ => PresentationRole::Semantic(kind_id("document/navigation")),
        };
    }
    subjects.extend((0..80).map(|number| PresentationSubject {
        identity: format!("item/{number:03}"),
        role: PresentationRole::Item,
        name: format!("Item {number}"),
    }));
    let mut actions = base.actions.clone();
    actions[1].availability = PresentationActionAvailability::Unavailable {
        reason_code: "birth/name-missing".into(),
        explanation: "Give the Body a name first.".into(),
    };
    let face = Presentation::new_with_semantics(
        base.revision + 1,
        base.basis,
        subjects,
        vec![PresentationRelationship {
            source: "arrival".into(),
            target: "draft".into(),
            kind: PresentationRelationshipKind::Contains,
        }],
        base.properties,
        base.text,
        actions,
        base.disclosures,
    )
    .unwrap();
    let show = common::available_mask_show(&face);
    let mut reader = SpokenFaceSession::new(face.clone(), show.clone()).unwrap();
    reader
        .command(&face, &show, ReaderCommand::ReadAll, 1)
        .unwrap();
    let clauses = reader.take_text_readout().unwrap().unwrap().clauses;
    assert!(clauses.iter().any(|clause| clause == "Item 79, item."));
    assert!(clauses.iter().any(|clause| clause == "Welcome, main."));
    assert!(clauses.iter().any(|clause| clause == "Body name, article."));
    assert!(clauses
        .iter()
        .any(|clause| clause == "Welcome contains Body name."));
    assert!(clauses
        .iter()
        .any(|clause| clause.contains("current name: New Body")));
    assert!(clauses
        .iter()
        .any(|clause| clause.contains("Unavailable: Give the Body a name first.")));

    reader
        .command(
            &face,
            &show,
            ReaderCommand::FocusSubject("arrival".into()),
            2,
        )
        .unwrap();
    reader.take_text_readout().unwrap();
    reader
        .command(
            &face,
            &show,
            ReaderCommand::NextRole(PresentationRole::Semantic(kind_id("document/article"))),
            3,
        )
        .unwrap();
    assert_eq!(
        reader.take_text_readout().unwrap().unwrap().clauses,
        ["Body name, article."]
    );
}

#[test]
fn spoken_reader_names_finite_text_choices_and_refuses_unoffered_value() {
    let (base, _) = face_with_action();
    let mut actions = base.actions.clone();
    actions[0].name = "Change interval".into();
    actions[0].arguments[0].name = "interval-ms".into();
    actions[0].arguments[0].value_name = "Interval in milliseconds".into();
    actions[0].arguments[0].contract = CheckedValueContract::new(
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
    let face = Presentation::new_with_semantics(
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
    let show = common::available_mask_show(&face);
    let mut reader = SpokenFaceSession::new(face.clone(), show.clone()).unwrap();
    reader
        .command(&face, &show, ReaderCommand::ReadAll, 1)
        .unwrap();
    let clauses = reader.take_text_readout().unwrap().unwrap().clauses;
    assert!(clauses.iter().any(|clause| clause
        == "Interval in milliseconds. Choose one of: 1000, 2000, 250, 500. Enter it with edit interval-ms, then activate Change interval."));

    reader
        .command(
            &face,
            &show,
            ReaderCommand::FocusAction("birth/set-name".into()),
            2,
        )
        .unwrap();
    reader.take_text_readout().unwrap();
    assert_eq!(
        reader.command(
            &face,
            &show,
            ReaderCommand::Edit {
                argument: "interval-ms".into(),
                value: b"750".to_vec(),
            },
            3
        ),
        Err(SpokenFaceRefusal::InvalidValue)
    );
    reader
        .command(
            &face,
            &show,
            ReaderCommand::Edit {
                argument: "interval-ms".into(),
                value: b"500".to_vec(),
            },
            4,
        )
        .unwrap();
    assert!(reader.take_text_readout().unwrap().unwrap().clauses[0].contains("ready"));

    let mut boolean = face.actions.clone();
    boolean[0].arguments[0].value_name = "Include Plot".into();
    boolean[0].arguments[0].contract =
        CheckedValueContract::new(kind_id("value/bool"), 1, vec![]).unwrap();
    let boolean_face = Presentation::new_with_semantics(
        face.revision + 1,
        face.basis.clone(),
        face.subjects.clone(),
        face.relationships.clone(),
        face.properties.clone(),
        face.text.clone(),
        boolean,
        face.disclosures.clone(),
    )
    .unwrap();
    let boolean_show = common::available_mask_show(&boolean_face);
    let mut boolean_reader =
        SpokenFaceSession::new(boolean_face.clone(), boolean_show.clone()).unwrap();
    boolean_reader
        .command(&boolean_face, &boolean_show, ReaderCommand::ReadAll, 1)
        .unwrap();
    let boolean_clauses = boolean_reader.take_text_readout().unwrap().unwrap().clauses;
    assert!(boolean_clauses.iter().any(|clause| {
        clause == "Include Plot. Choose true or false with edit interval-ms, then activate Change interval."
    }));

    // More than eight exact choices stay on the Face's generic bounded
    // contract phrasing rather than becoming an unwieldy spoken list.
    let mut many = face.actions.clone();
    many[0].arguments[0].contract = CheckedValueContract::new(
        kind_id(UTF8_TEXT_VALUE_KIND),
        2,
        vec![ValueConstraint::CanonicalMembership {
            members: (0..9).map(|value| value.to_string().into_bytes()).collect(),
            negated: false,
        }],
    )
    .unwrap();
    let crowded = Presentation::new_with_semantics(
        face.revision + 1,
        face.basis,
        face.subjects,
        face.relationships,
        face.properties,
        face.text,
        many,
        face.disclosures,
    )
    .unwrap();
    let crowded_show = common::available_mask_show(&crowded);
    let mut crowded_reader = SpokenFaceSession::new(crowded.clone(), crowded_show.clone()).unwrap();
    crowded_reader
        .command(&crowded, &crowded_show, ReaderCommand::ReadAll, 1)
        .unwrap();
    let generic = crowded_reader.take_text_readout().unwrap().unwrap().clauses;
    assert!(generic
        .iter()
        .any(|clause| clause.contains("canonical values")));
    assert!(!generic
        .iter()
        .any(|clause| clause.contains("Choose one of:")));

    let mut disguised = crowded.actions.clone();
    disguised[0].arguments[0].contract = CheckedValueContract::new(
        kind_id(UTF8_TEXT_VALUE_KIND),
        32,
        vec![ValueConstraint::CanonicalMembership {
            members: vec!["500\u{202e}0".as_bytes().to_vec()],
            negated: false,
        }],
    )
    .unwrap();
    let disguised = Presentation::new_with_semantics(
        crowded.revision + 1,
        crowded.basis,
        crowded.subjects,
        crowded.relationships,
        crowded.properties,
        crowded.text,
        disguised,
        crowded.disclosures,
    )
    .unwrap();
    let disguised_show = common::available_mask_show(&disguised);
    let mut disguised_reader =
        SpokenFaceSession::new(disguised.clone(), disguised_show.clone()).unwrap();
    disguised_reader
        .command(&disguised, &disguised_show, ReaderCommand::ReadAll, 1)
        .unwrap();
    let generic = disguised_reader
        .take_text_readout()
        .unwrap()
        .unwrap()
        .clauses;
    assert!(generic
        .iter()
        .any(|clause| clause.contains("canonical values")));
    assert!(!generic
        .iter()
        .any(|clause| clause.contains("Choose one of:")));
}

fn receipt(packet: &SpokenSegment) -> SpokenAudioReceipt {
    SpokenAudioReceipt {
        stream_identity: packet.segment.stream_identity.clone(),
        sequence: packet.segment.sequence,
        source_show_id: packet.show_id.clone(),
        speech_plan_id: "plan/test-speech".into(),
        speech_play_id: "play/test-speech".into(),
        text_sha256: packet.text_sha256.clone(),
        provider_sha256: "b".repeat(64),
        pcm_sha256: format!("{:x}", Sha256::digest(packet.segment.text.as_bytes())),
        pcm_bytes: 256,
    }
}

fn fixture_batch_receipt(batch: &SpokenBatch) -> SpokenBatchAudioReceipt {
    SpokenBatchAudioReceipt {
        stream_identity: batch.stream_identity.clone(),
        source_show_id: batch.source_show_id.clone(),
        source_segments_sha256: batch.source_segments_sha256.clone(),
        speech_plan_id: "plan/fixture-speech".into(),
        speech_play_id: "play/fixture-speech".into(),
        provider_sha256: "b".repeat(64),
        wav_sha256: "a".repeat(64),
        wav_bytes: 300,
        pcm_bytes: 256,
        pcm_blocks: 1,
    }
}

#[test]
fn requested_items_close_before_later_non_item_face_clauses() {
    let (base, _) = face_with_action();
    let mut subjects = base.subjects;
    let mut disclosures = base.disclosures;
    for number in 18..=20 {
        let identity = format!("todo/item/{number}");
        subjects.push(PresentationSubject {
            identity: identity.clone(),
            role: PresentationRole::Item,
            name: format!("Long-list item {number}"),
        });
        disclosures.push(PresentationDisclosure {
            subject: identity,
            level: PresentationDisclosureLevel::Primary,
        });
    }
    let face = Presentation::new_with_semantics(
        base.revision + 1,
        base.basis,
        subjects,
        base.relationships,
        base.properties,
        base.text,
        base.actions,
        disclosures,
    )
    .unwrap();
    let show = common::available_mask_show(&face);
    let mut reader = SpokenFaceSession::new(face.clone(), show.clone()).unwrap();
    reader
        .command(&face, &show, ReaderCommand::ReadCurrentItems, 1)
        .unwrap();

    let batch = reader.next_batch_with_limits(4, 64).unwrap().unwrap();
    assert_eq!(batch.segments.len(), 3);
    for (number, segment) in (18..=20).zip(&batch.segments) {
        assert!(segment
            .segment
            .text
            .contains(&format!("Long-list item {number}")));
    }
    assert_eq!(
        batch.segments.last().unwrap().segment.reason,
        SpeechCommitReason::FinalFlush
    );
    let turn = reader
        .acknowledge_batch(SpokenBatchDelivery::Completed(fixture_batch_receipt(
            &batch,
        )))
        .unwrap()
        .unwrap();
    assert_eq!(turn.outcome, SpokenTurnOutcome::Completed);
    assert!(reader.next_batch_with_limits(4, 64).unwrap().is_none());
}

#[test]
fn smaller_closing_flows_preserve_all_face_text_and_cancel_between_batches() {
    let (face, show) = face_with_action();
    let mut reference = SpokenFaceSession::new(face.clone(), show.clone()).unwrap();
    reference
        .command(&face, &show, ReaderCommand::ReadAll, 1)
        .unwrap();
    let expected = reference
        .take_text_readout()
        .unwrap()
        .unwrap()
        .clauses
        .join("");
    let mut reader = SpokenFaceSession::new(face.clone(), show.clone()).unwrap();
    reader
        .command(&face, &show, ReaderCommand::ReadAll, 1)
        .unwrap();
    assert_eq!(
        reader.next_batch_with_limits(0, 64),
        Err(SpokenFaceRefusal::InvalidValue)
    );
    assert_eq!(
        reader.next_batch_with_limits(2, 3),
        Err(SpokenFaceRefusal::InvalidValue)
    );
    let mut actual = String::new();
    let mut identities = std::collections::BTreeSet::new();
    let mut terminal = None;
    while let Some(batch) = reader.next_batch_with_limits(2, 64).unwrap() {
        batch.validate(&face, &show).unwrap();
        assert!(identities.insert(batch.stream_identity.clone()));
        assert_eq!(
            reader.next_batch_with_limits(2, 64),
            Err(SpokenFaceRefusal::SpeechPressure)
        );
        for (index, segment) in batch.segments.iter().enumerate() {
            assert_eq!(segment.segment.sequence as usize, index);
            assert!(segment.segment.text.len() <= 64);
            actual.push_str(&segment.segment.text);
        }
        terminal = reader
            .acknowledge_batch(SpokenBatchDelivery::Completed(fixture_batch_receipt(
                &batch,
            )))
            .unwrap();
    }
    assert!(identities.len() > 1);
    assert_eq!(actual, expected);
    assert_eq!(terminal.unwrap().outcome, SpokenTurnOutcome::Completed);

    reader
        .command(&face, &show, ReaderCommand::ReadAll, 2)
        .unwrap();
    let first = reader.next_batch_with_limits(2, 64).unwrap().unwrap();
    assert!(reader
        .acknowledge_batch(SpokenBatchDelivery::Completed(fixture_batch_receipt(
            &first
        )))
        .unwrap()
        .is_none());
    let stopped = reader
        .command(&face, &show, ReaderCommand::Stop, 3)
        .unwrap();
    assert_eq!(
        stopped.interrupted.unwrap().outcome,
        SpokenTurnOutcome::Cancelled
    );
    assert!(reader.next_batch_with_limits(2, 64).unwrap().is_none());

    reader
        .command(&face, &show, ReaderCommand::ReadAll, 4)
        .unwrap();
    let failed = reader.next_batch_with_limits(2, 64).unwrap().unwrap();
    assert_eq!(failed.segments.len(), 2);
    let terminal = reader
        .acknowledge_batch(SpokenBatchDelivery::Failed("output capacity".into()))
        .unwrap()
        .unwrap();
    assert_eq!(
        terminal.outcome,
        SpokenTurnOutcome::Failed("output capacity".into())
    );
    assert_eq!(terminal.produced_pcm_bytes, 0);
    assert!(reader.next_batch_with_limits(2, 64).unwrap().is_none());
}

fn focus_action(session: &mut SpokenFaceSession, face: &Presentation, show: &MaskShow, id: &str) {
    let target = session
        .plan()
        .clauses
        .iter()
        .position(|clause| {
            matches!(
                &clause.provenance, FaceUtteranceProvenance::Action(value) if value.identity() == id
            )
        })
        .expect("action in Face");
    while session.focused_index() != target {
        let direction = if session.focused_index() < target {
            ReaderCommand::Next
        } else {
            ReaderCommand::Previous
        };
        session.command(face, show, direction, 0).unwrap();
        let packet = session.next_segment().unwrap().unwrap();
        session
            .acknowledge(SpokenDelivery::Completed(receipt(&packet)))
            .unwrap();
    }
}

#[test]
fn host_owned_arrival_is_read_completely_with_one_bounded_segment_in_flight() {
    let (face, show) = face_with_action();
    assert!(face.basis.body_id.is_none());
    let mut reader = SpokenFaceSession::new(face.clone(), show.clone()).unwrap();
    reader
        .command(&face, &show, ReaderCommand::ReadAll, 1)
        .unwrap();
    let mut words = String::new();
    let mut count = 0;
    while let Some(packet) = reader.next_segment().unwrap() {
        assert_eq!(
            reader.next_segment(),
            Err(SpokenFaceRefusal::SpeechPressure)
        );
        assert!(packet.segment.text.len() <= MAXIMUM_SPEAKABLE_SEGMENT_BYTES);
        words.push_str(&packet.segment.text);
        count += 1;
        let terminal = reader
            .acknowledge(SpokenDelivery::Completed(receipt(&packet)))
            .unwrap();
        if let Some(terminal) = terminal {
            assert_eq!(terminal.outcome, SpokenTurnOutcome::Completed);
            assert_eq!(terminal.completed_segments, count);
        }
    }
    assert!(words.contains("Welcome, region."));
    assert!(words.contains("Body name, text entry."));
    assert!(words.contains("Choose a name"));
    assert!(words.contains("Create Body"));
    assert!(words.contains("Body name. Enter 1 to 32 UTF-8 bytes"));
}

#[test]
fn host_owned_text_readout_and_focus_claim_no_audio() {
    let (face, show) = face_with_action();
    let mut reader = SpokenFaceSession::new(face.clone(), show.clone()).unwrap();
    reader
        .command(&face, &show, ReaderCommand::ReadAll, 1)
        .unwrap();
    let readout = reader.take_text_readout().unwrap().unwrap();
    assert_eq!(readout.face_id, face.identity.as_str());
    assert_eq!(readout.show_id, show.show_id.as_str());
    assert!(readout.clauses.join(" ").contains("Create Body"));
    assert!(reader.next_segment().unwrap().is_none());
    reader
        .command(
            &face,
            &show,
            ReaderCommand::FocusAction("birth/confirm".into()),
            2,
        )
        .unwrap();
    assert!(reader
        .take_text_readout()
        .unwrap()
        .unwrap()
        .clauses
        .join(" ")
        .contains("Create Body"));
    assert_eq!(
        reader.command(
            &face,
            &show,
            ReaderCommand::FocusAction("birth/absent".into()),
            3,
        ),
        Err(SpokenFaceRefusal::UnknownAction)
    );
}

#[test]
fn refused_navigation_or_input_preserves_the_current_read_all_turn() {
    let (face, show) = face_with_action();
    let mut reference = SpokenFaceSession::new(face.clone(), show.clone()).unwrap();
    reference
        .command(&face, &show, ReaderCommand::ReadAll, 1)
        .unwrap();
    let expected = reference.take_text_readout().unwrap().unwrap().clauses;

    let mut reader = SpokenFaceSession::new(face.clone(), show.clone()).unwrap();
    reader
        .command(&face, &show, ReaderCommand::ReadAll, 1)
        .unwrap();
    assert_eq!(
        reader.command(
            &face,
            &show,
            ReaderCommand::FocusSubject("absent".into()),
            2,
        ),
        Err(SpokenFaceRefusal::UnknownSubject)
    );
    assert_eq!(
        reader.command(&face, &show, ReaderCommand::FocusAction("absent".into()), 3,),
        Err(SpokenFaceRefusal::UnknownAction)
    );
    assert_eq!(
        reader.command(
            &face,
            &show,
            ReaderCommand::Edit {
                argument: "name".into(),
                value: b"Ada".to_vec(),
            },
            4,
        ),
        Err(SpokenFaceRefusal::NoActionInFocus)
    );
    assert_eq!(
        reader.command(&face, &show, ReaderCommand::Activate, 5),
        Err(SpokenFaceRefusal::NoActionInFocus)
    );
    assert_eq!(reader.focused_index(), 0);
    assert_eq!(
        reader.take_text_readout().unwrap().unwrap().clauses,
        expected
    );
}

#[test]
fn accepted_navigation_retires_only_the_old_turn_and_reads_the_new_focus() {
    let (face, show) = face_with_action();
    let mut reference = SpokenFaceSession::new(face.clone(), show.clone()).unwrap();
    reference
        .command(&face, &show, ReaderCommand::Next, 1)
        .unwrap();
    let expected = reference.take_text_readout().unwrap().unwrap().clauses;

    let mut reader = SpokenFaceSession::new(face.clone(), show.clone()).unwrap();
    reader
        .command(&face, &show, ReaderCommand::ReadAll, 1)
        .unwrap();
    let moved = reader
        .command(&face, &show, ReaderCommand::Next, 2)
        .unwrap();
    assert_eq!(
        moved.interrupted.unwrap().outcome,
        SpokenTurnOutcome::Cancelled
    );
    assert_eq!(moved.focused_clause, 1);
    assert_eq!(
        reader.take_text_readout().unwrap().unwrap().clauses,
        expected
    );
}

#[test]
fn inward_text_edit_and_birth_request_are_exact_typed_interactions() {
    let (face, show) = face_with_action();
    let mut reader = SpokenFaceSession::new(face.clone(), show.clone()).unwrap();
    focus_action(&mut reader, &face, &show, "birth/set-name");
    reader
        .command(
            &face,
            &show,
            ReaderCommand::Edit {
                argument: "name".into(),
                value: b"Aria".to_vec(),
            },
            2,
        )
        .unwrap();
    let packet = reader.next_segment().unwrap().unwrap();
    reader
        .acknowledge(SpokenDelivery::Completed(receipt(&packet)))
        .unwrap();
    let result = reader
        .command(&face, &show, ReaderCommand::Activate, 3)
        .unwrap();
    let interaction = result.interaction.unwrap();
    interaction.validate_against(&face, &show).unwrap();
    assert_eq!(interaction.arguments[0].value, b"Aria");
    let packet = reader.next_segment().unwrap().unwrap();
    reader
        .acknowledge(SpokenDelivery::Completed(receipt(&packet)))
        .unwrap();
    focus_action(&mut reader, &face, &show, "birth/confirm");
    let birth = reader
        .command(&face, &show, ReaderCommand::Activate, 4)
        .unwrap()
        .interaction
        .unwrap();
    assert_eq!(birth.action_id, "birth/confirm");
    assert!(birth.arguments.is_empty());
}

#[test]
fn stale_revision_cannot_speak_or_invoke_and_stop_requires_cancel_ack() {
    let (face, show) = face_with_action();
    let mut reader = SpokenFaceSession::new(face.clone(), show.clone()).unwrap();
    reader
        .command(&face, &show, ReaderCommand::ReadAll, 1)
        .unwrap();
    let packet = reader.next_segment().unwrap().unwrap();
    assert_eq!(
        conduit_tongues::decode_speakable_segment(&packet.encode_tongues().unwrap()).unwrap(),
        packet.segment
    );
    let stop = reader
        .command(&face, &show, ReaderCommand::Stop, 2)
        .unwrap();
    assert_eq!(
        stop.cancel_stream_identity.as_deref(),
        Some(packet.segment.stream_identity.as_str())
    );
    assert!(stop.interrupted.is_none());
    let terminal = reader
        .acknowledge(SpokenDelivery::Cancelled)
        .unwrap()
        .unwrap();
    assert_eq!(terminal.outcome, SpokenTurnOutcome::Cancelled);
    let mut stale = face.clone();
    stale.revision += 1;
    assert_eq!(
        reader.command(&stale, &show, ReaderCommand::ReadAll, 3),
        Err(SpokenFaceRefusal::StaleFace)
    );
    let changed_show = show
        .transition(
            ManifestationLifecycle::Closed,
            conduit_core::SignId::from("test/show-closed"),
        )
        .unwrap();
    assert_eq!(
        reader.command(&face, &changed_show, ReaderCommand::Activate, 4),
        Err(SpokenFaceRefusal::StaleShow)
    );
}

#[test]
fn source_audio_receipts_refuse_wrong_text_and_preserve_chunk_order() {
    let (face, show) = face_with_action();
    let mut reader = SpokenFaceSession::new(face.clone(), show.clone()).unwrap();
    reader
        .command(&face, &show, ReaderCommand::ReadAll, 1)
        .unwrap();
    let packet = reader.next_segment().unwrap().unwrap();
    let mut wrong = receipt(&packet);
    wrong.text_sha256 = "wrong".into();
    assert_eq!(
        reader.acknowledge(SpokenDelivery::Completed(wrong)),
        Err(SpokenFaceRefusal::SpeechReceipt)
    );
    assert_eq!(
        reader.next_segment(),
        Err(SpokenFaceRefusal::SpeechPressure)
    );
    reader
        .acknowledge(SpokenDelivery::Completed(receipt(&packet)))
        .unwrap();
    let second = reader.next_segment().unwrap().unwrap();
    assert_eq!(second.segment.sequence, packet.segment.sequence + 1);
    reader
        .acknowledge(SpokenDelivery::Failed("provider lost".into()))
        .unwrap();
    assert!(reader.next_segment().unwrap().is_none());
}

#[test]
fn utf8_chunking_never_splits_a_character() {
    let text = "é".repeat(700);
    let first = split_at_char_boundary(&text, MAXIMUM_SPEAKABLE_SEGMENT_BYTES);
    assert_eq!(first, 1_024);
    assert!(text.is_char_boundary(first));
}

#[test]
fn one_long_face_fact_streams_in_order_and_refresh_resets_indexed_focus() {
    let (base, _) = face_with_action();
    let mut properties = base.properties.clone();
    properties[0].value = PresentationPropertyValue::Text("é".repeat(512));
    let face = Presentation::new_with_semantics(
        base.revision + 1,
        base.basis.clone(),
        base.subjects.clone(),
        base.relationships.clone(),
        properties,
        base.text.clone(),
        base.actions.clone(),
        base.disclosures.clone(),
    )
    .unwrap();
    let show = common::available_mask_show(&face);
    let mut reader = SpokenFaceSession::new(face.clone(), show.clone()).unwrap();
    let property_index = reader
        .plan()
        .clauses
        .iter()
        .position(|clause| matches!(clause.provenance, FaceUtteranceProvenance::Property(_)))
        .unwrap();
    for _ in 0..property_index {
        reader
            .command(&face, &show, ReaderCommand::Next, 1)
            .unwrap();
        let packet = reader.next_segment().unwrap().unwrap();
        reader
            .acknowledge(SpokenDelivery::Completed(receipt(&packet)))
            .unwrap();
    }
    assert_eq!(reader.focused_index(), property_index);
    reader
        .command(&face, &show, ReaderCommand::Repeat, 2)
        .unwrap();
    let first = reader.next_segment().unwrap().unwrap();
    assert!(first.segment.text.len() <= MAXIMUM_SPEAKABLE_SEGMENT_BYTES);
    assert_eq!(
        reader.next_segment(),
        Err(SpokenFaceRefusal::SpeechPressure)
    );
    reader
        .acknowledge(SpokenDelivery::Completed(receipt(&first)))
        .unwrap();
    let second = reader.next_segment().unwrap().unwrap();
    assert_eq!(second.segment.sequence, first.segment.sequence + 1);
    let outcome = reader
        .acknowledge(SpokenDelivery::Completed(receipt(&second)))
        .unwrap()
        .unwrap();
    assert_eq!(outcome.completed_segments, 2);
    assert_eq!(outcome.outcome, SpokenTurnOutcome::Completed);
    assert!(reader.next_segment().unwrap().is_none());

    let mut changed = face.clone();
    changed.properties[0].value = PresentationPropertyValue::Text("New name".into());
    changed = Presentation::new_with_semantics(
        face.revision + 1,
        changed.basis,
        changed.subjects,
        changed.relationships,
        changed.properties,
        changed.text,
        changed.actions,
        changed.disclosures,
    )
    .unwrap();
    let changed_show = common::available_mask_show(&changed);
    reader
        .refresh(changed.clone(), changed_show.clone())
        .unwrap();
    assert!(matches!(
        reader.focused_clause().provenance,
        FaceUtteranceProvenance::Subject(_)
    ));
    let announced = reader.next_segment().unwrap().unwrap();
    assert!(announced.segment.text.contains("Welcome"));
    reader
        .acknowledge(SpokenDelivery::Completed(receipt(&announced)))
        .unwrap();
    assert_eq!(
        reader.command(&face, &show, ReaderCommand::Activate, 3),
        Err(SpokenFaceRefusal::StaleFace)
    );
}

#[test]
fn stop_never_turns_a_late_audio_completion_into_success() {
    let (face, show) = face_with_action();
    let mut reader = SpokenFaceSession::new(face.clone(), show.clone()).unwrap();
    reader
        .command(&face, &show, ReaderCommand::ReadAll, 1)
        .unwrap();
    let packet = reader.next_segment().unwrap().unwrap();
    reader
        .command(&face, &show, ReaderCommand::Stop, 2)
        .unwrap();
    let terminal = reader
        .acknowledge(SpokenDelivery::Completed(receipt(&packet)))
        .unwrap()
        .unwrap();
    assert_eq!(terminal.outcome, SpokenTurnOutcome::Cancelled);
    assert_eq!(terminal.completed_segments, 1);
}
