//! Deterministic operator capture proof; no model or physical serial device.
use super::super::clock::ClockSource;
use super::*;
use conduit_core::InfoDecodeError;
use std::collections::VecDeque;
const FIXTURE_CLOCK: &str = "clock/fixture/prepared-source";
struct FixtureClock(u64);
impl ClockSource for FixtureClock {
    fn basis(&self) -> &str {
        FIXTURE_CLOCK
    }
    fn millis(&mut self) -> Result<u64, String> {
        let ticks = self.0;
        self.0 += 1;
        Ok(ticks)
    }
}

#[derive(Default)]
struct Provider {
    bytes: VecDeque<u8>,
    writes: Vec<Vec<u8>>,
    read_failed: bool,
    pause_failed: bool,
}
impl CreateUartProvider for Provider {
    type Error = ();
    fn is_available(&self) -> bool {
        true
    }
    fn profile(&self) -> UartProfile {
        UartProfile::CREATE_OI
    }
    fn write_all(&mut self, bytes: &[u8]) -> Result<(), ()> {
        self.writes.push(bytes.to_vec());
        if self.pause_failed && bytes == [150, 0] {
            Err(())
        } else {
            Ok(())
        }
    }
    fn read_byte(&mut self, _: u64) -> Result<Option<u8>, ()> {
        if self.read_failed {
            Err(())
        } else {
            Ok(self.bytes.pop_front())
        }
    }
}
fn wire_frame(capacity: u16) -> Vec<u8> {
    let mut group = [0; 26];
    group[17..19].copy_from_slice(&14_200u16.to_be_bytes());
    group[22..24].copy_from_slice(&1_200u16.to_be_bytes());
    group[24..26].copy_from_slice(&capacity.to_be_bytes());
    let mut frame = vec![19, 29, 0];
    frame.extend(group);
    frame.extend([34, 1]);
    frame.push(0u8.wrapping_sub(frame.iter().fold(0u8, |sum, b| sum.wrapping_add(*b))));
    frame
}
fn identity() -> StdCreateUartIdentity {
    StdCreateUartIdentity {
        base_id: "fixture/create".into(),
        device_path: "/fixture/serial".into(),
        device_number: 123,
        profile: UartProfile::CREATE_OI,
        maximum_write_wait_ms: 250,
    }
}
fn receipt(bytes: &[u8]) -> Value {
    let line = std::str::from_utf8(bytes).unwrap();
    assert_eq!(line.lines().count(), 1);
    serde_json::from_str(line.trim_end().strip_prefix(PREFIX).unwrap()).unwrap()
}
fn shared_frame(
    provider: Option<&mut Provider>,
    namespace: &str,
) -> (Result<conduit_pete::PeteCognitionFrame, String>, Value) {
    let identity = identity();
    let mut output = Vec::new();
    let result = super::super::frame_with_namespace_using(
        provider,
        CaptureContext {
            identity: Some(&identity),
            revision: 7,
            namespace,
        },
        "What is your current battery?".into(),
        false,
        &mut FixtureClock(100),
        &mut output,
    );
    (result, receipt(&output))
}

#[test]
fn shared_cli_path_emits_correlated_sample_for_ordinary_and_cadenced_namespaces() {
    for namespace in ["", "cadenced-fixture"] {
        let mut provider = Provider {
            bytes: wire_frame(2_400).into(),
            ..Provider::default()
        };
        let (frame, record) = shared_frame(Some(&mut provider), namespace);
        let frame = frame.unwrap();
        let conduit_pete::PeteCognitionBatteryInput::Observed(sample) = frame.battery() else {
            panic!("expected observed")
        };
        assert_eq!(*sample.charge_permille(), 500);
        assert_eq!(sample.observed_at().clock_basis(), FIXTURE_CLOCK);
        assert_eq!(frame.utterance().observed_at().clock_basis(), FIXTURE_CLOCK);
        assert_eq!(frame.reference().clock_basis(), FIXTURE_CLOCK);
        assert_eq!(*frame.utterance().observed_at().ticks(), 100);
        assert_eq!(*sample.observed_at().ticks(), 102);
        assert_eq!(*frame.reference().ticks(), 103);
        for time in [
            sample.observed_at(),
            frame.utterance().observed_at(),
            frame.reference(),
        ] {
            assert_eq!(*time.resolution_ticks(), 1);
            assert_eq!(*time.uncertainty_ticks(), 0);
            assert_eq!(
                *time.scale(),
                conduit_time::NativeTemporalScale::Milliseconds
            );
        }
        assert_eq!(record["schema"], "conduit.pete/capture-evidence@1");
        assert_eq!(record["source"], super::super::CREATE_SOURCE);
        assert_eq!(record["frame_revision"], 7);
        assert_eq!(record["frame_namespace"], namespace);
        assert_eq!(record["selected_device"]["device_number"], 123);
        assert_eq!(record["selected_device"]["device_path"], "/fixture/serial");
        assert_eq!(record["selected_device"]["uart_profile"]["baud"], 57_600);
        assert_eq!(record["clock"]["basis"], FIXTURE_CLOCK);
        assert_eq!(record["read_timeout_ms"], 1_000);
        let tick = record["clock"]["deadline_basis_tick"].as_u64().unwrap();
        assert_eq!(record["clock"]["read_deadline_tick"], tick + 1_000);
        assert_eq!(
            record["clock"]["post_read_tick"],
            *sample.observed_at().ticks()
        );
        assert_eq!(record["outcome"], "sample");
        assert_eq!(record["pause_write"]["disposition"], "write_completed");
        assert_eq!(
            record["pause_write"]["robot_acknowledgment"],
            "not_established"
        );
        assert_eq!(provider.writes.last().unwrap(), &[150, 0]);
        assert_eq!(
            provider
                .writes
                .iter()
                .filter(|bytes| bytes.starts_with(&[148]))
                .count(),
            1
        );
    }
}

#[test]
fn no_selected_sensor_and_valid_capture_without_capacity_keep_missing_input_distinct() {
    let mut bytes = Vec::new();
    let result = super::super::frame_with_namespace_using::<Provider>(
        None,
        CaptureContext {
            identity: None,
            revision: 1,
            namespace: "",
        },
        "Hello".into(),
        false,
        &mut FixtureClock(100),
        &mut bytes,
    )
    .unwrap();
    let no_sensor = receipt(&bytes);
    assert!(matches!(
        result.battery(),
        conduit_pete::PeteCognitionBatteryInput::Missing
    ));
    assert_eq!(no_sensor["outcome"], "no_selected_sensor");
    assert!(no_sensor["selected_device"].is_null());
    assert!(no_sensor["clock"]["read_deadline_tick"].is_null());
    assert_eq!(no_sensor["pause_write"]["disposition"], "not_attempted");
    let mut provider = Provider {
        bytes: wire_frame(0).into(),
        ..Provider::default()
    };
    let (result, missing_capacity) = shared_frame(Some(&mut provider), "");
    assert!(matches!(
        result.unwrap().battery(),
        conduit_pete::PeteCognitionBatteryInput::Missing
    ));
    assert_eq!(missing_capacity["outcome"], "missing_capacity");
    assert_eq!(
        missing_capacity["pause_write"]["disposition"],
        "write_completed"
    );
}

#[test]
fn shared_cli_path_keeps_distinct_read_failures_unavailable_without_retry() {
    let mut malformed = wire_frame(2_400);
    *malformed.last_mut().unwrap() ^= 1;
    let mut semantic = wire_frame(2_400);
    // Protocol-valid u16 voltage exceeds the portable battery semantic bound.
    semantic[20..22].copy_from_slice(&65_000u16.to_be_bytes());
    semantic[31] = 0;
    semantic[31] = 0u8.wrapping_sub(semantic.iter().fold(0u8, |sum, b| sum.wrapping_add(*b)));
    let cases = [
        (
            Provider {
                bytes: semantic.into(),
                ..Provider::default()
            },
            "semantic_value_invalid",
        ),
        (Provider::default(), "device_no_response"),
        (
            Provider {
                bytes: malformed.into(),
                ..Provider::default()
            },
            "malformed_frame",
        ),
        (
            Provider {
                bytes: vec![19, 29, 0].into(),
                ..Provider::default()
            },
            "truncated_frame",
        ),
        (
            Provider {
                bytes: vec![0; 65].into(),
                ..Provider::default()
            },
            "stream_synchronization_limit",
        ),
        (
            Provider {
                read_failed: true,
                ..Provider::default()
            },
            "read_failed",
        ),
    ];
    for (mut provider, code) in cases {
        let (result, record) = shared_frame(Some(&mut provider), "read-fixture");
        assert!(matches!(
            result.unwrap().battery(),
            conduit_pete::PeteCognitionBatteryInput::Unavailable(_)
        ));
        assert_eq!(record["outcome"], "capture_failed");
        assert_eq!(record["capture_stage"], "read");
        assert_eq!(record["failure"]["code"], code);
        assert_eq!(record["pause_write"]["disposition"], "write_completed");
        assert!(record["pause_write"]["failure"].is_null());
        assert_eq!(
            provider
                .writes
                .iter()
                .filter(|bytes| bytes.starts_with(&[148]))
                .count(),
            1
        );
        assert_eq!(
            provider
                .writes
                .iter()
                .filter(|bytes| bytes.starts_with(&[150]))
                .count(),
            1
        );
    }
}

#[test]
fn shared_cli_emits_both_read_and_pause_failure_before_preserving_refusal() {
    let mut provider = Provider {
        pause_failed: true,
        ..Provider::default()
    };
    let (result, record) = shared_frame(Some(&mut provider), "refusal-fixture");
    assert!(result.unwrap_err().contains("pause_failure: Some"));
    assert_eq!(record["failure"]["code"], "device_no_response");
    assert_eq!(record["pause_write"]["disposition"], "failed");
    assert_eq!(record["pause_write"]["failure"]["code"], "write_failed");
    let mut provider = Provider {
        bytes: wire_frame(2_400).into(),
        pause_failed: true,
        ..Provider::default()
    };
    let (result, record) = shared_frame(Some(&mut provider), "");
    assert!(result.unwrap_err().contains("Pause"));
    assert_eq!(record["capture_stage"], "pause");
    assert_eq!(record["failure"]["code"], "write_failed");
}

fn synthetic(error: CreateCognitionCaptureError<()>) -> Value {
    record(
        &CaptureContext {
            identity: None,
            revision: 4,
            namespace: "synthetic",
        },
        Some(&Err(error)),
        [Some(100), None],
        FIXTURE_CLOCK,
    )
}

#[test]
fn exact_protocol_payloads_and_semantic_failures_are_retained_without_overclaiming_cause() {
    use CreateObservationFailure as Failure;
    let cases = [
        (
            Failure::Protocol(CreateOiFailure::WrongUartProfile {
                observed: UartProfile::CREATE_OI_19200,
            }),
            "wrong_uart_profile",
            json!({"baud":19_200,"data_bits":8,"stop_bits":1,"parity":"None"}),
        ),
        (
            Failure::Protocol(CreateOiFailure::UnsupportedPacket(99)),
            "unsupported_packet",
            json!({"packet":99}),
        ),
        (
            Failure::Protocol(CreateOiFailure::SynchronizationLimit {
                maximum_discarded_bytes: 64,
            }),
            "stream_synchronization_limit",
            json!({"maximum_discarded_bytes":64}),
        ),
        (
            Failure::Lowering(CreateSensorLoweringError::WrongPacket {
                expected: 0,
                actual: 34,
            }),
            "wrong_packet",
            json!({"expected":0,"actual":34}),
        ),
        (
            Failure::Lowering(CreateSensorLoweringError::Semantic(
                InfoDecodeError::NonCanonicalBoolean(2),
            )),
            "semantic_value_invalid",
            json!({"semantic_code":"non_canonical_boolean","actual":2}),
        ),
        (
            Failure::Protocol(CreateOiFailure::ProviderUnavailable),
            "provider_unavailable",
            Value::Null,
        ),
    ];
    for (failure, code, payload) in cases {
        let record = synthetic(CreateCognitionCaptureError::Read {
            failure,
            pause_failure: None,
        });
        assert_eq!(record["failure"]["code"], code);
        assert_eq!(record["failure"]["payload"], payload);
        assert_eq!(record["failure"]["debug_detail"], format!("{failure:?}"));
    }
}

#[test]
fn early_clock_and_late_refusals_do_not_invent_a_pause_acknowledgment_or_known_phase() {
    let record = synthetic(CreateCognitionCaptureError::Clock(()));
    assert_eq!(record["capture_stage"], "clock_phase_unknown");
    assert_eq!(record["pause_write"]["disposition"], "unknown");
    let record = synthetic(CreateCognitionCaptureError::Start(
        CreateObservationFailure::Protocol(CreateOiFailure::WriteFailed),
    ));
    assert_eq!(record["capture_stage"], "start");
    assert_eq!(record["pause_write"]["disposition"], "not_attempted");
    let record = synthetic(CreateCognitionCaptureError::Observation(
        conduit_pete::PeteCognitionRefusal::InvalidObservation,
    ));
    assert_eq!(record["capture_stage"], "observation");
    assert_eq!(record["failure"]["code"], "observation_refused");
    assert_eq!(record["failure"]["refusal"], "invalid_observation");
    assert_eq!(record["pause_write"]["disposition"], "write_completed");
}

#[test]
fn capture_records_exact_clock_reads_without_extra_clock_or_serial_attempts() {
    let mut provider = Provider::default();
    let mut ticks = [100u64, 125].into_iter();
    let mut output = Vec::new();
    let result = capture(
        Some(&mut provider),
        CaptureContext {
            identity: None,
            revision: 2,
            namespace: "clock-fixture",
        },
        PeteCognitionSign::new("fixture/battery".into()).unwrap(),
        FIXTURE_CLOCK,
        || Ok::<_, ()>(ticks.next().unwrap()),
        &mut output,
    )
    .unwrap();
    assert!(matches!(
        result,
        Some(Err(CreateCognitionCaptureError::Read {
            pause_failure: None,
            ..
        }))
    ));
    let record = receipt(&output);
    assert_eq!(record["clock"]["deadline_basis_tick"], 100);
    assert_eq!(record["clock"]["read_deadline_tick"], 1_100);
    assert_eq!(record["clock"]["post_read_tick"], 125);
    assert!(ticks.next().is_none());
}

#[test]
fn selected_identity_retains_non_utf8_path_bytes_without_serialization_failure() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let mut identity = identity();
    let raw_path = b"/fixture/serial-\xff".to_vec();
    identity.device_path = OsString::from_vec(raw_path.clone()).into();
    let record = record::<()>(
        &CaptureContext {
            identity: Some(&identity),
            revision: 1,
            namespace: "",
        },
        None,
        [None, None],
        FIXTURE_CLOCK,
    );
    assert!(record["selected_device"]["device_path"].is_null());
    assert_eq!(
        record["selected_device"]["device_path_bytes"],
        json!(raw_path)
    );
}
