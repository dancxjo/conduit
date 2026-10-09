//! A real waiting action and verified read for installed Mask provider tests.
use super::*;

pub(crate) fn published_fixture() -> (Owner, std::path::PathBuf) {
    let (mut owner, source, plot, grant, root, checkpoint) = fixture();
    let mut worker = owner
        .start_waiting_todo(
            &root,
            &source,
            &plot,
            &grant,
            super::super::super::todo_waiting::NewTodoCheckpoint {
                root: checkpoint.clone(),
                identity: CheckpointIdentity {
                    body: owner.session.evidence().body_id.as_str().into(),
                    plot: plot.expanded.checked_plot_id.as_str().into(),
                    workload: "todo-list".into(),
                    missing_v2: MissingV2Disposition::StartNewList,
                },
                current: TodoState::new("Groceries".into()).unwrap(),
            },
            5_000,
        )
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while owner.todo_live.is_none() {
        assert!(worker.progress(&mut owner, &root).unwrap().is_none());
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let face = owner.local_face_snapshot().unwrap();
    let show = mask_test_common::available_mask_show(&face);
    let action = FaceInteraction::new(
        &face,
        &show,
        "todo.add",
        "todo/list",
        vec![FaceInteractionArgument {
            name: "text".into(),
            value_kind: UTF8_TEXT_VALUE_KIND.into(),
            value: b"Buy milk".to_vec(),
        }],
        1,
    )
    .unwrap();
    worker.submit_interaction(&owner, &show, &action).unwrap();
    let committed = loop {
        if let Some(state) = worker.progress(&mut owner, &root).unwrap() {
            break state;
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(5));
    };
    let selected = crate::durable_host::selected_todo_checkpoint(&root)
        .unwrap()
        .unwrap();
    owner
        .read_committed_todo(&root, &checkpoint, &selected.content, &committed, 5_000)
        .unwrap();
    (owner, root)
}
