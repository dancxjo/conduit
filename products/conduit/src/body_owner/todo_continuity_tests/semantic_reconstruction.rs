//! Real committed-state Plays feed native and nonvisual reconstruction; no state is
//! manufactured by a Mask and no synthetic receipt substitutes for the read.
use super::*;
use conduit_presentation::{plan_face_utterances, render_linear_presentation};

#[test]
fn verified_three_item_todo_and_completion_preserve_nonvisual_meaning() {
    let (mut owner, source, plot, grant, root, _) = fixture();
    let body_id = owner.session.evidence().body_id.clone();
    let mut state = next_action(
        &mut owner,
        &root,
        Some("Milk"),
        1,
        Some((&source, &plot, &grant)),
    );
    assert_eq!(state.items.len(), 1);
    state = next_action(&mut owner, &root, Some("Eggs"), 2, None);
    assert_eq!(state.items.len(), 2);
    state = next_action(&mut owner, &root, Some("Bread"), 3, None);
    let mut exchanges = Vec::new();
    for expected_remaining in [3, 2] {
        assert_eq!(state.items.len(), 3);
        assert_eq!(
            state.items.iter().filter(|item| !item.complete).count(),
            expected_remaining
        );
        assert_eq!(owner.session.evidence().body_id, body_id);
        assert_eq!(owner.session.evidence().body.state, BodyState::Lulled);
        assert!(owner.session.realization().is_none());
        let (basis, restored) = owner.todo_verified.as_ref().unwrap();
        assert_eq!(restored, &state);
        let face = owner.project_verified_todo_face(basis, restored).unwrap();
        assert_eq!(face.basis.body_id.as_ref(), Some(&body_id));
        assert!(face.basis.active_play_id.is_none());
        assert!(face.basis.sign_ids.contains(&basis.read.terminal_sign_id));
        assert!(face.basis.sign_ids.contains(&basis.write.terminal_sign_id));
        let mut native =
            conduitos::native_face_scene::NativeFaceScene::prepare(face.clone(), 640, 480).unwrap();
        assert_eq!(native.presentation(), &face);
        let mut rendered = String::new();
        for _ in 0..native.pages() {
            let frame = native.frame().unwrap();
            for command in frame.scene.commands() {
                assert_eq!(
                    command.clip_class(),
                    conduit_presentation::GraphicsClipClass::FullyVisible
                );
                rendered.push_str(command.payload());
            }
            native.turn_page(true);
        }
        for item in state.items.iter().filter(|item| !item.complete) {
            assert!(
                rendered.contains(&item.text),
                "native primary lost pending item {}",
                item.id
            );
        }
        assert!(rendered.contains(&format!("{expected_remaining} things left")));
        native.show_details(true);
        let mut details = String::new();
        for _ in 0..native.pages() {
            let frame = native.frame().unwrap();
            for command in frame.scene.commands() {
                assert_eq!(
                    command.clip_class(),
                    conduit_presentation::GraphicsClipClass::FullyVisible
                );
                details.push_str(command.payload());
            }
            native.turn_page(true);
        }
        for item in &state.items {
            assert!(
                details.contains(&item.text),
                "native details lost committed item {}",
                item.id
            );
        }
        assert_eq!(native.presentation(), &face);
        exchanges.push(serde_json::json!({
            "face": face, "remaining": expected_remaining,
            "verified_read_receipt": owner.todo_verified_read_receipt().unwrap(),
        }));
        let linear = render_linear_presentation(&face).unwrap().lines.join("\n");
        let spoken = plan_face_utterances(&face)
            .unwrap()
            .clauses
            .into_iter()
            .map(|clause| clause.text)
            .collect::<Vec<_>>()
            .join("\n");
        for item in &state.items {
            assert!(linear.contains(&item.text), "linear lost {}", item.id);
            assert!(spoken.contains(&item.text), "aural lost {}", item.id);
        }
        let status = format!("{expected_remaining} things left");
        assert!(linear.contains(&status));
        assert!(spoken.contains(&status));
        if expected_remaining == 3 {
            let stale_basis = basis.clone();
            let stale_state = state.clone();
            state = next_action(&mut owner, &root, None, 4, None);
            let current_receipt = owner.todo_verified_read_receipt().unwrap().clone();
            assert_eq!(
                owner
                    .project_verified_todo_face(&stale_basis, &stale_state)
                    .unwrap_err(),
                conduit_presentation::CommittedStateEvidenceRefusal::ReadDigestMismatch.as_str()
            );
            assert_eq!(
                owner.todo_verified_read_receipt().unwrap(),
                &current_receipt
            );
        }
    }
    if let Some(path) = std::env::var_os("CONDUIT_CROSS_MASK_TODO_EXCHANGE") {
        let path = std::path::PathBuf::from(path);
        assert!(path.is_absolute());
        std::fs::write(
            path,
            serde_json::to_vec_pretty(&serde_json::json!({
                "schema": "conduit.proof/committed-todo-mask-exchange@1",
                "proof_class": "real checkpoint execution with fixture owner; no live DOM",
                "exchanges": exchanges,
            }))
            .unwrap(),
        )
        .unwrap();
    }
    drop(owner);
    std::fs::remove_dir_all(root).unwrap();
}
