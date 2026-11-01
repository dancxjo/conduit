use super::*;
/// Actual confined HTTP worker plus ordinary native Body failure receipt.
pub(in super::super) fn exercise_refusal(
    runtime: DurableHostRuntime,
    device: GatedDevice,
    mode: u8,
) {
    let source =
        include_str!("../../../../../bodies/environment/profiles/continuing-observation.conduit")
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
        "conduit-source-refusal-{}",
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
        crate::durable_host::owner::Owner::open(prior, resident, None, "Refusal proof").unwrap();
    owner.persist(&root).unwrap();
    fs::write(root.join("body/source.conduit"), &source).unwrap();
    drop(owner);
    let inventory = runtime.truth().advertisement;
    let mut runtime = Box::new(runtime.with_owned_body(&root).unwrap());
    device.release.store(true, Ordering::SeqCst);
    runtime.start_owned_body(5000, None).unwrap();
    let graph = wait_for_graph(&mut runtime, 2);
    assert!(graph.series()[0].points.len() >= 2);
    let truth = owner_truth(&runtime);
    let fragment: conduit_core::PlanFragment = serde_json::from_value(
        truth["realization"]["plan"]["plots"][0]["plan"]["fragments"][0].clone(),
    )
    .unwrap();
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(&fragment).unwrap();
    let source = lowered
        .host_calls
        .iter()
        .find(|c| c.contract_id.as_str() == "environment/read-environmental-source")
        .unwrap();
    let (node, call) = (source.node.0, source.call.0);
    device.fault.store(mode, Ordering::SeqCst);
    finish(&mut runtime);
    let truth = owner_truth(&runtime);
    let receipt = &truth["last_execution"];
    assert!(
        receipt["accepted_source_fore"].is_null(),
        "failed native work published Fore"
    );
    let expected = match mode {
        1 => "transport-timeout",
        2 => "source-identity",
        3 => "source-continuity",
        4 => "malformed-observation",
        _ => unreachable!(),
    };
    assert_eq!(receipt["native_failure"]["category"], expected);
    assert_eq!(receipt["native_failure"]["node"], node);
    assert_eq!(receipt["native_failure"]["call"], call);
    // Acquisition reads once, first Source reads a baseline then its advance;
    // every later request reads exactly once in this controlled device.
    let failed_request = device.count.load(Ordering::SeqCst) - 3;
    assert!(failed_request >= 2);
    assert_eq!(receipt["native_failure"]["request"], failed_request);
    assert!(receipt["failure"]
        .as_str()
        .unwrap()
        .contains("HostCallFailed"));
    assert!(receipt["cleanup_failure"].is_null());
    let encoded = serde_json::to_string(receipt).unwrap();
    assert!(!encoded.contains("private-config-must-not-leak"));
    assert!(!encoded.contains("private-payload-must-not-leak"));
    assert_eq!(
        runtime.truth().advertisement,
        inventory,
        "failure lost original Host"
    );
    let reads = device.count.load(Ordering::SeqCst);
    runtime.start_owned_body(5000, None).unwrap();
    finish(&mut runtime);
    let receipt = owner_truth(&runtime)["last_execution"].clone();
    assert_eq!(receipt["native_failure"]["category"], "retired");
    assert!(receipt["accepted_source_fore"].is_null());
    assert_eq!(
        device.count.load(Ordering::SeqCst),
        reads,
        "retired equipment was reacquired"
    );
    drop(runtime);
    fs::remove_dir_all(root).unwrap();
}
