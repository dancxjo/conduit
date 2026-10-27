use super::*;
use conduit_std_host::{StdHost, StdHostConfig};

fn fixture() -> (Owner, std::path::PathBuf) {
    let root = std::env::temp_dir().join(super::super::super::super::fresh_identity(
        "notebook-owner",
        "edits",
    ));
    std::fs::create_dir_all(&root).unwrap();
    let installation = super::super::super::super::Installation {
        schema: super::super::super::super::INSTALL_SCHEMA.into(),
        host_id: "host/notebook-owner".into(),
        release_source_identity: "source/test".into(),
        release_bundle_sha256: super::super::super::super::digest(b"bundle/test"),
        product_executable: "fixture-unused".into(),
        body_state: None,
        joined_body_state: None,
        selected_speech: None,
        selected_model: None,
        selected_todo_checkpoint: None,
    };
    super::super::super::super::write_json_atomic(&root.join("installation.json"), &installation)
        .unwrap();
    let source = crate::plot_source::parse(SOURCE).unwrap();
    let plot = source.expand_entry_for_authoring().unwrap();
    let resident = ResidentPlot::new(
        plot.expanded.source_document_id.clone(),
        plot.expanded.checked_plot_id.clone(),
    );
    let host = StdHost::new_with_config(StdHostConfig {
        host_id: "host/notebook-owner".into(),
        boot_id: "boot/notebook-owner".into(),
        offer_generation: conduit_core::OfferGeneration(1),
    });
    let mut owner = Owner::open(host, resident, None, "Thoughts").unwrap();
    owner.set_resident_plot_name(&plot).unwrap();
    owner.initialize_notebook("Thoughts").unwrap();
    owner.persist(&root).unwrap();
    (owner, root)
}

#[test]
fn notebook_owner_adopts_source_outputs_and_keeps_the_document_after_stale_edit() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            let (mut owner, root) = fixture();
            let body = owner.session.evidence().body_id.clone();
            owner
                .edit_notebook(
                    &root,
                    TextEdit {
                        base_revision: 0,
                        start: 0,
                        end: 0,
                        insert_text: b"I was here.",
                    },
                )
                .unwrap();
            assert_eq!(
                owner.notebook_document().unwrap(),
                (
                    "Thoughts",
                    TextDocument {
                        revision: 1,
                        text: b"I was here."
                    }
                )
            );
            assert!(owner
                .edit_notebook(
                    &root,
                    TextEdit {
                        base_revision: 0,
                        start: 0,
                        end: 0,
                        insert_text: b"stale",
                    }
                )
                .is_err());
            assert_eq!(
                owner.notebook_document().unwrap(),
                (
                    "Thoughts",
                    TextDocument {
                        revision: 1,
                        text: b"I was here."
                    }
                )
            );
            assert_eq!(
                owner.last_execution.as_ref().unwrap()["document_adopted"],
                false
            );
            owner
                .edit_notebook(
                    &root,
                    TextEdit {
                        base_revision: 1,
                        start: 11,
                        end: 11,
                        insert_text: b" Again.",
                    },
                )
                .unwrap();
            assert_eq!(
                owner.notebook_document().unwrap(),
                (
                    "Thoughts",
                    TextDocument {
                        revision: 2,
                        text: b"I was here. Again."
                    }
                )
            );
            assert_eq!(owner.session.evidence().body_id, body);
            assert!(owner.session.realization().is_none());
            assert!(owner.initialize_notebook("Another").is_err());
            #[cfg(target_os = "linux")]
            {
                let storage = root.join("notebook-storage");
                std::fs::create_dir(&storage).unwrap();
                owner
                    .save_notebook(&root, std::fs::File::open(&storage).unwrap())
                    .unwrap();
                owner
                    .edit_notebook(
                        &root,
                        TextEdit {
                            base_revision: 2,
                            start: 18,
                            end: 18,
                            insert_text: b" Unsaved.",
                        },
                    )
                    .unwrap();
                assert_eq!(owner.notebook_document().unwrap().1.revision, 3);
                let other = root.join("other-storage");
                std::fs::create_dir(&other).unwrap();
                assert!(owner
                    .save_notebook(&root, std::fs::File::open(&other).unwrap())
                    .is_err());
                assert_eq!(owner.notebook_document().unwrap().1.revision, 3);
                assert_eq!(
                    owner.notebook_document().unwrap().1.text,
                    b"I was here. Again. Unsaved."
                );
                owner
                    .reopen_notebook(&root, std::fs::File::open(&storage).unwrap())
                    .unwrap();
                assert_eq!(
                    owner.notebook_document().unwrap().1,
                    TextDocument {
                        revision: 2,
                        text: b"I was here. Again.",
                    }
                );
                assert_eq!(owner.session.evidence().body_id, body);
                owner
                    .edit_notebook(
                        &root,
                        TextEdit {
                            base_revision: 2,
                            start: 18,
                            end: 18,
                            insert_text: b" Unsaved.",
                        },
                    )
                    .unwrap();
                // An unlinked directory descriptor remains a genuine selected
                // root, but Linux refuses new publication inside it for all UIDs.
                let unavailable = std::fs::File::open(&storage).unwrap();
                let saved_file = std::fs::read_dir(&storage)
                    .unwrap()
                    .next()
                    .unwrap()
                    .unwrap()
                    .path();
                std::fs::rename(saved_file, root.join("retained-file-record")).unwrap();
                std::fs::remove_dir(&storage).unwrap();
                assert!(owner.save_notebook(&root, unavailable).is_err());
                assert_eq!(
                    owner.notebook_document().unwrap().1,
                    TextDocument {
                        revision: 3,
                        text: b"I was here. Again. Unsaved.",
                    }
                );
                assert_eq!(
                    owner.last_execution.as_ref().unwrap()["file_operation_succeeded"],
                    false
                );
            }
            std::fs::remove_dir_all(root).unwrap();
        })
        .unwrap()
        .join()
        .unwrap();
}

#[cfg(target_os = "linux")]
#[test]
fn notebook_waiting_face_refuses_an_old_show_before_admitting_another_edit() {
    use conduit_face::{FaceInteraction, FaceInteractionArgument, UTF8_TEXT_VALUE_KIND};
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            let (mut owner, root) = fixture();
            let storage = root.join("storage");
            std::fs::create_dir(&storage).unwrap();
            owner
                .save_notebook(&root, std::fs::File::open(&storage).unwrap())
                .unwrap();
            let body = owner.notebook_body_id().clone();
            let idle_face = owner.local_face_snapshot().unwrap();
            assert!(crate::durable_host_control::has_native_return_route_intent(&idle_face));
            assert!(!crate::durable_host_control::has_remote_mask_action(&idle_face));
            assert!(matches!(idle_face.resolve_action(idle_face.revision, "notebook.replace"),
                Err(conduit_face::FaceActionRefusal::Unavailable { reason_code })
                    if reason_code == "notebook-edit-encounter-idle"));
            assert!(!idle_face.subjects.iter().any(|subject| subject.identity == "notebook/document"));
            let mut worker = owner.start_notebook_waiting(&root, 30_000).unwrap();
            let deadline = Instant::now() + Duration::from_secs(30);
            while owner.current_play_id().is_none() {
                assert_eq!(worker.progress(&mut owner, &root).unwrap(), None);
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(2));
            }
            let face = owner.local_face_snapshot().unwrap();
            let show = crate::mask_test_common::available_mask_show(&face);
            let action = FaceInteraction::new(
                &face,
                &show,
                "notebook.replace",
                "notebook/document",
                vec![FaceInteractionArgument {
                    name: "text".into(),
                    value_kind: UTF8_TEXT_VALUE_KIND.into(),
                    value: b"I was here.".to_vec(),
                }],
                1,
            )
            .unwrap();
            worker.submit_interaction(&owner, &show, &action).unwrap();
            loop {
                if let Some(adopted) = worker.progress(&mut owner, &root).unwrap() {
                    assert!(adopted);
                    break;
                }
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(2));
            }
            assert_eq!(
                owner.notebook_document().unwrap().1,
                TextDocument {
                    revision: 1,
                    text: b"I was here."
                }
            );
            let mut next = owner.start_notebook_waiting(&root, 30_000).unwrap();
            while owner.current_play_id().is_none() {
                assert_eq!(next.progress(&mut owner, &root).unwrap(), None);
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(2));
            }
            assert!(next.submit_interaction(&owner, &show, &action).is_err());
            assert_eq!(
                owner.notebook_document().unwrap().1,
                TextDocument {
                    revision: 1,
                    text: b"I was here."
                }
            );
            let current_face = owner.local_face_snapshot().unwrap();
            assert_ne!(face.identity, current_face.identity);
            // Masks receive the same exact value contract as the edit action,
            // so they can present the current text without inferring it from prose.
            let edit = current_face.actions.iter().find(|action| action.identity == "notebook.replace").unwrap();
            assert!(current_face.properties.iter().any(|property|
                property.subject == "notebook/document" && property.name == "current-value"
                    && matches!(&property.value, conduit_face::FacePropertyValue::TypedValue { contract, bytes }
                        if contract == &edit.arguments[0].contract && bytes == b"I was here.")));
            assert!(current_face.properties.iter().any(|property|
                property.subject == "notebook/document" && property.name == "value-change-action"
                    && property.value == conduit_face::FacePropertyValue::Identity(edit.identity.clone())));
            assert!(current_face
                .text
                .iter()
                .any(|item| item.text == "Unsaved changes"));
            let save_show = crate::mask_test_common::available_mask_show(&current_face);
            let save = FaceInteraction::new(
                &current_face,
                &save_show,
                "notebook.save",
                "notebook/document",
                vec![],
                2,
            )
            .unwrap();
            next.authorize_save(&owner, &save_show, &save).unwrap();
            loop {
                if let Some(adopted) = next.progress(&mut owner, &root).unwrap() {
                    assert!(!adopted);
                    break;
                }
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(2));
            }
            owner.save_selected_notebook(&root).unwrap();
            assert_eq!(
                owner.last_execution.as_ref().unwrap()["publication_interaction"]["identity"],
                save.identity.as_str()
            );
            let mut after_save = owner.start_notebook_waiting(&root, 30_000).unwrap();
            while owner.current_play_id().is_none() {
                assert_eq!(after_save.progress(&mut owner, &root).unwrap(), None);
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(2));
            }
            let saved_face = owner.local_face_snapshot().unwrap();
            assert!(saved_face.text.iter().any(|item| item.text == "Saved"));
            assert!(after_save
                .authorize_save(&owner, &save_show, &save)
                .is_err());
            after_save.request_lull().unwrap();
            loop {
                if let Some(adopted) = after_save.progress(&mut owner, &root).unwrap() {
                    assert!(!adopted);
                    break;
                }
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(2));
            }
            assert_eq!(owner.notebook_body_id(), &body);
            assert_eq!(owner.notebook_document().unwrap().1.revision, 1);
            assert!(!owner.host.is_playing());
            std::fs::remove_dir_all(root).unwrap();
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn notebook_idle_expiry_retires_the_play_and_preserves_the_body_document() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            let (mut owner, root) = fixture();
            let body = owner.notebook_body_id().clone();
            let mut worker = owner.start_notebook_waiting(&root, 20).unwrap();
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                if let Some(adopted) = worker.progress(&mut owner, &root).unwrap() {
                    assert!(!adopted);
                    break;
                }
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(2));
            }
            assert_eq!(owner.notebook_body_id(), &body);
            assert_eq!(
                owner.notebook_document().unwrap().1,
                TextDocument {
                    revision: 0,
                    text: b""
                }
            );
            assert!(!owner.host.is_playing());
            assert!(owner.session.realization().is_none());
            std::fs::remove_dir_all(root).unwrap();
        })
        .unwrap()
        .join()
        .unwrap();
}
