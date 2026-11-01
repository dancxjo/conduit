use super::super::{DurableHostRuntime, Request, Response, PROTOCOL};
use super::*;
fn request(operation: RecordingOperation) -> RecordingRequest {
    RecordingRequest {
        operation,
        maximum_millis: 5000,
        preparation_limit_bytes: 1024 * 1024,
        expected_body_id: "body/explicit".into(),
        expected_boot_id: "boot/explicit".into(),
    }
}
#[test]
fn recording_request_is_closed_and_never_accepts_reference_or_extra_authority_fields() {
    let value = serde_json::to_value(request(RecordingOperation::RecoverCatalog)).unwrap();
    assert!(serde_json::from_value::<RecordingRequest>(value.clone()).is_ok());
    for field in ["reference", "root_directory", "source", "acknowledged"] {
        let mut extra = value.clone();
        extra[field] = serde_json::json!("forbidden");
        assert!(serde_json::from_value::<RecordingRequest>(extra).is_err());
    }
    let mut extra = value.clone();
    extra["operation"]["authority"] = serde_json::json!({});
    assert!(serde_json::from_value::<RecordingRequest>(extra).is_err());
    let mut unknown = value;
    unknown["operation"]["operation"] = serde_json::json!("mint_grant");
    assert!(serde_json::from_value::<RecordingRequest>(unknown).is_err());
}
#[test]
fn recording_authentication_refuses_before_any_owner_preparation() {
    let mut runtime = DurableHostRuntime::new(
        "std/x86_64/computer".into(),
        "sha256:fixture".into(),
        conduit_std_host::StdHost::new(),
    );
    let before = runtime.truth().advertisement;
    for token in [vec![], vec![9; 32]] {
        let response = super::super::handle(
            Request::BodyRecording {
                protocol: PROTOCOL,
                token,
                request: Box::new(request(RecordingOperation::RecoverCatalog)),
            },
            &[7; 32],
            &mut runtime,
        );
        assert!(matches!(response,Response::Refused{code,..} if code=="unauthorized"));
        assert_eq!(runtime.truth().advertisement, before);
    }
}

#[cfg(all(
    feature = "installed-environmental-sources",
    feature = "installed-body-storage",
    target_os = "linux"
))]
mod owned {
    use super::super::super::body::HostSource;
    use super::*;
    use crate::durable_host::owner::{
        source_recording_control_fixture, source_recording_control_read_grant,
    };
    fn make_request(
        runtime: &DurableHostRuntime,
        operation: RecordingOperation,
    ) -> RecordingRequest {
        let HostSource::Body { owner, .. } = &runtime.host else {
            panic!("fixture owner missing")
        };
        RecordingRequest {
            operation,
            maximum_millis: 5000,
            preparation_limit_bytes: 1024 * 1024,
            expected_body_id: owner.current_body_id().as_str().into(),
            expected_boot_id: owner.host.advertisement().boot_id.as_str().into(),
        }
    }
    fn invoke(runtime: &mut DurableHostRuntime, request: RecordingRequest) -> Response {
        super::super::super::handle(
            Request::BodyRecording {
                protocol: PROTOCOL,
                token: vec![7; 32],
                request: Box::new(request),
            },
            &[7; 32],
            runtime,
        )
    }
    fn finish(runtime: &mut DurableHostRuntime) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            runtime.progress_owned_body().unwrap();
            if !matches!(
                &runtime.host,
                HostSource::Body {
                    running: Some(_),
                    ..
                }
            ) {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "recording worker did not quiesce"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
    fn assert_marker(runtime: &DurableHostRuntime, root: &Path) {
        let marker: serde_json::Value =
            serde_json::from_slice(&std::fs::read(root.join("runtime.json")).unwrap()).unwrap();
        assert_eq!(
            marker["offer_generation"],
            runtime.truth().advertisement.offer_generation.0
        );
    }
    #[test]
    fn authenticated_control_runs_all_six_operations_on_original_owner_and_reconciles_refusals() {
        let (root, _disk, mut owner, publish) = source_recording_control_fixture();
        let original_body = owner.current_body_id().clone();
        // Move the original StdHost through runtime construction; no fabricated
        // advertisement, second equipment owner, or replacement ledger.
        let host = owner.host.take_for_play().unwrap();
        let mut runtime =
            DurableHostRuntime::new("std/x86_64/computer".into(), "fixture-image".into(), host);
        let HostSource::Bare(mut held) =
            std::mem::replace(&mut runtime.host, HostSource::Transitioning)
        else {
            unreachable!()
        };
        owner
            .host
            .restore_after_play(held.take_for_play().unwrap())
            .unwrap();
        runtime.host = HostSource::Body {
            owner: Box::new(owner),
            root: root.clone(),
            running: None,
        };
        let ad = runtime.truth().advertisement;
        std::fs::write(root.join("runtime.json"),serde_json::to_vec(&serde_json::json!({"schema":"conduit.install/durable-host-runtime@1","host_id":ad.host_id,"boot_id":ad.boot_id,"offer_generation":ad.offer_generation.0,"process_id":std::process::id(),"release_bundle_sha256":"sha256:fixture","body_id":original_body})).unwrap()).unwrap();
        let before = runtime.truth().advertisement;
        for (body, boot, millis, prep) in [
            ("body/foreign", ad.boot_id.as_str(), 5000, 1024 * 1024),
            (original_body.as_str(), "boot/foreign", 5000, 1024 * 1024),
            (original_body.as_str(), ad.boot_id.as_str(), 0, 1024 * 1024),
            (original_body.as_str(), ad.boot_id.as_str(), 5000, 0),
            (
                original_body.as_str(),
                ad.boot_id.as_str(),
                5000,
                1024 * 1024 + 1,
            ),
        ] {
            let mut req = make_request(&runtime, RecordingOperation::RecoverCatalog);
            req.expected_body_id = body.into();
            req.expected_boot_id = boot.into();
            req.maximum_millis = millis;
            req.preparation_limit_bytes = prep;
            assert!(matches!(
                invoke(&mut runtime, req),
                Response::Refused { .. }
            ));
            assert_eq!(runtime.truth().advertisement, before);
        }
        let req = make_request(
            &runtime,
            RecordingOperation::Publish {
                authority: publish.clone(),
            },
        );
        assert!(matches!(
            invoke(&mut runtime, req),
            Response::BodyRunRequested { .. }
        ));
        assert!(matches!(
            &runtime.host,
            HostSource::Body {
                running: Some(super::super::super::body_run::OwnedRunWorker::General(_)),
                ..
            }
        ));
        let busy = make_request(&runtime, RecordingOperation::RecoverCatalog);
        assert!(
            matches!(invoke(&mut runtime,busy),Response::Refused{code,..} if code=="body-play-active")
        );
        finish(&mut runtime);
        assert_marker(&runtime, &root);
        // Wrong publication authority causes a selected read-generation change,
        // then refuses without a Play; marker must still expose that generation.
        let wrong = make_request(
            &runtime,
            RecordingOperation::Readback {
                authority: publish.clone(),
            },
        );
        assert!(matches!(
            invoke(&mut runtime, wrong),
            Response::Refused { .. }
        ));
        assert_marker(&runtime, &root);
        assert!(!matches!(
            &runtime.host,
            HostSource::Body {
                running: Some(_),
                ..
            }
        ));
        let read = match &mut runtime.host {
            HostSource::Body { owner, .. } => source_recording_control_read_grant(owner),
            _ => unreachable!(),
        };
        for operation in [
            RecordingOperation::Readback {
                authority: read.clone(),
            },
            RecordingOperation::Catalog { authority: publish },
            RecordingOperation::RecoverCatalog,
            RecordingOperation::ReadRecoveredCatalog {
                authority: read.clone(),
            },
            RecordingOperation::ReadRecoveredChunk {
                authority: read,
                index: 0,
            },
        ] {
            let req = make_request(&runtime, operation);
            let response = invoke(&mut runtime, req);
            assert!(
                matches!(response, Response::BodyRunRequested { .. }),
                "{}",
                serde_json::to_value(&response).unwrap()
            );
            finish(&mut runtime);
            assert_marker(&runtime, &root);
        }
        let truth = runtime.owned_body_truth().unwrap();
        let receipt = &truth["last_execution"];
        assert_eq!(
            receipt["source_recovered_read"]["read_completed"], true,
            "{truth}"
        );
        assert_eq!(
            receipt["source_recovered_read"]["producer_retirement_authorized"],
            false
        );
        assert_eq!(receipt["source_recording_pending"]["count"], 1);
        let HostSource::Body { owner, .. } = &runtime.host else {
            unreachable!()
        };
        assert_eq!(owner.current_body_id(), &original_body);
        assert!(!owner.host.is_playing());
        assert!(runtime.selected_environmental_equipment.is_none());
        drop(runtime);
        std::fs::remove_dir_all(root).unwrap();
    }
}
