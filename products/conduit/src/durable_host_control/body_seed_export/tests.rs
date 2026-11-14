use super::*;
use crate::durable_host_control::{handle, Request, Response, PROTOCOL};
use conduit_std_host::StdHost;

fn bare() -> DurableHostRuntime {
    DurableHostRuntime::new("test/target".into(), "sha256:test".into(), StdHost::new())
}
fn request(token: &[u8]) -> Request {
    Request::BodyExportSeed {
        protocol: PROTOCOL,
        token: token.to_vec(),
        destination: "unused-seed.zip".into(),
    }
}
#[test]
fn local_export_requires_current_control_token_and_actual_body_owner() {
    let mut runtime = bare();
    assert!(
        matches!(handle(request(&[8;32]),&[9;32],&mut runtime),Response::Refused {code,..} if code=="unauthorized")
    );
    assert!(
        matches!(handle(request(&[9;32]),&[9;32],&mut runtime),Response::Refused {code,..} if code.contains("does not own a live Body"))
    );
}
#[test]
fn local_export_wire_preserves_destination_and_only_carrier_stage_receipt() {
    let encoded = serde_json::to_vec(&request(&[9; 32])).unwrap();
    let decoded: Request = serde_json::from_slice(&encoded).unwrap();
    assert!(
        matches!(decoded,Request::BodyExportSeed {destination,..} if destination==std::path::Path::new("unused-seed.zip"))
    );
    let receipt = conduit_host_make::DeploymentRealizationReceipt {
        schema: conduit_host_make::DEPLOYMENT_RECEIPT_SCHEMA.into(),
        carrier_id: "carrier/download".into(),
        implementation_id: "download@1".into(),
        target_id: "browser/wasm32/page".into(),
        spore_id: "spore/test".into(),
        artifact_content_sha256: format!("sha256:{}", "1".repeat(64)),
        terminal: conduit_host_make::CarrierTerminal::Completed,
        bytes_realized: 16,
        byte_verification_completed: true,
        explicit_authority_observed: true,
        boot_observed: false,
        join_observed: false,
        membership_admitted: false,
    };
    let encoded = serde_json::to_vec(&Response::BodySeedExported {
        protocol: PROTOCOL,
        receipt: receipt.clone(),
    })
    .unwrap();
    let Response::BodySeedExported {
        receipt: decoded, ..
    } = serde_json::from_slice(&encoded).unwrap()
    else {
        panic!("wrong response")
    };
    assert_eq!(decoded, receipt);
    assert!(!decoded.boot_observed && !decoded.join_observed && !decoded.membership_admitted);
}
#[test]
fn active_body_worker_refuses_export_before_any_carrier_effect() {
    use conduit_body::ResidentPlot;
    use std::{
        fs,
        time::{Duration, Instant},
    };
    let root = std::env::temp_dir().join(crate::durable_host::fresh_identity(
        "seed-export-active-test",
        "control",
    ));
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("installation.json"),serde_json::to_vec(&serde_json::json!({"schema":"conduit.install/durable-host@1","host_id":"host/seed-export-test","release_source_identity":"source/test","release_bundle_sha256":format!("sha256:{}","1".repeat(64)),"product_executable":"fixture-unused","body_state":null,"joined_body_state":null})).unwrap()).unwrap();
    let source = include_str!("../../../../../plots/clock/main.conduit");
    let checked = crate::plot_source::parse(source)
        .unwrap()
        .expand_entry_for_authoring()
        .unwrap();
    let resident = ResidentPlot::new(
        checked.expanded.source_document_id.clone(),
        checked.expanded.checked_plot_id.clone(),
    );
    let mut owner = crate::durable_host::owner::Owner::open(
        StdHost::new_with_config(conduit_std_host::StdHostConfig {
            host_id: "host/seed-export-test".into(),
            boot_id: "boot/seed-export-test".into(),
            offer_generation: conduit_core::OfferGeneration(1),
        }),
        resident,
        None,
        "Export gate",
    )
    .unwrap();
    owner.set_resident_plot_name(&checked).unwrap();
    owner.persist(&root).unwrap();
    fs::write(root.join("body/source.conduit"), source).unwrap();
    let worker = owner.start_service_run(&root, 1000).unwrap();
    let mut runtime = bare();
    runtime.host = HostSource::Body {
        owner: Box::new(owner),
        root: root.clone(),
        running: Some(super::super::body_run::OwnedRunWorker::General(worker)),
    };
    let destination = root.join("must-not-exist.zip");
    assert_eq!(
        runtime.export_owned_seed(&destination).unwrap_err(),
        "body-play-active"
    );
    assert!(
        matches!(handle(Request::BodyExportSeed{protocol:PROTOCOL,token:vec![9;32],destination:destination.clone()},&[9;32],&mut runtime),Response::Refused{code,..} if code=="body-play-active")
    );
    assert!(!destination.exists());
    let deadline = Instant::now() + Duration::from_secs(5);
    while runtime.host.body_is_running() {
        runtime.progress_owned_body().unwrap();
        assert!(
            Instant::now() < deadline,
            "finite Body worker failed to retire"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    drop(runtime);
    fs::remove_dir_all(root).unwrap();
}
