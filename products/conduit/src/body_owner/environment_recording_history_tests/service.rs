//! An actual selected Source Owner retained by the ordinary local control actor.
use super::*;
use crate::durable_host_control::environment_history_fixture::{
    serve_owner, FixtureIdentity, FixtureStop,
};

#[test]
#[ignore = "requires a fresh bounded service directory and matching product CLI"]
fn retain_complete_recording_owner_for_live_browser_fixture() {
    let output = PathBuf::from(
        std::env::var_os("CONDUIT_ENVIRONMENT_HISTORY64_SERVICE_DIR").expect("service directory"),
    );
    let binary = PathBuf::from(
        std::env::var_os("CONDUIT_ENVIRONMENT_HISTORY64_SERVICE_BINARY").expect("product CLI"),
    )
    .canonicalize()
    .unwrap();
    assert!(binary.is_file());
    let allowed = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/environment-recording-source")
        .canonicalize()
        .unwrap();
    assert!(output.is_absolute());
    assert!(output
        .parent()
        .unwrap()
        .canonicalize()
        .unwrap()
        .starts_with(allowed));
    fs::create_dir(&output).expect("service fixture directory must be new");
    let (original, current) = installation();
    let root = output.join("state");
    fs::create_dir(&root).unwrap();
    fs::copy(
        original.join("installation.json"),
        root.join("installation.json"),
    )
    .unwrap();
    fs::remove_file(original.join("installation.json")).unwrap();
    fs::remove_dir(original).unwrap();
    let installation_path = root.join("installation.json");
    let mut installed: serde_json::Value =
        serde_json::from_slice(&fs::read(&installation_path).unwrap()).unwrap();
    installed["product_executable"] = binary.to_str().unwrap().into();
    fs::write(&installation_path, serde_json::to_vec(&installed).unwrap()).unwrap();
    let host = crate::durable_host::environment_recording_history64_host_fixture(current.clone());
    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../plots/environment-recording-history/main.conduit"
    ));
    let expanded = crate::plot_source::parse(SOURCE)
        .unwrap()
        .expand_entry()
        .unwrap();
    let resident = ResidentPlot::new(expanded.source_document_id, expanded.checked_plot_id);
    let mut owner = Owner::open(host, resident, None, "Complete original recording").unwrap();
    let sources = state::WorksetSources::checked(
        owner.session.evidence(),
        vec![(SOURCE.into(), "environment-recording-history".into())],
    )
    .unwrap();
    state::retain_session_with_workset_sources(&root, &mut owner.session, &sources, None, None)
        .unwrap();
    let body = owner.session.evidence().body_id.clone();
    fs::write(
        output.join("ready.json"),
        serde_json::to_vec(&serde_json::json!({
            "schema": "conduit.proof/environment-recording-owner-service@1",
            "state_dir": root, "binary": binary, "body_id": body,
            "owner_host_id": current.host_id, "owner_boot_id": current.boot_id,
            "fixture": "original-sealed-recordings", "physical_sensor": false,
        }))
        .unwrap(),
    )
    .unwrap();
    let served = serve_owner(
        &root,
        owner,
        None,
        FixtureIdentity {
            target_id: "test/original-recording-owner".into(),
            image_content_digest: "test/checked-selected-source64".into(),
        },
        Duration::from_secs(120),
        Duration::from_secs(15),
        || Ok(output.join("stop").is_file()),
    )
    .unwrap();
    assert_eq!(served.stop, FixtureStop::Requested);
    assert_eq!(served.owner.session.evidence().body_id, body);
    assert!(served.owner.current_play_id().is_none());
    assert_eq!(
        served.owner.session.evidence().body.state,
        conduit_body::BodyState::Lulled
    );
    assert_eq!(served.owner.host.advertisement().host_id, current.host_id);
    assert_eq!(served.owner.host.advertisement().boot_id, current.boot_id);
    fs::write(output.join("retired"), "ordinary Lull completed").unwrap();
}
