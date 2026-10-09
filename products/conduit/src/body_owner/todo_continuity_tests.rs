//! Hosted continuity proof: real checkpoint reads, one Birth, two admitted Hosts.
use super::*;
use crate::durable_host as installed;
use conduit_body::{
    AdmissionManager, BodyBiographyRecordKind, BodyState, PortableSpawnAdmissionRequest,
    SpawnInvitationSecret, SPAWN_ADMISSION_REQUEST_SCHEMA,
};
use std::time::{Duration, Instant};

fn next_action(
    owner: &mut Owner,
    root: &Path,
    text: Option<&str>,
    sequence: u64,
    initial: Option<(
        &crate::plot_source::CanonicalSource,
        &conduit_plot::ExpandedAuthoringPlot,
        &AuthorityGrant,
    )>,
) -> TodoState {
    let selection = installed::next_selected_todo_checkpoint(root).unwrap();
    let selected_write = if initial.is_some() {
        installed::selected_todo_checkpoint(root)
            .unwrap()
            .unwrap()
            .content
    } else {
        selection.write_content()
    };
    let mut worker = if let Some((source, plot, grant)) = initial {
        owner
            .start_waiting_todo(
                root,
                source,
                plot,
                grant,
                super::super::super::todo_waiting::NewTodoCheckpoint {
                    root: selection.root.clone(),
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
            .unwrap()
    } else {
        owner
            .start_next_todo_action(root, &selection, 5_000)
            .unwrap()
    };
    let deadline = Instant::now() + Duration::from_secs(5);
    while owner.todo_live.is_none() {
        assert!(worker.progress(owner, root).unwrap().is_none());
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    // The existing typed ingress requires exact Show correlation. This is a
    // deterministic fixture Show, not evidence of a rendered Mask or gesture.
    let face = owner.local_face_snapshot().unwrap();
    let show = mask_test_common::available_mask_show(&face);
    let action = FaceInteraction::new(
        &face,
        &show,
        if text.is_some() {
            "todo.add"
        } else {
            "todo.complete.task-1"
        },
        if text.is_some() {
            "todo/list"
        } else {
            "todo/item/task-1"
        },
        text.map(|text| {
            vec![FaceInteractionArgument {
                name: "text".into(),
                value_kind: UTF8_TEXT_VALUE_KIND.into(),
                value: text.as_bytes().to_vec(),
            }]
        })
        .unwrap_or_default(),
        sequence,
    )
    .unwrap();
    worker.submit_interaction(owner, &show, &action).unwrap();
    let committed = loop {
        if let Some(value) = worker.progress(owner, root).unwrap() {
            break value;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    };
    owner
        .read_committed_todo(root, &selection.root, &selected_write, &committed, 5_000)
        .unwrap()
}

fn second_host(root: &Path, boot: &str) -> StdHost {
    let selected = installed::selected_todo_checkpoint(root).unwrap().unwrap();
    StdHost::new_for_todo_checkpoint_once(
        StdHostConfig {
            host_id: "host/todo-second".into(),
            boot_id: boot.into(),
            offer_generation: OfferGeneration(1),
        },
        &selected.root,
        selected.content,
    )
    .unwrap()
}

fn admit_second_host(owner: &mut Owner, root: &Path) {
    let host = second_host(root, "boot/todo-second/admission");
    let advertisement = host.advertisement().clone();
    let authority = owner.host.advertisement().clone();
    let secret = SpawnInvitationSecret::from_csprng_bytes([13; 32]).unwrap();
    let mut manager = AdmissionManager::new(owner.session.evidence().body_id.clone()).unwrap();
    let now = installed::current_time_millis().unwrap();
    let claim = owner
        .session
        .issue_invitation(
            &mut manager,
            secret.clone(),
            [17; 32],
            now,
            now + 60_000,
            &authority.host_id,
            &authority.boot_id,
        )
        .unwrap();
    let request = PortableSpawnAdmissionRequest {
        schema: SPAWN_ADMISSION_REQUEST_SCHEMA.into(),
        invitation_id: claim.invitation_id.clone(),
        body_id: claim.body_id.clone(),
        host_advertisement: advertisement.clone(),
        nonce: claim.nonce,
        signature: secret
            .sign(&claim.signing_transcript(
                &advertisement.host_id,
                &advertisement.boot_id,
                advertisement.offer_generation,
            ))
            .to_vec(),
        membership_admitted: false,
        plan_created: false,
        play_created: false,
    };
    owner.admissions = Some(manager);
    owner.persist(root).unwrap();
    owner
        .admit_invited(root, request, "host/todo-second")
        .unwrap();
}

#[test]
fn same_body_three_items_recover_on_another_admitted_host_without_a_cache() {
    let (mut owner, source, plot, grant, first_root, checkpoint_root) = fixture();
    let body_id = owner.session.evidence().body_id.clone();
    let first_host = owner.host.advertisement().host_id.clone();
    next_action(
        &mut owner,
        &first_root,
        Some("Milk"),
        1,
        Some((&source, &plot, &grant)),
    );
    next_action(&mut owner, &first_root, Some("Eggs"), 2, None);
    next_action(&mut owner, &first_root, Some("Bread"), 3, None);
    let expected = next_action(&mut owner, &first_root, None, 4, None);
    assert!(expected.items[0].complete);
    assert_eq!(expected.items.len(), 3);
    assert_eq!(
        expected
            .items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        ["task-1", "task-2", "task-3"]
    );
    assert_eq!(owner.session.evidence().body.state, BodyState::Lulled);
    assert!(owner.session.realization().is_none());
    let before = owner.todo_verified_read_receipt().unwrap().clone();
    let write_source = plot.expanded.source_document_id.clone();
    admit_second_host(&mut owner, &first_root);

    // Explicitly hand off retained continuity to a second installed Host.
    // No Todo bytes or display projection are included in this handoff.
    let second_root = first_root.join("second-installation");
    std::fs::create_dir(&second_root).unwrap();
    let mut installation =
        installed::read_installation(&first_root.join("installation.json")).unwrap();
    installation.host_id = "host/todo-second".into();
    installation.body_state = None;
    installed::write_json_atomic(&second_root.join("installation.json"), &installation).unwrap();
    let archives = first_root.join("body/archive");
    if archives.is_dir() {
        std::fs::create_dir_all(second_root.join("body/archive")).unwrap();
        for entry in std::fs::read_dir(archives).unwrap() {
            let entry = entry.unwrap();
            std::fs::copy(
                entry.path(),
                second_root.join("body/archive").join(entry.file_name()),
            )
            .unwrap();
        }
    }
    installed::owner::state::retain(
        &second_root,
        owner.session.evidence(),
        owner.last_execution.as_ref(),
        owner.admissions.as_ref(),
    )
    .unwrap();
    std::fs::copy(
        first_root.join("body/source.conduit"),
        second_root.join("body/source.conduit"),
    )
    .unwrap();
    owner.todo_verified = None;
    drop(owner);
    // The original owner's retained files cannot supply the new read either.
    std::fs::remove_dir_all(first_root.join("body")).unwrap();
    let owner = installed::owner::resume_service(
        second_host(&second_root, "boot/todo-second/rejoin"),
        &second_root,
    )
    .unwrap();
    let (basis, restored) = owner.todo_verified.as_ref().unwrap();
    assert_eq!(restored, &expected);
    assert_eq!(owner.session.evidence().body_id, body_id);
    assert_ne!(owner.host.advertisement().host_id, first_host);
    assert_eq!(owner.session.evidence().body.state, BodyState::Lulled);
    assert!(owner.session.realization().is_none());
    assert_eq!(
        owner
            .session
            .evidence()
            .records
            .iter()
            .filter(|record| matches!(record.kind, BodyBiographyRecordKind::Born { .. }))
            .count(),
        1
    );
    let after = owner.todo_verified_read_receipt().unwrap().clone();
    assert_eq!(after["write"], before["write"]);
    assert_eq!(
        after["restored_fore_sha256"],
        before["restored_fore_sha256"]
    );
    assert_ne!(
        after["read_play"]["active_play_id"],
        before["read_play"]["active_play_id"]
    );
    assert_ne!(
        after["read_terminal_sign"]["sign_id"],
        before["read_terminal_sign"]["sign_id"]
    );
    assert_eq!(
        basis.selection.selected_version,
        installed::selected_todo_checkpoint(&second_root)
            .unwrap()
            .unwrap()
            .content
            .version
    );

    // A surviving receipt is not a substitute for the selected resource.
    let candidate = std::fs::read_dir(&checkpoint_root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.file_name().unwrap().to_string_lossy().contains(
                &installation
                    .selected_todo_checkpoint
                    .as_ref()
                    .unwrap()
                    .version_hex(),
            ) && path.extension().is_some_and(|ext| ext == "checkpoint")
        })
        .unwrap();
    let bytes = std::fs::read(&candidate).unwrap();
    let checkpoint_sha256 = installed::digest(&bytes);
    drop(owner);
    std::fs::remove_file(&candidate).unwrap();
    assert!(installed::owner::resume_service(
        second_host(&second_root, "boot/todo-second/missing"),
        &second_root,
    )
    .is_err());
    let failed = installed::owner::state::execution(&second_root)
        .unwrap()
        .unwrap();
    assert_eq!(failed["verified"], false);
    assert!(failed["read_failure"].is_string());
    assert!(failed["restored_fore_sha256"].is_null());
    assert!(failed["read_terminal_sign"]["sign_id"].is_string());
    std::fs::write(&candidate, bytes).unwrap();
    let recovered = installed::owner::resume_service(
        second_host(&second_root, "boot/todo-second/recovered"),
        &second_root,
    )
    .unwrap();
    assert_eq!(&recovered.todo_verified.as_ref().unwrap().1, &expected);
    println!(
        "TODO_CONTINUITY_RECEIPT={}",
        serde_json::json!({
            "schema":"conduit.test/todo-continuity@1", "proof_class":"executable-hosted",
            "body_id":body_id, "write_source_document_id":write_source,
            "write_source_sha256":installed::digest(SOURCE.as_bytes()),
            "checkpoint_sha256":checkpoint_sha256,
            "before":before, "after":after, "failed_read":failed, "items":expected.items.iter().map(|item| serde_json::json!({"id":item.id.as_str(), "text":item.text.as_str(), "completed":item.complete})).collect::<Vec<_>>(),
            "writer_host":first_host, "reader_host":recovered.host.advertisement().host_id,
            "limits":"explicitly shared local residence; fixture handoff; no remote transport or rendered Mask proof",
        })
    );
    drop(recovered);
    std::fs::remove_dir_all(first_root).unwrap();
}

#[test]
fn interrupted_next_action_can_read_an_explicitly_reselected_published_version() {
    reselect_prior_published_version(false, false);
}

#[test]
fn cancelled_next_action_can_read_an_explicitly_reselected_published_version() {
    reselect_prior_published_version(true, false);
}

#[test]
fn cancelled_next_action_refuses_unverified_retained_witness() {
    reselect_prior_published_version(true, true);
}

fn reselect_prior_published_version(collect_failure: bool, corrupt_witness: bool) {
    let (mut owner, source, plot, grant, root, _checkpoint) = fixture();
    let expected = next_action(
        &mut owner,
        &root,
        Some("Milk"),
        1,
        Some((&source, &plot, &grant)),
    );
    let body = owner.session.evidence().body_id.clone();
    let read = owner.todo_verified_read_receipt().unwrap().clone();
    let published = installed::read_installation(&root.join("installation.json"))
        .unwrap()
        .selected_todo_checkpoint
        .unwrap();
    let next = installed::next_selected_todo_checkpoint(&root).unwrap();
    let mut worker = owner.start_next_todo_action(&root, &next, 5_000).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while owner.todo_live.is_none() {
        assert!(worker.progress(&mut owner, &root).unwrap().is_none());
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    worker.request_lull().unwrap();
    if collect_failure {
        loop {
            match worker.progress(&mut owner, &root) {
                Err(error) => {
                    assert_eq!(error, "Todo waiting Play did not commit one checkpoint");
                    break;
                }
                Ok(None) => {
                    assert!(Instant::now() < deadline);
                    std::thread::sleep(Duration::from_millis(5));
                }
                Ok(Some(_)) => panic!("cancelled command must not publish"),
            }
        }
    }
    drop(worker);
    if collect_failure {
        assert!(owner.todo_verified.is_none());
        assert!(owner.todo_verified_read_receipt().is_none());
        let failed = owner.last_execution.as_mut().unwrap();
        assert_eq!(failed["retained_verified_read"], read);
        assert!(failed["retained_verified_read"]["write"]["retained_verified_read"].is_null());
        if corrupt_witness {
            failed["retained_verified_read"]["verified"] = serde_json::json!(false);
            owner.persist(&root).unwrap();
        }
    }
    drop(owner);
    let unavailable = installed::owner::resume_service(
        resumed_todo_host_on_boot(&root, "boot/todo-interrupted/candidate"),
        &root,
    )
    .unwrap();
    assert!(
        unavailable.todo_verified.is_none(),
        "uncommitted candidate must not select an older list"
    );
    drop(unavailable);
    let mut installation = installed::read_installation(&root.join("installation.json")).unwrap();
    assert_ne!(
        installation
            .selected_todo_checkpoint
            .as_ref()
            .unwrap()
            .version_hex(),
        published.version_hex()
    );
    installation.selected_todo_checkpoint = Some(published);
    installed::write_json_atomic(&root.join("installation.json"), &installation).unwrap();
    let owner = installed::owner::resume_service(
        resumed_todo_host_on_boot(&root, "boot/todo-interrupted/published"),
        &root,
    )
    .unwrap();
    assert_eq!(owner.session.evidence().body_id, body);
    if corrupt_witness {
        assert!(
            owner.todo_verified.is_none(),
            "unverified witness must not recover Todo"
        );
        drop(owner);
        std::fs::remove_dir_all(root).unwrap();
        return;
    }
    assert_eq!(
        &owner
            .todo_verified
            .as_ref()
            .expect("fresh admitted read after explicit reselection")
            .1,
        &expected
    );
    let recovered = owner.todo_verified_read_receipt().unwrap();
    assert_eq!(recovered["write"], read["write"]);
    assert_ne!(
        recovered["read_play"]["active_play_id"],
        read["read_play"]["active_play_id"]
    );
    drop(owner);
    std::fs::remove_dir_all(root).unwrap();
}
