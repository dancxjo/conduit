use super::*;
use conduit_face::{Face, FaceInteraction, FaceOperatorActionKind};

fn navigation(owner: &Owner, intent: &str) -> (conduit_face::MaskShow, FaceInteraction) {
    let face = owner.local_face_snapshot().unwrap();
    let offered = face
        .actions
        .iter()
        .find(|action| action.intent == intent)
        .unwrap();
    let show = crate::mask_test_common::available_mask_show(&face);
    let action =
        FaceInteraction::new(&face, &show, &offered.identity, &offered.target, vec![], 1).unwrap();
    (show, action)
}
fn assert_context(face: &Face, context: &str) {
    assert!(face
        .properties
        .iter()
        .any(|property| property.name == "operator-context"
            && property.value == conduit_face::FacePropertyValue::Text(context.into())));
}
#[test]
fn notebook_library_opens_the_exact_resident_without_replacing_document_or_play() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            let (mut owner, root) = fixture();
            let storage = root.join("storage");
            std::fs::create_dir(&storage).unwrap();
            owner
                .save_notebook(&root, std::fs::File::open(storage).unwrap())
                .unwrap();
            let selected = std::fs::read(root.join("body/notebook-commit.json")).unwrap();
            let mut worker = owner.start_notebook_waiting(&root, 30_000).unwrap();
            let deadline = Instant::now() + Duration::from_secs(10);
            while owner.current_play_id().is_none() {
                assert_eq!(worker.progress(&mut owner, &root).unwrap(), None);
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(2));
            }
            let truth = serde_json::to_value(owner.session.realization()).unwrap();
            let body = owner.notebook_body_id().clone();
            let resident = owner.resident.as_ref().unwrap().checked_plot_id.clone();
            let initial = owner.local_face_snapshot().unwrap();
            let (old_show, old_action) = navigation(&owner, "conduit.intent/open-plot-library@1");
            let stale_edit = FaceInteraction::new(
                &initial,
                &old_show,
                "notebook.replace",
                "notebook/document",
                vec![conduit_face::FaceInteractionArgument {
                    name: "text".into(),
                    value_kind: "value/text".into(),
                    value: b"obsolete".to_vec(),
                }],
                2,
            )
            .unwrap();
            assert_eq!(
                owner
                    .resolve_clock_interaction(&old_show, &old_action)
                    .unwrap(),
                super::super::super::ClockAction::NotebookNavigation
            );
            owner.navigate_notebook(&old_show, &old_action).unwrap();
            let library = owner.local_face_snapshot().unwrap();
            assert_context(&library, "library");
            assert!(library.revision > initial.revision);
            assert!(library.subjects.iter().any(|subject| subject.identity
                == format!("plot/{}", resident.as_str())
                && subject.name == "notebook/transition"));
            assert!(library
                .disclosures
                .iter()
                .any(
                    |disclosure| disclosure.subject == format!("plot/{}", resident.as_str())
                        && disclosure.level == conduit_face::FaceDisclosureLevel::Primary
                ));
            assert!(owner.navigate_notebook(&old_show, &old_action).is_err());
            assert!(worker
                .submit_interaction(&owner, &old_show, &stale_edit)
                .is_err());
            assert_eq!(owner.local_face_snapshot().unwrap(), library);
            let (show, action) = navigation(&owner, "conduit.intent/open-resident-plot@1");
            let projected = owner.project_notebook_body_face().unwrap();
            assert_eq!(
                projected
                    .resolve_operator_action(action.face_revision, &action.action_id)
                    .unwrap()
                    .kind,
                FaceOperatorActionKind::OpenResidentPlot(resident)
            );
            owner.navigate_notebook(&show, &action).unwrap();
            assert_context(&owner.local_face_snapshot().unwrap(), "resident-plot");
            let (show, action) = navigation(&owner, "conduit.intent/open-body-overview@1");
            owner.navigate_notebook(&show, &action).unwrap();
            assert_context(&owner.local_face_snapshot().unwrap(), "overview");
            assert_eq!(owner.notebook_body_id(), &body);
            assert_eq!(
                owner.notebook_document().unwrap().1,
                TextDocument {
                    revision: 0,
                    text: b""
                }
            );
            assert_eq!(
                serde_json::to_value(owner.session.realization()).unwrap(),
                truth
            );
            assert_eq!(
                std::fs::read(root.join("body/notebook-commit.json")).unwrap(),
                selected
            );
            worker.request_lull().unwrap();
            while worker.progress(&mut owner, &root).unwrap().is_none() {
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(2));
            }
            assert!(owner.local_face_snapshot().unwrap().revision > initial.revision);
            std::fs::remove_dir_all(root).unwrap();
        })
        .unwrap()
        .join()
        .unwrap();
}
