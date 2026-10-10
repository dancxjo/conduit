use super::*;
use conduit_core::{
    bind_active_play, ActivePlayId, CheckedPlotId, ExpandedPlotId, PlanId, SignId, SourceDocumentId,
};
use conduit_presentation::{
    FaceInteractionArgument, ManifestationLifecycle, PresentationBasis,
    PresentationContributionBasis, PresentationRole, PresentationSubject, UTF8_TEXT_VALUE_KIND,
};
use conduit_todo_face::todo_fragment;
use conduit_todo_plot::TodoCommand;
use std::sync::Mutex;

use crate::mask_test_common as common;

struct FakeQueue {
    maximum: usize,
    accepted: Mutex<Vec<Vec<u8>>>,
}

impl FakeQueue {
    fn new(maximum: usize) -> Self {
        Self {
            maximum,
            accepted: Mutex::new(Vec::new()),
        }
    }
}

impl TodoForeSubmission for FakeQueue {
    fn submit(&self, canonical: &[u8]) -> Result<BodyLiveForeAdmission, String> {
        TodoCommand::decode_info(canonical).map_err(|error| format!("{error:?}"))?;
        let mut accepted = self.accepted.lock().unwrap();
        if accepted.len() == self.maximum {
            return Ok(BodyLiveForeAdmission::Full);
        }
        let sequence = accepted.len() as u64;
        accepted.push(canonical.to_vec());
        Ok(BodyLiveForeAdmission::Accepted { sequence })
    }
}

fn face_and_show(state: &TodoState) -> (Presentation, MaskShow) {
    let basis = PresentationContributionBasis {
        checked_plot_id: CheckedPlotId::from("checked/todo"),
        plan_id: PlanId::from("plan/todo"),
        active_play_id: ActivePlayId::from("play/todo"),
        required_interaction_context: None,
    };
    let fragment = todo_fragment(state, basis.clone(), true).unwrap();
    let mut subjects = fragment.subjects;
    subjects.push(PresentationSubject {
        identity: "patchbay/plot".into(),
        role: PresentationRole::Plot,
        name: "Todo encounter".into(),
    });
    let face = Presentation::new_with_semantics(
        u64::from(state.revision),
        PresentationBasis {
            body_id: None,
            wake_id: None,
            source_document_id: Some(SourceDocumentId::from("source/todo")),
            checked_plot_id: Some(basis.checked_plot_id),
            expanded_plot_id: Some(ExpandedPlotId::from("expanded/todo")),
            plan_id: Some(basis.plan_id),
            active_play_id: Some(basis.active_play_id),
            sign_ids: vec![],
        },
        subjects,
        fragment.relationships,
        fragment.properties,
        fragment.text,
        fragment.actions,
        fragment.disclosures,
    )
    .unwrap();
    let show = common::available_mask_show(&face);
    (face, show)
}

fn another_show(face: &Presentation, first: &MaskShow) -> MaskShow {
    let placement = first.planned_mask.show_placement();
    let active = bind_active_play(
        &first.planned_mask.plan.plan_id,
        &placement.host_id,
        &placement.boot_id,
        2,
    );
    MaskShow::prepared(
        &first.planned_mask,
        face,
        active,
        "patchbay/plot".into(),
        "display/another".into(),
        SignId::from("sign/todo-other-prepared"),
    )
    .unwrap()
    .transition(
        ManifestationLifecycle::Available,
        SignId::from("sign/todo-other-available"),
    )
    .unwrap()
}

fn add(face: &Presentation, show: &MaskShow, sequence: u64) -> FaceInteraction {
    FaceInteraction::new(
        face,
        show,
        "todo.add",
        "todo/list",
        vec![FaceInteractionArgument {
            name: "text".into(),
            value_kind: UTF8_TEXT_VALUE_KIND.into(),
            value: b"Milk".to_vec(),
        }],
        sequence,
    )
    .unwrap()
}

#[test]
fn distinct_show_actions_stage_exact_commands_and_retain_separate_ids() {
    let state = TodoState::new("Groceries".into()).unwrap();
    let (face, first_show) = face_and_show(&state);
    let second_show = another_show(&face, &first_show);
    let first = add(&face, &first_show, 1);
    let second = add(&face, &second_show, 1);
    assert_ne!(first.identity, second.identity);
    let mut bridge = TodoActionBridge::new(FakeQueue::new(2), 2).unwrap();
    assert_eq!(
        bridge.submit(&state, &face, &first_show, first.clone()),
        Err(TodoActionBridgeRefusal::PlayNotBound)
    );
    bridge
        .bind_parent_play(ActivePlayId::from("play/todo"))
        .unwrap();
    let first_receipt = match bridge.submit(&state, &face, &first_show, first).unwrap() {
        TodoActionAdmission::Accepted(receipt) => receipt,
        TodoActionAdmission::Full => panic!("first slot admitted"),
    };
    let second_receipt = match bridge.submit(&state, &face, &second_show, second).unwrap() {
        TodoActionAdmission::Accepted(receipt) => receipt,
        TodoActionAdmission::Full => panic!("second slot admitted"),
    };
    assert_eq!(
        (first_receipt.queue_sequence, second_receipt.queue_sequence),
        (0, 1)
    );
    assert_ne!(first_receipt.interaction_id, second_receipt.interaction_id);
    assert_ne!(first_receipt.show_id, second_receipt.show_id);
    assert_eq!(state.revision, 0); // Queue staging did not mutate the Body.
    assert_eq!(bridge.evidence().len(), 2);
    assert_eq!(bridge.evidence()[0].action_id, "todo.add");
    assert_eq!(bridge.evidence()[1].action_id, "todo.add");
    assert_eq!(
        bridge
            .queue
            .accepted
            .lock()
            .unwrap()
            .iter()
            .map(|bytes| { TodoCommand::decode_info(bytes).unwrap() })
            .collect::<Vec<_>>(),
        vec![
            TodoCommand::Add {
                text: "Milk".into()
            },
            TodoCommand::Add {
                text: "Milk".into()
            },
        ]
    );
}

#[test]
fn malformed_stale_and_pressure_leave_queue_and_evidence_unchanged() {
    let state = TodoState::new("Groceries".into()).unwrap();
    let (face, show) = face_and_show(&state);
    let mut bridge = TodoActionBridge::new(FakeQueue::new(1), 1).unwrap();
    bridge
        .bind_parent_play(ActivePlayId::from("play/todo"))
        .unwrap();
    let mut wrong_kind = add(&face, &show, 1);
    wrong_kind.arguments[0].value_kind = "value/bool".into();
    assert!(matches!(
        bridge.submit(&state, &face, &show, wrong_kind),
        Err(TodoActionBridgeRefusal::Face(_))
    ));
    let mut unknown = add(&face, &show, 1);
    unknown.action_id = "todo.unknown".into();
    assert!(matches!(
        bridge.submit(&state, &face, &show, unknown),
        Err(TodoActionBridgeRefusal::Face(_))
    ));
    let mut stale = add(&face, &show, 1);
    stale.face_revision += 1;
    assert!(matches!(
        bridge.submit(&state, &face, &show, stale),
        Err(TodoActionBridgeRefusal::Face(_))
    ));
    let other_show = another_show(&face, &show);
    assert!(matches!(
        bridge.submit(&state, &face, &other_show, add(&face, &show, 1)),
        Err(TodoActionBridgeRefusal::Face(_))
    ));
    assert!(bridge.queue.accepted.lock().unwrap().is_empty());
    assert!(bridge.evidence().is_empty());

    let first = add(&face, &show, 1);
    let accepted = bridge.submit(&state, &face, &show, first.clone()).unwrap();
    assert!(matches!(accepted, TodoActionAdmission::Accepted(_)));
    assert_eq!(
        bridge.submit(&state, &face, &show, first),
        Err(TodoActionBridgeRefusal::Ledger(
            FaceInteractionRefusal::DuplicateDelivery
        ))
    );
    let second = add(&face, &show, 2);
    assert_eq!(
        bridge.submit(&state, &face, &show, second.clone()),
        Err(TodoActionBridgeRefusal::Ledger(
            FaceInteractionRefusal::EvidenceExhausted
        ))
    );
    let evidence = bridge.evidence()[0].interaction_id.clone();
    bridge
        .acknowledge_persisted_evidence_prefix(&[evidence])
        .unwrap();
    assert_eq!(
        bridge.submit(&state, &face, &show, second).unwrap(),
        TodoActionAdmission::Full
    );
    assert_eq!(bridge.queue.accepted.lock().unwrap().len(), 1);
    assert!(bridge.evidence().is_empty());
}

#[test]
fn face_from_another_parent_play_cannot_stage() {
    let state = TodoState::new("Groceries".into()).unwrap();
    let (face, show) = face_and_show(&state);
    let mut bridge = TodoActionBridge::new(FakeQueue::new(1), 1).unwrap();
    bridge
        .bind_parent_play(ActivePlayId::from("play/other"))
        .unwrap();
    assert_eq!(
        bridge.submit(&state, &face, &show, add(&face, &show, 1)),
        Err(TodoActionBridgeRefusal::StalePlay)
    );
    assert!(bridge.queue.accepted.lock().unwrap().is_empty());
    assert!(bridge.evidence().is_empty());
}
