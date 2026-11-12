//! CPU owner authority fixture with actual checked IMAGE, file effects and
//! recoverable owner journal. This does not qualify MCU input or retirement.
use super::*;
use std::fs;
#[test]
fn issuing_file_owner_retains_local_signs_without_advancing_global_body() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            use crate::durable_host::{text_file_policy, Installation, INSTALL_SCHEMA};
            let (checked, image, config, selected) = text_file_policy::text_file_fixture();
            fs::create_dir(&selected.directory).unwrap();
            let root = selected.directory.join("owner-state");
            fs::create_dir(&root).unwrap();
            let installation = Installation {
                schema: INSTALL_SCHEMA.into(),
                host_id: config.host_id.as_str().into(),
                release_source_identity: "source/test".into(),
                release_bundle_sha256: crate::durable_host::digest(b"bundle/test"),
                product_executable: "fixture-unused".into(),
                body_state: None,
                joined_body_state: None,
                selected_speech: None,
                selected_model: None,
                selected_host_configuration: None,
                selected_todo_checkpoint: None,
            };
            crate::durable_host::write_json_atomic(&root.join("installation.json"), &installation)
                .unwrap();
            let initialization =
                text_file_policy::prepare(&selected, &config, &image, &checked).unwrap();
            let host = conduit_std_host::StdHost::from_selected_timer_and_text_file_image(
                config,
                &image,
                &conduit_workspace_make::catalog(),
                &checked,
                initialization,
            )
            .unwrap();
            let source = include_str!("../../../../plots/notebook/publish.conduit")
                .replace("notebook/publish", "recording/publish");
            let parsed = crate::plot_source::parse(&source).unwrap();
            let plot = parsed.expand_entry_for_authoring().unwrap();
            let resident = ResidentPlot::new(
                plot.expanded.source_document_id.clone(),
                plot.expanded.checked_plot_id.clone(),
            );
            let mut owner = Owner::open(host, resident, None, "Recording").unwrap();
            state::retain_session(
                &root,
                &mut owner.session,
                None,
                None,
                Some(source.as_bytes()),
                None,
            )
            .unwrap();
            owner.plan_selected_text_file(&source, true).unwrap();
            let proposed = owner.session.realization().unwrap();
            let issued = owner
                .host
                .prospective_body_play(&proposed.wake, &proposed.plan)
                .unwrap();
            let before = owner.session.evidence().clone();
            let input = owner
                .host
                .retain_selected_image_fore_input(ExternalForeInput {
                    front_port_id: port_id("value"),
                    track: ConnectionTrack::Payload,
                    bytes: b"I was here.".to_vec(),
                })
                .unwrap();
            let report = owner
                .run_selected_issuing_text_file_plan(&root, &issued, true, &input)
                .unwrap();
            assert_eq!(
                report.local_execution().local_terminal(),
                &TerminalDisposition::Completed
            );
            assert_eq!(owner.session.evidence(), &before);
            assert!(owner.session.realization().unwrap().play.is_none());
            assert_eq!(
                owner.last_execution.as_ref().unwrap()["phase"],
                "local-terminal"
            );
            let saved = report.local_execution().fore_deliveries()[0].bytes.to_vec();
            assert!(
                conduit_data::DataReference::decode_for(&kind_id("value/text"), &saved).is_ok()
            );
            let history = state::execution_history(&root).unwrap();
            let receipts = history["receipts"].as_array().unwrap();
            assert_eq!(receipts.len(), 2);
            assert_eq!(receipts[0]["phase"], "local-start");
            assert_eq!(receipts[1]["phase"], "local-terminal");
            assert_eq!(receipts[0]["play"], receipts[1]["play"]);
            assert_ne!(
                receipts[0]["local_start_sign"],
                receipts[1]["local_terminal_sign"]
            );
            assert!(owner
                .run_selected_issuing_text_file_plan(&root, &issued, true, &input)
                .is_err());
            assert_eq!(state::execution_history(&root).unwrap(), history);
            drop(report);
            drop(owner);
            state::recover(&root).unwrap();
            assert_eq!(state::load(&root).unwrap().unwrap(), before);
            assert_eq!(state::execution_history(&root).unwrap(), history);
            assert_eq!(fs::read_dir(&selected.directory).unwrap().count(), 2); // owner state and exact published file
            fs::remove_dir_all(selected.directory).unwrap();
        })
        .unwrap()
        .join()
        .unwrap();
}
