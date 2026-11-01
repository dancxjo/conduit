//! Actual owner lifecycle + selected isolated provider, using the existing actor fixture.
use super::*;

pub(super) fn exercise_owned_source(
    runtime: DurableHostRuntime,
    device: GatedDevice,
    cancel: bool,
) {
    let source =
        include_str!("../../../../../bodies/environment/profiles/source-observation.conduit");
    let checked = crate::plot_source::parse(source)
        .unwrap()
        .expand_entry_for_authoring()
        .unwrap();
    let resident = conduit_body::ResidentPlot::new(
        checked.expanded.source_document_id,
        checked.expanded.checked_plot_id,
    );
    let root = std::env::temp_dir().join(format!(
        "conduit-owned-native-{}",
        crate::birth_identity::fresh_uuid().unwrap()
    ));
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("installation.json"), serde_json::to_vec(&serde_json::json!({
        "schema":"conduit.install/durable-host@1", "host_id":"actor/native-host",
        "release_source_identity":"source/test", "release_bundle_sha256":format!("sha256:{}", "a".repeat(64)),
        "product_executable":"fixture-unused", "body_state":null, "joined_body_state":null
    })).unwrap()).unwrap();
    let prior = StdHost::new_with_config(StdHostConfig {
        host_id: "actor/native-host".into(),
        boot_id: "actor/prior-boot".into(),
        offer_generation: OfferGeneration(1),
    });
    let mut owner =
        crate::durable_host::owner::Owner::open(prior, resident, None, "Owned Source").unwrap();
    owner.persist(&root).unwrap();
    fs::write(root.join("body/source.conduit"), source).unwrap();
    drop(owner);
    let inventory = runtime.truth().advertisement;
    let mut runtime = runtime.with_owned_body(&root).unwrap();
    let session = runtime
        .selected_environmental_equipment
        .as_ref()
        .unwrap()
        .instruments[0]
        .session
        .session()
        .clone();
    // Two descriptions of one possession cannot supply unique attribution.
    // Refusal must preserve the actual equipment and leave no proposed Play.
    let equipment = runtime.selected_environmental_equipment.as_mut().unwrap();
    let original = &equipment.instruments[0];
    equipment.instruments.insert(
        0,
        SelectedInstrument {
            attachment_id: "ambiguous-alias".into(),
            source_configuration: original.source_configuration.clone(),
            firmware: original.firmware,
            session: original.session.clone(),
        },
    );
    assert!(runtime
        .start_owned_body(5_000, None)
        .unwrap_err()
        .contains("multiple equipment associations"));
    assert!(!host_is_out(&runtime));
    assert!(owner_truth(&runtime)["realization"].is_null());
    runtime
        .selected_environmental_equipment
        .as_mut()
        .unwrap()
        .instruments
        .remove(0);
    device.armed.store(true, Ordering::SeqCst);
    runtime.start_owned_body(5_000, None).unwrap();
    assert!(runtime.selected_environmental_equipment.is_none());
    let deadline = Instant::now() + Duration::from_secs(4);
    loop {
        runtime.progress_owned_body().unwrap();
        if device.entered.try_recv().is_ok() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "owned Source did not enter admitted IO"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
    let before = Instant::now();
    assert_eq!(runtime.truth().advertisement, inventory);
    assert!(before.elapsed() < Duration::from_millis(100));
    assert!(host_is_out(&runtime));
    if cancel {
        runtime.request_owned_body_lull().unwrap();
    }
    device.release.store(true, Ordering::SeqCst);
    finish(&mut runtime);
    assert!(!host_is_out(&runtime));
    assert_eq!(runtime.truth().advertisement, inventory);
    let equipment = runtime.selected_environmental_equipment.as_ref().unwrap();
    equipment
        .validate_owner(match &runtime.host {
            HostSource::Body { owner, .. } => owner.host.current(),
            _ => panic!("owner vanished"),
        })
        .unwrap();
    assert_eq!(equipment.instruments[0].session.session(), &session);
    let truth = owner_truth(&runtime);
    let receipt = &truth["last_execution"];
    assert!(receipt["play"].is_object());
    if !cancel {
        let selection = &receipt["accepted_source_fore"]["source_selection"];
        assert_eq!(selection["attachment_id"], "actor-device");
        assert_eq!(selection["host_id"], inventory.host_id.as_str());
        assert_eq!(selection["boot_id"], inventory.boot_id.as_str());
        assert_eq!(selection["provider_instance_id"], session.as_str());
        assert_eq!(
            selection["firmware_sha256"],
            serde_json::to_value([1u8; 32]).unwrap()
        );
        assert_eq!(
            selection["capability_id"],
            equipment.instruments[0].session.capability().as_str()
        );
        let native = &receipt["accepted_source_fore"]["native_play"];
        assert_eq!(native["plan_id"], selection["plan_id"]);
        assert_eq!(native["host_id"], selection["host_id"]);
        assert_eq!(native["boot_id"], selection["boot_id"]);
    }
    if cancel {
        assert!(
            receipt.get("accepted_source_fore").is_none(),
            "late cancelled output was published"
        );
    } else {
        let first = observation(receipt);
        assert!(matches!(
            first.outcome,
            conduit_environment_measurement::source_observation::SourceOutcome::Available(_)
        ));
        runtime.start_owned_body(5_000, None).unwrap();
        finish(&mut runtime);
        let next = owner_truth(&runtime);
        assert_ne!(receipt["play"], next["last_execution"]["play"]);
        let second = observation(&next["last_execution"]);
        let available =
            |o: conduit_environment_measurement::source_observation::SourceObservation| match o
                .outcome
            {
                conduit_environment_measurement::source_observation::SourceOutcome::Available(
                    sample,
                ) => sample.source_counter,
                _ => panic!("source became unavailable"),
            };
        assert!(available(second) > available(first));
        assert_eq!(
            runtime
                .selected_environmental_equipment
                .as_ref()
                .unwrap()
                .instruments[0]
                .session
                .session(),
            &session
        );
    }
    drop(runtime);
    fs::remove_dir_all(root).unwrap();
}
fn finish(runtime: &mut DurableHostRuntime) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        runtime.progress_owned_body().unwrap();
        if matches!(&runtime.host, HostSource::Body { running: None, .. }) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "owned Source did not return Host/equipment"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}
fn owner_truth(runtime: &DurableHostRuntime) -> serde_json::Value {
    match &runtime.host {
        HostSource::Body { owner, .. } => owner.truth(),
        _ => panic!("Body owner vanished"),
    }
}
fn observation(
    receipt: &serde_json::Value,
) -> conduit_environment_measurement::source_observation::SourceObservation {
    let fore = &receipt["accepted_source_fore"];
    assert_eq!(fore["custody"], "accepted-typed-fore-not-durable-recording");
    let bytes: Vec<u8> = serde_json::from_value(fore["canonical_bytes"].clone()).unwrap();
    conduit_environment_measurement::source_observation::ObservationContract::prepare()
        .unwrap()
        .decode(&bytes)
        .unwrap()
}

/// Three accepted canonical observations reach the shared graph during the
/// actual admitted Body Play. Lull removes authority, not last-known storage.
pub(super) fn exercise_continuing_source(runtime: DurableHostRuntime, device: GatedDevice) {
    let source =
        include_str!("../../../../../bodies/environment/profiles/continuing-observation.conduit")
            .replace("count: Count = 16", "count: Count = 8")
            .replace("period-ms: Count = 30000", "period-ms: Count = 100");
    let checked = crate::plot_source::parse(&source)
        .unwrap()
        .expand_entry_for_authoring()
        .unwrap();
    let resident = conduit_body::ResidentPlot::new(
        checked.expanded.source_document_id,
        checked.expanded.checked_plot_id,
    );
    let root = std::env::temp_dir().join(format!(
        "conduit-owned-native-{}",
        crate::birth_identity::fresh_uuid().unwrap()
    ));
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("installation.json"), serde_json::to_vec(&serde_json::json!({
        "schema":"conduit.install/durable-host@1", "host_id":"actor/native-host",
        "release_source_identity":"source/test", "release_bundle_sha256":format!("sha256:{}", "a".repeat(64)),
        "product_executable":"fixture-unused", "body_state":null, "joined_body_state":null
    })).unwrap()).unwrap();
    let prior = StdHost::new_with_config(StdHostConfig {
        host_id: "actor/native-host".into(),
        boot_id: "actor/prior-boot".into(),
        offer_generation: OfferGeneration(1),
    });
    let mut owner =
        crate::durable_host::owner::Owner::open(prior, resident, None, "Owned Source").unwrap();
    owner.persist(&root).unwrap();
    fs::write(root.join("body/source.conduit"), &source).unwrap();
    drop(owner);
    let inventory = runtime.truth().advertisement;
    let mut runtime = runtime.with_owned_body(&root).unwrap();

    device.release.store(true, Ordering::SeqCst);
    runtime.start_owned_body(5_000, None).unwrap();
    let graph = wait_for_graph(&mut runtime, 3);
    assert_eq!(graph.collector_host(), inventory.host_id.as_str());
    assert_eq!(graph.collector_boot(), inventory.boot_id.as_str());
    assert_eq!(graph.series().len(), 1);
    assert_eq!(graph.series()[0].name, "actor-device");
    assert!(host_is_out(&runtime), "Face came from a retired Play");
    let first_counter = graph.series()[0].points.last().unwrap().sequence;
    let last_time = graph.series()[0]
        .points
        .last()
        .unwrap()
        .observation
        .observed_at_host_boot_ms;
    assert!(matches!(
        graph.series()[0].state_at(last_time + 90_001).unwrap(),
        conduit_environment_history::source_series::SourceSeriesState::Stale { .. }
    ));
    assert!(graph.series()[0].points.len() >= 3, "staleness erased view");
    runtime.request_owned_body_lull().unwrap();
    finish(&mut runtime);
    let idle = match &runtime.host {
        HostSource::Body { owner, .. } => owner.local_face_snapshot().unwrap(),
        _ => panic!("owner vanished"),
    };
    assert!(
        conduit_environment_history::source_series::SourceSeriesGraph::prepare(
            &idle,
            "environmental-observations"
        )
        .is_err(),
        "retired Source supplied current Face"
    );
    // Re-admission keeps the same provider session and earlier canonical rows.
    runtime.start_owned_body(5_000, None).unwrap();
    let next = wait_for_graph(&mut runtime, 4);
    assert!(next.series()[0].points.last().unwrap().sequence > first_counter);
    assert_eq!(next.series()[0].points[0], graph.series()[0].points[0]);
    runtime.request_owned_body_lull().unwrap();
    finish(&mut runtime);
    assert!(!host_is_out(&runtime));
    assert_eq!(runtime.truth().advertisement, inventory);
    drop(runtime);
    fs::remove_dir_all(root).unwrap();
}
fn wait_for_graph(
    runtime: &mut DurableHostRuntime,
    count: usize,
) -> conduit_environment_history::source_series::SourceSeriesGraph {
    let deadline = Instant::now() + Duration::from_secs(4);
    loop {
        runtime.progress_owned_body().unwrap();
        let face = match &runtime.host {
            HostSource::Body { owner, .. } => owner.local_face_snapshot().unwrap(),
            _ => panic!("owner vanished"),
        };
        if let Ok(graph) = conduit_environment_history::source_series::SourceSeriesGraph::prepare(
            &face,
            "environmental-observations",
        ) {
            if graph.series()[0].points.len() >= count {
                return graph;
            }
        }
        assert!(
            Instant::now() < deadline,
            "current admitted Source Face did not acquire {count} samples; receipt={}; current={}", owner_truth(runtime)["last_execution"], owner_truth(runtime)["realization"].get("play").unwrap_or(&serde_json::Value::Null)
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}
