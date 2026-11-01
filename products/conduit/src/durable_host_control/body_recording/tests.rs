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
