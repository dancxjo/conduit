//! Real installed-owner execution with non-Notebook Source and the same Body.
use super::*;
use conduit_std_host::{StdHost, StdHostConfig};
use std::fs::{self, File};
const PUBLISH: &str = include_str!("../../../../plots/notebook/publish.conduit");
const LOAD: &str = include_str!("../../../../plots/notebook/load.conduit");

fn select_source(owner: &mut Owner, root: &Path, text: &str) {
    let source = crate::plot_source::parse(text).unwrap();
    let plot = source.expand_entry_for_authoring().unwrap();
    let resident = ResidentPlot::new(
        plot.expanded.source_document_id.clone(),
        plot.expanded.checked_plot_id.clone(),
    );
    let ad = owner.host.advertisement().clone();
    if owner.resident.as_ref() != Some(&resident) {
        let old = owner.resident.as_ref().unwrap().clone();
        let revision = owner.session.evidence().body.workload_revision;
        owner
            .session
            .remove_plot(revision, &old, &ad.host_id, &ad.boot_id)
            .unwrap();
        owner
            .session
            .admit_plot(revision + 1, resident.clone(), &ad.host_id, &ad.boot_id)
            .unwrap();
        owner.resident = Some(resident);
    }
    owner.set_resident_plot_name(&plot).unwrap();
    state::retain_session(
        root,
        &mut owner.session,
        owner.last_execution.as_ref(),
        owner.admissions.as_ref(),
        Some(text.as_bytes()),
        None,
    )
    .unwrap();
}
fn settle(owner: &mut Owner, root: &Path, result: &TextFilePlayOutcome) {
    let mut next = owner.session.clone();
    assert_eq!(
        next.realization().unwrap().play.as_ref(),
        Some(&result.report.play)
    );
    next.lull(
        &result.host.host_id,
        &result.host.boot_id,
        Some(&result.report.play),
    )
    .unwrap();
    owner
        .retain_execution(
            root,
            next,
            serde_json::json!({
                "schema":"conduit.test/text-file-play@1", "play":result.report.play,
                "terminal":result.report.terminal,"terminal_sign":result.report.terminal_sign,
            }),
        )
        .unwrap();
}
#[test]
fn shared_text_file_play_publishes_and_reopens_non_notebook_content_on_one_body() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            let root = std::env::temp_dir().join(crate::durable_host::fresh_identity(
                "shared-text-owner",
                "recording",
            ));
            fs::create_dir_all(&root).unwrap();
            let installation = crate::durable_host::Installation {
                schema: crate::durable_host::INSTALL_SCHEMA.into(),
                host_id: "host/recorder".into(),
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
            let storage = root.join("selected-storage");
            fs::create_dir(&storage).unwrap();
            let publish = PUBLISH.replace("notebook/publish", "recording/publish");
            let load = LOAD.replace("notebook/load", "recording/load");
            let source = crate::plot_source::parse(&publish).unwrap();
            let plot = source.expand_entry_for_authoring().unwrap();
            let resident = ResidentPlot::new(
                plot.expanded.source_document_id.clone(),
                plot.expanded.checked_plot_id.clone(),
            );
            let host = StdHost::new_with_config(StdHostConfig {
                host_id: "host/recorder".into(),
                boot_id: "boot/recorder".into(),
                offer_generation: OfferGeneration(1),
            });
            let mut owner = Owner::open(host, resident, None, "Recording").unwrap();
            assert!(owner.notebook.is_none(), "runner needs no Notebook state");
            let body = owner.session.evidence().body_id.clone();
            let boot = owner.host.advertisement().boot_id.clone();
            select_source(&mut owner, &root, &publish);
            let content = b"original producer coordinates, retained as immutable Text";
            let reference = BoundedResourceRef {
                identity: ResourceSemanticIdentity::from_digest([17; 32]),
                content_profile: kind_id("value/text"),
                access_class: conduit_data::DATA_GENERATION_ACCESS_CLASS.into(),
                extent: ResourceExtent {
                    bytes: content.len() as u64,
                    items: None,
                },
                lifetime: ResourceLifetime {
                    version: ResourceVersionIdentity::from_digest([19; 32]),
                    expires_at: None,
                },
            };
            owner
                .host
                .transition_text_file_offer(File::open(&storage).unwrap(), reference.clone(), true)
                .unwrap();
            owner.plan_selected_text_file(&publish, true).unwrap();
            let saved = owner
                .run_selected_text_file_plan(&root, true, content.to_vec())
                .unwrap();
            assert_eq!(saved.report.terminal, TerminalDisposition::Completed);
            assert!(saved.report.failure.is_none() && saved.report.cleanup_failure.is_none());
            let encoded = saved.output.clone().unwrap();
            assert_eq!(
                conduit_data::DataReference::decode_for(&kind_id("value/text"), &encoded)
                    .unwrap()
                    .reference(),
                &reference
            );
            assert_eq!(fs::read_dir(&storage).unwrap().count(), 1);
            settle(&mut owner, &root, &saved);
            owner
                .host
                .transition_text_file_offer(File::open(&storage).unwrap(), reference, false)
                .unwrap();
            select_source(&mut owner, &root, &load);
            owner.plan_selected_text_file(&load, false).unwrap();
            let loaded = owner
                .run_selected_text_file_plan(&root, false, encoded)
                .unwrap();
            assert_eq!(loaded.report.terminal, TerminalDisposition::Completed);
            assert_eq!(loaded.output.as_deref(), Some(content.as_slice()));
            assert_ne!(
                saved.report.play.active_play_id,
                loaded.report.play.active_play_id
            );
            assert_eq!(loaded.report.play.body_id, body);
            assert_eq!(owner.host.advertisement().boot_id, boot);
            settle(&mut owner, &root, &loaded);
            assert!(owner.notebook.is_none());
            drop(owner);
            fs::remove_dir_all(root).unwrap();
        })
        .unwrap()
        .join()
        .unwrap();
}
