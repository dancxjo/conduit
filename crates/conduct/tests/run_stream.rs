use conduct::run_stream::{
    RUN_CHANNEL_CHUNK_MAX_BYTES, RUN_CHANNEL_CHUNK_MAX_HEX_BYTES, RUN_CHANNEL_RECORD_MAX_BYTES,
    RUN_STREAM_SCHEMA, RUN_STREAM_SCHEMA_VERSION, RUN_STRUCTURED_RECORD_MAX_BYTES,
    RUN_SUMMARY_RECORD_MAX_BYTES, RunNdjsonState, RunStreamSchemaError,
    validate_run_stream_version,
};
use conduit_runtime::OwnedExecutionEvent;

const FIXTURE: &str = include_str!("../../../conformance/c3/conduct-run-stream.json");

#[test]
fn current_version_limits_and_direct_records_match_the_implementation() {
    let fixture: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    let limits = &fixture["limits"];
    assert_eq!(limits["decoded_chunk_bytes"], RUN_CHANNEL_CHUNK_MAX_BYTES);
    assert_eq!(
        limits["encoded_payload_bytes"],
        RUN_CHANNEL_CHUNK_MAX_HEX_BYTES
    );
    assert_eq!(
        limits["serialized_channel_record_bytes"],
        RUN_CHANNEL_RECORD_MAX_BYTES
    );
    assert_eq!(
        limits["serialized_summary_record_bytes"],
        RUN_SUMMARY_RECORD_MAX_BYTES
    );
    assert_eq!(
        limits["serialized_structured_record_bytes"],
        RUN_STRUCTURED_RECORD_MAX_BYTES
    );

    for case in fixture["version_cases"].as_array().unwrap() {
        let expected = match case["expected"]["reason"].as_str() {
            None => Ok(()),
            Some("unsupported-version") => Err(RunStreamSchemaError::Unsupported),
            reason => panic!("unexpected fixture reason {reason:?}"),
        };
        assert_eq!(
            validate_run_stream_version(
                case["schema"].as_str().unwrap(),
                case["schema_version"].as_u64().unwrap() as u16
            ),
            expected,
            "{}",
            case["id"]
        );
    }

    let mut channel_stream = RunNdjsonState::new(Vec::new());
    channel_stream
        .write_channel_chunk("stdout", &[0xff; RUN_CHANNEL_CHUNK_MAX_BYTES])
        .unwrap();
    assert!(channel_stream.inner.len() <= RUN_CHANNEL_RECORD_MAX_BYTES);
    let channel: serde_json::Value = serde_json::from_slice(&channel_stream.inner).unwrap();
    assert_eq!(channel["schema"], RUN_STREAM_SCHEMA);
    assert_eq!(channel["schema_version"], RUN_STREAM_SCHEMA_VERSION);
    assert_eq!(channel["record"], "channel_chunk");
    assert_eq!(channel["payload_bytes"], RUN_CHANNEL_CHUNK_MAX_BYTES);
    assert!(channel.get("event").is_none());
    assert!(channel.get("node").is_none());
    assert!(channel.get("port").is_none());

    let first_event = include_str!("../../../conformance/c2/execution-event.ndjson")
        .lines()
        .next()
        .unwrap();
    let event: OwnedExecutionEvent = serde_json::from_str(first_event).unwrap();
    let expected_event: serde_json::Value = serde_json::from_str(first_event).unwrap();
    let mut event_stream = RunNdjsonState::new(Vec::new());
    event_stream.write_execution_event(&event).unwrap();
    assert!(event_stream.inner.len() <= RUN_STRUCTURED_RECORD_MAX_BYTES);
    let outer: serde_json::Value = serde_json::from_slice(&event_stream.inner).unwrap();
    assert_eq!(outer["record"], "execution_event");
    assert_eq!(outer["event"], expected_event);
    assert!(outer.get("channel").is_none());
}
