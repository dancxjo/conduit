use super::*;
use conduit_core::{BootId, HostId, OfferGeneration, ResourceVersionIdentity};
use conduit_presentation::{FaceInteraction, FaceInteractionArgument, UTF8_TEXT_VALUE_KIND};
use conduit_std_host::todo_durable_resource::MissingV2Disposition;
use conduit_std_host::{StdHost, StdHostConfig};

#[path = "../../../../semantics/presentation/tests/common/mod.rs"]
mod mask_test_common;

#[path = "todo_continuity_tests.rs"]
mod todo_continuity;

const SOURCE: &str = include_str!("../../../../plots/todo/checkpoint-once.conduit");

fn selected_host(root: &Path) -> StdHost {
    let selection =
        super::super::super::super::selected_todo::Selection::select(root, Some(&"02".repeat(32)))
            .unwrap();
    StdHost::new_for_todo_checkpoint_once(
        StdHostConfig {
            host_id: HostId::from("host/todo-owner-test"),
            boot_id: BootId::from("boot/todo-owner-test"),
            offer_generation: OfferGeneration(1),
        },
        root,
        selection.content(),
    )
    .unwrap()
}

fn fixture() -> (
    Owner,
    crate::plot_source::CanonicalSource,
    conduit_plot::ExpandedAuthoringPlot,
    AuthorityGrant,
    std::path::PathBuf,
    std::path::PathBuf,
) {
    let state_root = std::env::temp_dir().join(super::super::super::super::fresh_identity(
        "owner-todo-checkpoint",
        "first-action",
    ));
    let checkpoint_root = state_root.join("checkpoint");
    std::fs::create_dir_all(&checkpoint_root).unwrap();
    let installation = super::super::super::super::Installation {
        schema: super::super::super::super::INSTALL_SCHEMA.into(),
        host_id: "host/todo-owner-test".into(),
        release_source_identity: "source/test".into(),
        release_bundle_sha256: super::super::super::super::digest(b"bundle/test"),
        product_executable: "fixture-unused".into(),
        body_state: None,
        joined_body_state: None,
        selected_speech: None,
        selected_model: None,
        selected_todo_checkpoint: Some(
            super::super::super::super::selected_todo::Selection::select(
                &checkpoint_root,
                Some(&"02".repeat(32)),
            )
            .unwrap(),
        ),
    };
    super::super::super::super::write_json_atomic(
        &state_root.join("installation.json"),
        &installation,
    )
    .unwrap();
    let source = crate::plot_source::parse(SOURCE).unwrap();
    let plot = source.expand_entry_for_authoring().unwrap();
    assert_eq!(plot.expanded.name, "todo/checkpoint-once");
    let host = selected_host(&checkpoint_root);
    let ad = host.advertisement();
    let offer = ad
        .capabilities
        .iter()
        .find(|offer| {
            offer.implementation.implementation_id.as_str()
                == conduit_std_offers::TODO_CHECKPOINT_IMPLEMENTATION
        })
        .unwrap();
    let requirement = &offer.authority_requirements[0];
    let grant = AuthorityGrant {
        grant_id: "grant/owner/checkpoint".into(),
        contract_id: requirement.contract_id.clone(),
        host_call_contract_id: requirement.host_call_contract_id.clone(),
        subject_kind: requirement.subject_kind.clone(),
        host_id: ad.host_id.clone(),
        boot_id: ad.boot_id.clone(),
        capability_id: offer.capability_id.clone(),
    };
    let resident = ResidentPlot::new(
        plot.expanded.source_document_id.clone(),
        plot.expanded.checked_plot_id.clone(),
    );
    let mut owner = Owner::open(host, resident, None, "Groceries").unwrap();
    owner.set_resident_plot_name(&plot).unwrap();
    owner.persist(&state_root).unwrap();
    (owner, source, plot, grant, state_root, checkpoint_root)
}

fn failed_large_todo_read_fixture() -> (std::path::PathBuf, std::path::PathBuf, String) {
    let (mut owner, source, plot, grant, state_root, checkpoint_root) = fixture();
    let body_id = owner.session.evidence().body_id.as_str().to_owned();
    let mut worker = owner
        .start_waiting_todo(
            &state_root,
            &source,
            &plot,
            &grant,
            super::super::todo_waiting::NewTodoCheckpoint {
                root: checkpoint_root.clone(),
                identity: CheckpointIdentity {
                    body: body_id.clone(),
                    plot: plot.expanded.checked_plot_id.as_str().into(),
                    workload: "todo-list".into(),
                    missing_v2: MissingV2Disposition::StartNewList,
                },
                current: TodoState::new("L".repeat(64)).unwrap(),
            },
            5_000,
        )
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while owner.todo_live.is_none() {
        assert!(worker.progress(&mut owner, &state_root).unwrap().is_none());
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
            value: vec![b'I'; 23],
        }],
        1,
    )
    .unwrap();
    worker.submit_interaction(&owner, &show, &action).unwrap();
    let committed = loop {
        if let Some(committed) = worker.progress(&mut owner, &state_root).unwrap() {
            break committed;
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(5));
    };
    assert_eq!(committed.encode_info().unwrap().len(), 104);
    let selected = super::super::super::super::selected_todo_checkpoint(&state_root)
        .unwrap()
        .unwrap();
    let candidate = std::fs::read_dir(&checkpoint_root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.extension().is_some_and(|ext| ext == "checkpoint"))
        .unwrap();
    let original = std::fs::read(&candidate).unwrap();
    std::fs::write(&candidate, b"corrupt").unwrap();
    assert!(owner
        .read_committed_todo(
            &state_root,
            &checkpoint_root,
            &selected.content,
            &committed,
            5_000,
        )
        .is_err());
    let failed = owner.last_execution.as_ref().unwrap();
    assert_eq!(failed["verified"], false);
    assert!(failed["read_failure"].is_string());
    assert_eq!(failed["write"]["terminal"], "Completed");
    std::fs::write(candidate, original).unwrap();
    (state_root, checkpoint_root, body_id)
}

fn resumed_todo_host(state_root: &Path) -> StdHost {
    resumed_todo_host_on_boot(state_root, "boot/todo-owner-failed-read-retry")
}

fn resumed_todo_host_on_boot(state_root: &Path, boot: &str) -> StdHost {
    let selected = super::super::super::super::selected_todo_checkpoint(state_root)
        .unwrap()
        .unwrap();
    StdHost::new_for_todo_checkpoint_once(
        StdHostConfig {
            host_id: HostId::from("host/todo-owner-test"),
            boot_id: BootId::from(boot),
            offer_generation: OfferGeneration(1),
        },
        &selected.root,
        selected.content,
    )
    .unwrap()
}

#[test]
fn fresh_boot_retries_failed_large_todo_read_only_from_exact_published_write() {
    let (state_root, _checkpoint_root, body_id) = failed_large_todo_read_fixture();
    let old = super::super::super::state::execution(&state_root)
        .unwrap()
        .unwrap();
    let owner =
        super::super::super::resume_service(resumed_todo_host(&state_root), &state_root).unwrap();
    assert_eq!(owner.session.evidence().body_id.as_str(), body_id);
    assert!(owner.has_verified_todo());
    let new = owner.todo_verified_read_receipt().unwrap();
    assert_eq!(new["write"], old["write"]);
    assert_ne!(
        new["read_play"]["active_play_id"],
        old["read_play"]["active_play_id"]
    );
    assert!(owner
        .local_face_snapshot()
        .unwrap()
        .subjects
        .iter()
        .any(|subject| subject.name == "I".repeat(23)));
    std::fs::remove_dir_all(state_root).unwrap();
}

#[test]
fn failed_todo_read_retry_refuses_stale_selector_and_corrupt_write_receipt() {
    let (state_root, checkpoint_root, _) = failed_large_todo_read_fixture();
    let selector = std::fs::read_dir(&checkpoint_root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.extension().is_some_and(|ext| ext == "current"))
        .unwrap();
    let mut bytes = std::fs::read(&selector).unwrap();
    bytes[..32].fill(9);
    std::fs::write(selector, bytes).unwrap();
    assert!(
        super::super::super::resume_service(resumed_todo_host(&state_root), &state_root).is_err()
    );
    let retained = super::super::super::state::execution(&state_root)
        .unwrap()
        .unwrap();
    assert_eq!(retained["verified"], false);
    std::fs::remove_dir_all(state_root).unwrap();

    let (state_root, _, _) = failed_large_todo_read_fixture();
    let mut retained = super::super::super::state::execution(&state_root)
        .unwrap()
        .unwrap();
    retained["write"]["committed_fore_sha256"] = serde_json::json!("sha256:wrong");
    let biography = super::super::super::state::load(&state_root)
        .unwrap()
        .unwrap();
    let admissions =
        super::super::super::state::admissions(&state_root, &biography.body_id).unwrap();
    super::super::super::state::retain(
        &state_root,
        &biography,
        Some(&retained),
        admissions.as_ref(),
    )
    .unwrap();
    assert!(
        super::super::super::resume_service(resumed_todo_host(&state_root), &state_root).is_err()
    );
    assert_eq!(
        super::super::super::state::execution(&state_root)
            .unwrap()
            .unwrap()["verified"],
        false
    );
    std::fs::remove_dir_all(state_root).unwrap();
}

#[test]
fn todo_face_refuses_a_pre_play_contribution() {
    let (owner, _, _, _, state_root, _) = fixture();
    let initial = TodoState::new("Groceries".into()).unwrap();
    assert!(owner.project_face(Some((&initial, true))).is_err());
    std::fs::remove_dir_all(state_root).unwrap();
}

#[test]
fn waiting_owner_admits_exact_show_action_then_retains_commit_and_sign() {
    let (mut owner, source, plot, grant, state_root, checkpoint_root) = fixture();
    let body = owner.session.evidence().body_id.as_str().to_owned();
    let mut worker = owner
        .start_waiting_todo(
            &state_root,
            &source,
            &plot,
            &grant,
            super::super::todo_waiting::NewTodoCheckpoint {
                root: checkpoint_root.clone(),
                identity: CheckpointIdentity {
                    body: body.clone(),
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
        assert!(worker.progress(&mut owner, &state_root).unwrap().is_none());
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
    assert!(matches!(
        worker.submit_interaction(&owner, &show, &action).unwrap(),
        conduit_std_host::BodyLiveForeAdmission::Accepted { .. }
    ));
    assert!(worker.submit_interaction(&owner, &show, &action).is_err());
    let committed = loop {
        if let Some(committed) = worker.progress(&mut owner, &state_root).unwrap() {
            break committed;
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(5));
    };
    assert_eq!(committed.revision, 1);
    assert_eq!(committed.items[0].text, "Buy milk");
    let receipt = owner.todo_commit_receipt().unwrap().clone();
    assert_eq!(
        receipt["interaction_id"],
        serde_json::json!(action.identity)
    );
    assert!(receipt["terminal_sign"]["active_play_id"].is_string());
    assert!(receipt["committed_fore_sha256"].is_string());
    assert_eq!(receipt["checkpoint_namespace"]["list_key"], "todo-list");
    assert_eq!(
        receipt["checkpoint_namespace"]["body_id"],
        owner.session.evidence().body_id.as_str()
    );
    assert_eq!(
        receipt["checkpoint_namespace"]["write_plot_id"],
        plot.expanded.checked_plot_id.as_str()
    );
    assert_eq!(
        receipt["selected_content"]["access"],
        "WriteCandidatePublish"
    );
    assert_eq!(receipt["host_call_request"]["request"], 0);
    assert!(owner.todo_live.is_none());
    assert!(!owner
        .local_face_snapshot()
        .unwrap()
        .actions
        .iter()
        .any(|a| a.identity.as_str().starts_with("todo.")));
    let boot = owner.host.advertisement().boot_id.clone();
    let selected_write = owner
        .host
        .advertisement()
        .resources
        .iter()
        .find(|offer| offer.class_id.as_str() == "resource/todo-checkpoint@1")
        .unwrap()
        .content
        .as_ref()
        .unwrap()
        .contract
        .clone();
    let blocked_state_root = state_root.join("blocked-state-root");
    std::fs::write(&blocked_state_root, b"not a directory").unwrap();
    assert!(owner
        .read_committed_todo(
            &blocked_state_root,
            &checkpoint_root,
            &selected_write,
            &committed,
            5_000,
        )
        .is_err());
    assert_eq!(
        owner
            .host
            .advertisement()
            .resources
            .iter()
            .find(|resource| resource.class_id.as_str() == "resource/todo-checkpoint@1")
            .unwrap()
            .content
            .as_ref()
            .unwrap()
            .contract,
        selected_write
    );
    assert_eq!(
        owner.resident.as_ref().unwrap().checked_plot_id,
        plot.expanded.checked_plot_id
    );
    std::fs::remove_file(blocked_state_root).unwrap();
    let restored = owner
        .read_committed_todo(
            &state_root,
            &checkpoint_root,
            &selected_write,
            &committed,
            5_000,
        )
        .unwrap();
    assert_eq!(restored, committed);
    assert!(owner.has_verified_todo());
    assert_eq!(owner.host.advertisement().boot_id, boot);
    assert_eq!(
        owner.session.evidence().body_id.as_str(),
        receipt["body_id"]
    );
    assert_eq!(
        owner.last_execution.as_ref().unwrap()["schema"],
        "conduit.todo/verified-read-receipt@1"
    );
    let lulled_face = owner.local_face_snapshot().unwrap();
    assert_eq!(
        lulled_face.basis.body_id.as_ref(),
        Some(&owner.session.evidence().body_id)
    );
    assert!(lulled_face
        .subjects
        .iter()
        .any(|subject| subject.name == "Buy milk"));
    conduit_std_host::terminal_face_mask::TerminalFaceMask::prepare_read_only(
        lulled_face.clone(),
        80,
        24,
    )
    .unwrap();
    let spoken = conduit_std_host::spoken_face_mask::primary_face_clauses(&lulled_face)
        .unwrap()
        .join(" ");
    assert!(spoken.contains("Groceries") && spoken.contains("Buy milk"));
    assert!(!spoken.contains("sha256:") && !spoken.contains("Unavailable"));
    let selected_read = owner
        .host
        .advertisement()
        .resources
        .iter()
        .find(|resource| resource.class_id.as_str() == "resource/todo-checkpoint@1")
        .unwrap()
        .content
        .as_ref()
        .unwrap()
        .contract
        .clone();
    let first_read_receipt = owner.todo_verified_read_receipt().unwrap().clone();
    let next_selection =
        super::super::super::super::next_selected_todo_checkpoint(&state_root).unwrap();
    let next_write = next_selection.write_content();
    assert_ne!(next_write.version, selected_read.version);
    let stale_show = show;
    let mut next = owner
        .start_next_todo_action(&state_root, &next_selection, 5_000)
        .unwrap();
    assert_eq!(
        super::super::super::super::selected_todo_checkpoint(&state_root)
            .unwrap()
            .unwrap()
            .content
            .version,
        next_write.version
    );
    while owner.todo_live.is_none() {
        assert!(next.progress(&mut owner, &state_root).unwrap().is_none());
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let second_face = owner.local_face_snapshot().unwrap();
    let second_show = mask_test_common::available_mask_show(&second_face);
    let item_id = restored.items[0].id.clone();
    let second_action = FaceInteraction::new(
        &second_face,
        &second_show,
        &format!("todo.complete.{item_id}"),
        &format!("todo/item/{item_id}"),
        vec![],
        2,
    )
    .unwrap();
    assert!(next
        .submit_interaction(&owner, &stale_show, &second_action)
        .is_err());
    assert!(matches!(
        next.submit_interaction(&owner, &second_show, &second_action)
            .unwrap(),
        conduit_std_host::BodyLiveForeAdmission::Accepted { .. }
    ));
    let second = loop {
        if let Some(committed) = next.progress(&mut owner, &state_root).unwrap() {
            break committed;
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(5));
    };
    assert_eq!(second.revision, 2);
    assert!(second.items[0].complete);
    let second_receipt = owner.todo_commit_receipt().unwrap().clone();
    assert_eq!(
        second_receipt["schema"],
        "conduit.todo/next-checkpoint-receipt@1"
    );
    assert_eq!(
        second_receipt["selected_content"],
        serde_json::json!(next_write)
    );
    assert_eq!(
        second_receipt["previous_read"]["read_plan_id"],
        first_read_receipt["read_plan_id"]
    );
    assert_eq!(
        second_receipt["previous_read"]["read_terminal_sign_id"],
        first_read_receipt["read_terminal_sign"]["sign_id"]
    );
    let second_read = owner
        .read_committed_todo(&state_root, &checkpoint_root, &next_write, &second, 5_000)
        .unwrap();
    assert_eq!(second_read, second);
    let second_read_receipt = owner.todo_verified_read_receipt().unwrap();
    assert_eq!(second_read_receipt["verified"], true);
    assert_eq!(
        second_read_receipt["write"]["plan_id"],
        second_receipt["plan_id"]
    );
    assert_ne!(
        second_read_receipt["read_plan_id"],
        first_read_receipt["read_plan_id"]
    );
    assert_eq!(owner.session.evidence().body_id.as_str(), body);
    assert_eq!(owner.host.advertisement().boot_id, boot);
    assert!(owner
        .local_face_snapshot()
        .unwrap()
        .subjects
        .iter()
        .any(|subject| subject.name == "Buy milk"));
    let old_read_play = second_read_receipt["read_play"]["active_play_id"].clone();
    let old_read_sign = second_read_receipt["read_terminal_sign"]["sign_id"].clone();
    drop(owner);
    let selected = super::super::super::super::selected_todo_checkpoint(&state_root)
        .unwrap()
        .unwrap();
    let next_host = StdHost::new_for_todo_checkpoint_once(
        StdHostConfig {
            host_id: HostId::from("host/todo-owner-test"),
            boot_id: BootId::from("boot/todo-owner-reencounter"),
            offer_generation: OfferGeneration(1),
        },
        &selected.root,
        selected.content.clone(),
    )
    .unwrap();
    let mut owner = super::super::super::resume_service(next_host, &state_root).unwrap();
    assert_eq!(owner.session.evidence().body_id.as_str(), body);
    assert_ne!(owner.host.advertisement().boot_id, boot);
    let new_read = owner.todo_verified_read_receipt().unwrap();
    assert_ne!(new_read["read_play"]["active_play_id"], old_read_play);
    assert_ne!(new_read["read_terminal_sign"]["sign_id"], old_read_sign);
    assert_eq!(new_read["write"]["plan_id"], second_receipt["plan_id"]);
    assert!(owner
        .local_face_snapshot()
        .unwrap()
        .subjects
        .iter()
        .any(|subject| subject.name == "Buy milk"));
    let mut stale_write = next_write.clone();
    stale_write.version = ResourceVersionIdentity::from_digest([10; 32]);
    owner
        .host
        .transition_todo_checkpoint_offer(&checkpoint_root, stale_write)
        .unwrap();
    assert!(owner.local_face_snapshot().is_err());
    owner.last_execution.as_mut().unwrap()["read_terminal_sign"] = serde_json::Value::Null;
    super::super::super::state::retain(
        &state_root,
        owner.session.evidence(),
        owner.last_execution.as_ref(),
        owner.admissions.as_ref(),
    )
    .unwrap();
    drop(owner);
    let missing_sign_host = StdHost::new_for_todo_checkpoint_once(
        StdHostConfig {
            host_id: HostId::from("host/todo-owner-test"),
            boot_id: BootId::from("boot/todo-owner-missing-sign"),
            offer_generation: OfferGeneration(1),
        },
        &selected.root,
        selected.content.clone(),
    )
    .unwrap();
    assert!(super::super::super::resume_service(missing_sign_host, &state_root).is_err());
    std::fs::remove_dir_all(state_root).unwrap();
}

#[test]
fn corrupt_selected_read_lulls_without_claiming_verified_todo() {
    let (mut owner, source, plot, grant, state_root, checkpoint_root) = fixture();
    let identity = CheckpointIdentity {
        body: owner.session.evidence().body_id.as_str().to_owned(),
        plot: plot.expanded.checked_plot_id.as_str().to_owned(),
        workload: "todo-list".into(),
        missing_v2: MissingV2Disposition::StartNewList,
    };
    let mut worker = owner
        .start_waiting_todo(
            &state_root,
            &source,
            &plot,
            &grant,
            super::super::todo_waiting::NewTodoCheckpoint {
                root: checkpoint_root.clone(),
                identity,
                current: TodoState::new("Groceries".into()).unwrap(),
            },
            5_000,
        )
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while owner.todo_live.is_none() {
        assert!(worker.progress(&mut owner, &state_root).unwrap().is_none());
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let face = owner.local_face_snapshot().unwrap();
    let show = mask_test_common::available_mask_show(&face);
    let interaction = FaceInteraction::new(
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
    worker
        .submit_interaction(&owner, &show, &interaction)
        .unwrap();
    let committed = loop {
        if let Some(value) = worker.progress(&mut owner, &state_root).unwrap() {
            break value;
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(5));
    };
    let selected_write = owner
        .host
        .advertisement()
        .resources
        .iter()
        .find(|offer| offer.class_id.as_str() == "resource/todo-checkpoint@1")
        .unwrap()
        .content
        .as_ref()
        .unwrap()
        .contract
        .clone();
    let selector = std::fs::read_dir(&checkpoint_root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.extension().is_some_and(|ext| ext == "current"))
        .unwrap();
    std::fs::write(selector, b"corrupt").unwrap();
    assert!(owner
        .read_committed_todo(
            &state_root,
            &checkpoint_root,
            &selected_write,
            &committed,
            5_000,
        )
        .is_err());
    assert_eq!(
        owner.session.evidence().body.state,
        conduit_body::BodyState::Lulled
    );
    assert!(owner.todo_verified_read_receipt().is_none());
    assert_eq!(owner.last_execution.as_ref().unwrap()["verified"], false);
    std::fs::remove_dir_all(state_root).unwrap();
}

#[test]
fn first_caller_supplied_action_commits_under_retained_body_before_ack() {
    let (mut owner, source, plot, grant, state_root, checkpoint_root) = fixture();
    let body_id = owner.session.evidence().body_id.clone();
    owner.plan_checkpoint_once(&source, &plot, &grant).unwrap();
    let current = TodoState::new("Groceries".into()).unwrap();
    let committed = owner
        .execute_checkpoint_once(
            &state_root,
            &checkpoint_root,
            CheckpointIdentity {
                body: body_id.as_str().into(),
                plot: plot.expanded.checked_plot_id.as_str().into(),
                workload: "todo-list".into(),
                missing_v2: MissingV2Disposition::StartNewList,
            },
            &current,
            &TodoCommand::Add {
                text: "Buy milk".into(),
            },
            5000,
        )
        .unwrap();
    assert_eq!(committed.revision, 1);
    assert_eq!(committed.items[0].text, "Buy milk");
    assert_eq!(owner.session.evidence().body_id, body_id);
    assert!(owner.last_execution.as_ref().unwrap()["terminal_sign"]["active_play_id"].is_string());
    assert!(owner.last_execution.as_ref().unwrap()["committed_fore_sha256"].is_string());
    assert!(owner.session.realization().is_none());
    assert!(owner
        .execute_checkpoint_once(
            &state_root,
            &checkpoint_root,
            CheckpointIdentity {
                body: body_id.as_str().into(),
                plot: plot.expanded.checked_plot_id.as_str().into(),
                workload: "todo-list".into(),
                missing_v2: MissingV2Disposition::StartNewList,
            },
            &committed,
            &TodoCommand::Add {
                text: "Eggs".into()
            },
            5000,
        )
        .is_err());
    std::fs::remove_dir_all(state_root).unwrap();
}

#[test]
fn foreign_checkpoint_namespace_refuses_before_play_or_publication() {
    let (mut owner, source, plot, grant, state_root, checkpoint_root) = fixture();
    owner.plan_checkpoint_once(&source, &plot, &grant).unwrap();
    let current = TodoState::new("Groceries".into()).unwrap();
    let error = owner
        .execute_checkpoint_once(
            &state_root,
            &checkpoint_root,
            CheckpointIdentity {
                body: "different-body".into(),
                plot: plot.expanded.checked_plot_id.as_str().into(),
                workload: "todo-list".into(),
                missing_v2: MissingV2Disposition::StartNewList,
            },
            &current,
            &TodoCommand::Add {
                text: "Milk".into(),
            },
            5000,
        )
        .unwrap_err();
    assert!(error.contains("namespace"));
    assert!(owner.session.realization().unwrap().play.is_none());
    assert!(std::fs::read_dir(&checkpoint_root)
        .unwrap()
        .next()
        .is_none());
    std::fs::remove_dir_all(state_root).unwrap();
}

#[test]
fn stale_current_and_foreign_grant_refuse_before_play() {
    let (mut owner, source, plot, grant, state_root, checkpoint_root) = fixture();
    let mut foreign = grant.clone();
    foreign.boot_id = BootId::from("boot/foreign");
    assert!(owner
        .plan_checkpoint_once(&source, &plot, &foreign)
        .is_err());
    assert!(owner.session.realization().is_none());
    owner.plan_checkpoint_once(&source, &plot, &grant).unwrap();
    let mut stale = TodoState::new("Groceries".into()).unwrap();
    stale.revision = 1;
    let body = owner.session.evidence().body_id.as_str().to_string();
    assert!(owner
        .execute_checkpoint_once(
            &state_root,
            &checkpoint_root,
            CheckpointIdentity {
                body,
                plot: plot.expanded.checked_plot_id.as_str().into(),
                workload: "todo-list".into(),
                missing_v2: MissingV2Disposition::StartNewList,
            },
            &stale,
            &TodoCommand::Add {
                text: "Milk".into()
            },
            5000,
        )
        .is_err());
    assert!(owner.session.realization().unwrap().play.is_none());
    assert!(std::fs::read_dir(&checkpoint_root)
        .unwrap()
        .next()
        .is_none());
    std::fs::remove_dir_all(state_root).unwrap();
}
