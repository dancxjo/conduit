use super::*;

#[path = "../../../../semantics/presentation/tests/common/mod.rs"]
mod common;

use conduit_core::kind_id;
use conduit_presentation::{
    FaceActionArgument, PresentationAction, PresentationDisclosureLevel, PresentationProperty,
    PresentationPropertyValue, PresentationRelationship, PresentationRelationshipKind,
    PresentationRole, PresentationSubject, PresentationText,
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
