//! An actual terminal provider consumes its Show before the next Todo Play.
use super::*;
use conduit_presentation::{FaceInteraction, PresentationPropertyValue};

#[test]
fn terminal_completes_verified_todo_and_rejects_prior_show() {
    let (owner, root) = crate::durable_host::owner::published_todo_test_fixture();
    let mut runtime = DurableHostRuntime::new("test".into(), "test".into(), StdHost::new());
    runtime.host = HostSource::Body {
        owner: Box::new(owner),
        root: root.clone(),
        running: None,
    };
    let marker_root = state(&runtime);
    fs::copy(marker_root.join("runtime.json"), root.join("runtime.json")).unwrap();
    fs::remove_dir_all(marker_root).unwrap();
    let (client, plan, show) = attach_once(&mut runtime, &root, &[7; 32]);
    let HostSource::Body { owner, .. } = &runtime.host else {
        unreachable!()
    };
    let before = owner.local_face_snapshot().unwrap();
    assert!(before.basis.active_play_id.is_none());
    let action = before
        .actions
        .iter()
        .find(|a| a.identity == "todo.complete.task-1")
        .unwrap();
    let interaction =
        FaceInteraction::new(&before, &show, &action.identity, &action.target, vec![], 1).unwrap();
    let result = runtime
        .attached_terminal_interaction(&plan, &show, interaction.clone())
        .unwrap();
    assert_eq!(result["schema"], "conduit.todo/committed-action@1");
    assert_eq!(result["state_revision"], 2);
    assert_eq!(
        result["receipt"]["initiating_show"],
        serde_json::json!(show)
    );
    assert_eq!(
        result["receipt"]["initiating_action"],
        serde_json::json!(interaction)
    );
    assert_eq!(
        result["receipt"]["interaction_id"],
        serde_json::json!(interaction.identity)
    );
    assert!(result["receipt"]["terminal_sign"]["sign_id"].is_string());
    assert_eq!(result["read_receipt"]["verified"], true);
    assert!(runtime.terminal_route.is_none());
    assert!(!is_attached(&mut runtime));
    drop(client);
    let (client, next_plan, next_show) = attach_once(&mut runtime, &root, &[7; 32]);
    let HostSource::Body { owner, .. } = &runtime.host else {
        unreachable!()
    };
    let after = owner.local_face_snapshot().unwrap();
    assert_eq!(after.basis.body_id, before.basis.body_id);
    assert!(after.basis.active_play_id.is_none());
    assert!(after
        .properties
        .iter()
        .any(|p| p.subject == "todo/item/task-1"
            && p.name == "complete"
            && p.value == PresentationPropertyValue::Flag(true)));
    assert_ne!(next_plan, plan);
    assert_ne!(next_show.show_id, show.show_id);
    assert!(runtime
        .attached_terminal_interaction(&plan, &show, interaction)
        .is_err());
    let HostSource::Body { owner, .. } = &runtime.host else {
        unreachable!()
    };
    assert_eq!(
        owner.local_face_snapshot().unwrap().properties,
        after.properties
    );
    drop(client);
    retire_closed_attachment(&root, &mut runtime).unwrap();
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn committed_todo_admits_next_action_without_terminal_provider() {
    let (owner, root) = crate::durable_host::owner::published_todo_test_fixture();
    let mut runtime = DurableHostRuntime::new("test".into(), "test".into(), StdHost::new());
    runtime.host = HostSource::Body {
        owner: Box::new(owner),
        root: root.clone(),
        running: None,
    };
    let marker_root = state(&runtime);
    fs::copy(marker_root.join("runtime.json"), root.join("runtime.json")).unwrap();
    fs::remove_dir_all(marker_root).unwrap();
    for sequence in 1..=2 {
        let HostSource::Body { owner, .. } = &runtime.host else {
            unreachable!()
        };
        let face = owner.local_face_snapshot().unwrap();
        let show = crate::mask_test_common::available_mask_show(&face);
        let interaction = FaceInteraction::new(
            &face,
            &show,
            "todo.add",
            "todo/list",
            vec![conduit_presentation::FaceInteractionArgument {
                name: "text".into(),
                value_kind: conduit_presentation::UTF8_TEXT_VALUE_KIND.into(),
                value: format!("Next {sequence}").into_bytes(),
            }],
            sequence,
        )
        .unwrap();
        let result = runtime
            .submit_owned_todo_action(&show, &interaction)
            .unwrap();
        assert_eq!(result["state_revision"], sequence + 1);
        assert_eq!(result["read_receipt"]["verified"], true);
    }
    fs::remove_dir_all(root).unwrap();
}
