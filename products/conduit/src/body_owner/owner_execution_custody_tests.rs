//! Real owner worker effects and retained terminal evidence under storage failure.
use super::super::super::todo_waiting;
use super::*;
use crate::durable_host::owner::state;
use std::{
    fs,
    time::{Duration, Instant},
};

fn start(
    owner: &mut Owner,
    source: &crate::plot_source::CanonicalSource,
    plot: &conduit_plot::ExpandedAuthoringPlot,
    grant: &AuthorityGrant,
    root: &Path,
    checkpoint: &Path,
) -> Result<todo_waiting::TodoWaitingWorker, String> {
    owner.start_waiting_todo(
        root,
        source,
        plot,
        grant,
        todo_waiting::NewTodoCheckpoint {
            root: checkpoint.into(),
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
}
#[test]
fn terminal_receipt_survives_publication_failure_and_retry_does_not_replay_effects() {
    let (mut owner, source, plot, grant, root, checkpoint) = fixture();
    let mut worker = start(&mut owner, &source, &plot, &grant, &root, &checkpoint).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while owner.todo_live.is_none() {
        assert!(worker.progress(&mut owner, &root).unwrap().is_none());
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    let started = owner.current_play_id().unwrap().clone();
    // Lose owner persistence only after the real started Play was acknowledged.
    let blocked = root.join("body/owner-transaction.json");
    fs::create_dir(&blocked).unwrap();
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
            value: b"One actual effect".to_vec(),
        }],
        1,
    )
    .unwrap();
    worker.submit_interaction(&owner, &show, &action).unwrap();
    loop {
        match worker.progress(&mut owner, &root) {
            Err(_) => break,
            Ok(None) => {}
            Ok(Some(_)) => panic!("publication fault was not exercised"),
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    let retained = owner
        .last_execution
        .clone()
        .expect("terminal evidence cannot be lost with a failed write");
    assert_eq!(retained["play"]["active_play_id"], started.as_str());
    assert_eq!(retained["committed_fore_count"], 1);
    assert!(retained["terminal_sign"]["sign_id"].is_string());
    assert!(owner.session.realization().is_none());
    assert!(owner.todo_live.is_none());
    assert_eq!(owner.truth()["execution_persistence_pending"], true);
    let files = checkpoint_files(&checkpoint);
    assert_eq!(files.len(), 2);
    assert!(owner.persist(&root).is_err());
    assert_eq!(owner.last_execution, Some(retained.clone()));
    fs::remove_dir(&blocked).unwrap();
    owner.persist(&root).unwrap();
    owner.persist(&root).unwrap();
    state::recover(&root).unwrap();
    assert_eq!(owner.truth()["execution_persistence_pending"], false);
    assert_eq!(state::execution(&root).unwrap(), Some(retained.clone()));
    assert_eq!(
        state::execution_history(&root).unwrap()["receipts"],
        serde_json::json!([retained])
    );
    assert_eq!(
        checkpoint_files(&checkpoint),
        files,
        "persistence retry must not rerun the checkpoint Host Call"
    );
    println!(
        "{}",
        serde_json::json!({"case":"owner-terminal-publication-retry-without-replay",
        "receipt":retained,"history":state::execution_history(&root).unwrap(),
        "checkpoint_files":files.iter().map(|(name,bytes)|serde_json::json!({"name":name.to_string_lossy(),"sha256":crate::durable_host::digest(bytes)})).collect::<Vec<_>>()})
    );
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn full_custody_refuses_normal_owner_start_before_checkpoint_effects() {
    let (mut owner, source, plot, grant, root, checkpoint) = fixture();
    // Fill the declared metadata profile, without pretending these are kernel runs.
    for ordinal in 0..64 {
        state::retain(
            &root,
            owner.session.evidence(),
            Some(&serde_json::json!({
                "body_id":owner.session.evidence().body_id,"profile_fixture_slot":ordinal
            })),
            owner.admissions.as_ref(),
        )
        .unwrap();
    }
    let history = state::execution_history(&root).unwrap();
    let error = start(&mut owner, &source, &plot, &grant, &root, &checkpoint)
        .err()
        .expect("full custody must refuse");
    assert!(error.contains("before Play"), "{error}");
    assert!(!owner.host.is_playing());
    assert!(owner.todo_live.is_none());
    assert!(checkpoint_files(&checkpoint).is_empty());
    assert_eq!(state::execution_history(&root).unwrap(), history);
    println!(
        "{}",
        serde_json::json!({"case":"normal-owner-custody-capacity-before-effects",
        "capacity":64,"fixture_receipts":true,"retained_count":history["receipts"].as_array().unwrap().len(),
        "refusal":error,"host_started":owner.host.is_playing(),"checkpoint_files":checkpoint_files(&checkpoint).len()})
    );
    fs::remove_dir_all(root).unwrap();
}
fn checkpoint_files(root: &Path) -> Vec<(std::ffi::OsString, Vec<u8>)> {
    let mut files = fs::read_dir(root)
        .unwrap()
        .map(|entry| {
            let p = entry.unwrap().path();
            (p.file_name().unwrap().to_owned(), fs::read(p).unwrap())
        })
        .collect::<Vec<_>>();
    files.sort();
    files
}
