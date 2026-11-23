//! Operator-only capture receipts. These never become model observations.
use conduit_core::{InfoDecodeError, QuantityConversionRefusal};
use conduit_pete::{
    capture_pete_cognition_battery, CreateCognitionCaptureError, CreateObservationFailure,
    CreateOiFailure, CreateSensorLoweringError, CreateUartProvider, PeteCognitionBattery,
    PeteCognitionRefusal, PeteCognitionSign, UartProfile,
};
use conduit_std_host::std_create_uart::StdCreateUartIdentity;
use serde_json::{json, Value};
use std::{fmt::Debug, io::Write, os::unix::ffi::OsStrExt};

const READ_TIMEOUT_MS: u32 = 1_000;
const PREFIX: &str = "CONDUIT_PETE_CAPTURE_EVIDENCE=";
type CaptureResult<E> = Result<Option<PeteCognitionBattery>, CreateCognitionCaptureError<E>>;

pub(super) struct CaptureContext<'a> {
    pub identity: Option<&'a StdCreateUartIdentity>,
    pub revision: u64,
    pub namespace: &'a str,
}

/// Emit before the caller converts the result to Missing/Unavailable/Observed.
/// Capture's clock error combines pre-start and post-read failures, so neither
/// its phase nor its pause-write disposition can be inferred from that variant.
pub(super) fn capture<P: CreateUartProvider, E: Debug>(
    provider: Option<&mut P>,
    context: CaptureContext<'_>,
    sign: PeteCognitionSign,
    mut clock: impl FnMut() -> Result<u64, E>,
    writer: &mut impl Write,
) -> Result<Option<CaptureResult<E>>, String> {
    let mut clock_ticks = [None; 2];
    let mut clock_index = 0;
    let result = provider.map(|provider| {
        capture_pete_cognition_battery(provider, sign, super::CLOCK.into(), READ_TIMEOUT_MS, || {
            let tick = clock();
            clock_ticks[clock_index] = tick.as_ref().ok().copied();
            clock_index += 1;
            tick
        })
    });
    let receipt = record(&context, result.as_ref(), clock_ticks);
    writeln!(writer, "{PREFIX}{receipt}")
        .map_err(|error| format!("write Pete capture evidence: {error}"))?;
    Ok(result)
}

fn record<E: Debug>(
    context: &CaptureContext<'_>,
    result: Option<&CaptureResult<E>>,
    clock_ticks: [Option<u64>; 2],
) -> Value {
    use CreateCognitionCaptureError as Error;
    let (outcome, stage, failure, pause, pause_failure) = match result {
        None => (
            "no_selected_sensor",
            "not_requested",
            Value::Null,
            "not_attempted",
            Value::Null,
        ),
        Some(Ok(sample)) => (
            if sample.is_some() {
                "sample"
            } else {
                "missing_capacity"
            },
            "completed",
            Value::Null,
            "write_completed",
            Value::Null,
        ),
        Some(Err(error)) => {
            let (stage, failure, pause, pause_failure) = match error {
                Error::Deadline => (
                    "deadline",
                    json!({"code":"invalid_deadline"}),
                    "not_attempted",
                    Value::Null,
                ),
                Error::Clock(error) => (
                    "clock_phase_unknown",
                    json!({"code":"clock_failed", "debug_detail":format!("{error:?}")}),
                    "unknown",
                    Value::Null,
                ),
                Error::Start(error) => (
                    "start",
                    observation_failure(*error),
                    "not_attempted",
                    Value::Null,
                ),
                Error::Read {
                    failure,
                    pause_failure,
                } => (
                    "read",
                    observation_failure(*failure),
                    if pause_failure.is_some() {
                        "failed"
                    } else {
                        "write_completed"
                    },
                    pause_failure.map_or(Value::Null, observation_failure),
                ),
                Error::Pause(error) => (
                    "pause",
                    observation_failure(*error),
                    "failed",
                    observation_failure(*error),
                ),
                Error::Observation(error) => (
                    "observation",
                    json!({"code":"observation_refused", "refusal":refusal_code(*error), "debug_detail":format!("{error:?}")}),
                    "write_completed",
                    Value::Null,
                ),
                Error::Time => (
                    "time",
                    json!({"code":"invalid_capture_time"}),
                    "write_completed",
                    Value::Null,
                ),
            };
            ("capture_failed", stage, failure, pause, pause_failure)
        }
    };
    json!({
        "schema": "conduit.pete/capture-evidence@1",
        "proof_class": "operator_capture_result",
        "source": super::CREATE_SOURCE,
        "frame_revision": context.revision,
        "frame_namespace": context.namespace,
        "selected_device": context.identity.map(|identity| json!({
            "base_id": identity.base_id,
            "device_path": identity.device_path.to_str(),
            "device_path_bytes": identity.device_path.as_os_str().as_bytes(),
            "device_number": identity.device_number,
            "uart_profile": profile(identity.profile),
            "maximum_write_wait_ms": identity.maximum_write_wait_ms,
        })),
        "clock": {"basis": super::CLOCK, "scale":"milliseconds", "resolution_ticks":1,
            "deadline_basis_tick":clock_ticks[0],
            "read_deadline_tick":clock_ticks[0].and_then(|tick| tick.checked_add(u64::from(READ_TIMEOUT_MS))),
            "post_read_tick":clock_ticks[1]},
        "read_timeout_ms": READ_TIMEOUT_MS,
        "outcome": outcome,
        "capture_stage": stage,
        "failure": failure,
        "pause_write": {"disposition":pause, "failure":pause_failure,
            "robot_acknowledgment":"not_established"},
        "diagnostic_detail_format":"rust_debug_not_stable",
    })
}

fn profile(profile: UartProfile) -> Value {
    json!({"baud":profile.baud, "data_bits":profile.data_bits,
        "stop_bits":profile.stop_bits, "parity":format!("{:?}", profile.parity)})
}

/// Match the stable std-observe codes without changing its separately owned
/// evidence surface. Debug detail retains available nested semantic refusals;
/// it does not identify checksum rejection or recover erased provider errno.
fn observation_failure(error: CreateObservationFailure) -> Value {
    use CreateObservationFailure as Failure;
    use CreateOiFailure as Protocol;
    let code = match error {
        Failure::InvalidState => "invalid_session_state",
        Failure::Lowering(CreateSensorLoweringError::WrongPacket { .. }) => "wrong_packet",
        Failure::Lowering(CreateSensorLoweringError::Semantic(_)) => "semantic_value_invalid",
        Failure::Protocol(Protocol::ProviderUnavailable) => "provider_unavailable",
        Failure::Protocol(Protocol::WrongUartProfile { .. }) => "wrong_uart_profile",
        Failure::Protocol(Protocol::WriteFailed) => "write_failed",
        Failure::Protocol(Protocol::ReadFailed) => "read_failed",
        Failure::Protocol(Protocol::Timeout) => "timeout",
        Failure::Protocol(Protocol::DeviceNoResponse) => "device_no_response",
        Failure::Protocol(Protocol::UnsupportedPacket(_)) => "unsupported_packet",
        Failure::Protocol(Protocol::TruncatedFrame) => "truncated_frame",
        Failure::Protocol(Protocol::MalformedFrame) => "malformed_frame",
        Failure::Protocol(Protocol::SynchronizationLimit { .. }) => "stream_synchronization_limit",
    };
    let payload = match error {
        Failure::Protocol(Protocol::WrongUartProfile { observed }) => profile(observed),
        Failure::Protocol(Protocol::UnsupportedPacket(packet)) => json!({"packet":packet}),
        Failure::Protocol(Protocol::SynchronizationLimit {
            maximum_discarded_bytes,
        }) => json!({"maximum_discarded_bytes":maximum_discarded_bytes}),
        Failure::Lowering(CreateSensorLoweringError::WrongPacket { expected, actual }) => {
            json!({"expected":expected,"actual":actual})
        }
        Failure::Lowering(CreateSensorLoweringError::Semantic(error)) => semantic_payload(error),
        _ => Value::Null,
    };
    json!({"code":code, "payload":payload, "debug_detail":format!("{error:?}")})
}

fn refusal_code(error: PeteCognitionRefusal) -> &'static str {
    match error {
        PeteCognitionRefusal::InvalidObservation => "invalid_observation",
        PeteCognitionRefusal::Situation => "situation",
        PeteCognitionRefusal::Experience => "experience",
        PeteCognitionRefusal::Evidence => "evidence",
        PeteCognitionRefusal::Homeostasis => "homeostasis",
    }
}

fn semantic_payload(error: InfoDecodeError) -> Value {
    use InfoDecodeError as Error;
    match error {
        Error::WrongLength { expected, actual } => {
            json!({"semantic_code":"wrong_length", "expected":expected, "actual":actual})
        }
        Error::NonCanonicalBoolean(actual) => {
            json!({"semantic_code":"non_canonical_boolean", "actual":actual})
        }
        Error::NonCanonicalEnum(actual) => {
            json!({"semantic_code":"non_canonical_enum", "actual":actual})
        }
        Error::InconsistentValue(reason) => {
            json!({"semantic_code":"inconsistent_value", "reason":reason})
        }
        Error::ReservedValue { field, actual } => {
            json!({"semantic_code":"reserved_value", "field":field, "actual":actual})
        }
        Error::OutOfRange {
            field,
            minimum,
            maximum,
            actual,
        } => {
            json!({"semantic_code":"out_of_range", "field":field, "minimum":minimum, "maximum":maximum, "actual":actual})
        }
        Error::QuantityConversion(refusal) => {
            let reason = match refusal {
                QuantityConversionRefusal::IncompatibleDimensions => "incompatible_dimensions",
                QuantityConversionRefusal::IncompatibleQuantityFamilies => {
                    "incompatible_quantity_families"
                }
                QuantityConversionRefusal::IncompatibleQuantityRoles => {
                    "incompatible_quantity_roles"
                }
                QuantityConversionRefusal::Inexact => "inexact",
                QuantityConversionRefusal::Overflow => "overflow",
            };
            json!({"semantic_code":"quantity_conversion", "reason":reason})
        }
    }
}

#[cfg(test)]
#[path = "capture_evidence_tests.rs"]
mod tests;
