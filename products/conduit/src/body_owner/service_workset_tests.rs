//! Already-retained workset recovery through the installed service worker.
//! Fixtures establish Source custody; they are not an adoption-authority proof.
use super::*;
use std::{
    fs,
    time::{Duration, Instant},
};
const SOURCE: &str = "plot alpha {\n show: face/text(\"Alpha\")\n \"first output\" >> show\n}\nplot beta {\n show: face/text(\"Beta\")\n \"second output\" >> show\n}.";

fn fixture() -> (std::path::PathBuf, Owner) {
    let root = std::env::temp_dir().join(super::super::super::fresh_identity(
        "service-workset",
        "resume",
    ));
    fs::create_dir_all(&root).unwrap();
    let installation = super::super::super::Installation {
        schema: super::super::super::INSTALL_SCHEMA.into(),
        host_id: "host/workset-service".into(),
        release_source_identity: "source/test".into(),
        release_bundle_sha256: super::super::super::digest(b"bundle"),
        product_executable: "fixture-unused".into(),
        body_state: None,
        joined_body_state: None,
        selected_speech: None,
        selected_model: None,
        selected_host_configuration: None,
        selected_todo_checkpoint: None,
    };
    super::super::super::write_json_atomic(&root.join("installation.json"), &installation).unwrap();
    let host = conduit_std_host::StdHost::new_with_config(conduit_std_host::StdHostConfig {
        host_id: installation.host_id.into(),
        boot_id: "boot/workset-service".into(),
        offer_generation: conduit_core::OfferGeneration(1),
    });
    let source = crate::plot_source::parse(SOURCE).unwrap();
    let a = source.expand_named_for_authoring("alpha").unwrap();
    let b = source.expand_named_for_authoring("beta").unwrap();
    let resident = |plot: &conduit_plot::ExpandedAuthoringPlot| {
        ResidentPlot::new(
            plot.expanded.source_document_id.clone(),
            plot.expanded.checked_plot_id.clone(),
        )
    };
    let mut owner = Owner::open(host, resident(&a), None, "Two checked Plots").unwrap();
    let ad = owner.host.advertisement();
    owner
        .session
        .admit_plot(0, resident(&b), &ad.host_id, &ad.boot_id)
        .unwrap();
    let sources = state::WorksetSources::checked(
        owner.session.evidence(),
        vec![
            (SOURCE.into(), "alpha".into()),
            (SOURCE.into(), "beta".into()),
        ],
    )
    .unwrap();
    state::retain_session_with_workset_sources(&root, &mut owner.session, &sources, None, None)
        .unwrap();
    (root, owner)
}

#[test]
fn resumed_complete_sources_start_one_service_play_and_retain_both_outputs() {
    let (root, original) = fixture();
    let body = original.session.evidence().body_id.clone();
    let workset = original.session.evidence().body.workset.clone();
    let host = conduit_std_host::StdHost::new_with_config(conduit_std_host::StdHostConfig {
        host_id: original.host.advertisement().host_id.clone(),
        boot_id: "boot/workset-resumed".into(),
        offer_generation: conduit_core::OfferGeneration(1),
    });
    drop(original);
    let mut owner = super::super::resume_service(host, &root).unwrap();
    assert_eq!(owner.session.evidence().body_id, body);
    assert_eq!(owner.session.evidence().body.workset, workset);
    let mut worker = owner.start_service_run(&root, 5_000).unwrap();
    assert_eq!(owner.session.realization().unwrap().plan.plots.len(), 2);
    assert!(owner.start_service_run(&root, 5_000).is_err());
    let until = Instant::now() + Duration::from_secs(10);
    while !worker.progress(&mut owner, &root).unwrap() {
        assert!(
            Instant::now() < until,
            "finite workset worker failed to retire"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    let report = owner.last_execution.as_ref().unwrap();
    assert_eq!(
        report["terminal"],
        serde_json::json!(conduit_core::TerminalDisposition::Completed)
    );
    let output = report["output_utf8"].as_str().unwrap();
    assert!(output.contains("first output"), "{output}");
    assert!(output.contains("second output"), "{output}");
    assert!(owner.session.realization().is_none());
    assert_eq!(
        owner.session.evidence().body.state,
        conduit_body::BodyState::Lulled
    );
    assert_eq!(owner.session.evidence().body.workset, workset);
    assert_eq!(state::load(&root).unwrap().unwrap().body_id, body);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn incomplete_or_modified_retained_sources_cannot_publish_a_partial_service_wake() {
    let (root, mut owner) = fixture();
    let before = owner.session.evidence().clone();
    let file = root.join("body/workset-sources.json");
    let original = fs::read(&file).unwrap();
    let mut value: serde_json::Value = serde_json::from_slice(&original).unwrap();
    value["entries"].as_array_mut().unwrap().pop();
    fs::write(&file, serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(owner.start_service_run(&root, 5_000).is_err());
    assert!(owner.session.realization().is_none());
    assert!(!owner.host.is_playing());
    assert_eq!(owner.session.evidence(), &before);
    let mut changed: serde_json::Value = serde_json::from_slice(&original).unwrap();
    changed["entries"][1]["source"] = serde_json::json!("unrelated bytes");
    fs::write(&file, serde_json::to_vec(&changed).unwrap()).unwrap();
    assert!(owner.start_service_run(&root, 5_000).is_err());
    assert!(owner.session.realization().is_none());
    assert_eq!(owner.session.evidence(), &before);
    fs::remove_file(&file).unwrap();
    fs::write(root.join("body/source.conduit"), SOURCE).unwrap();
    assert!(owner
        .start_service_run(&root, 5_000)
        .err()
        .unwrap()
        .contains("complete Body workset"));
    assert_eq!(owner.session.evidence(), &before);
    fs::remove_dir_all(root).unwrap();
}
