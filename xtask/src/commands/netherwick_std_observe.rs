//! Explicit non-actuating std UART entrance for one Create observation.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use clap::{Args, Subcommand};
use conduit_core::ChargingState;
use conduit_netherwick::{
    CreateObservationFailure, CreateObservationSession, CreatePortableObservation,
    CreateSensorLoweringError,
};
use conduit_std_host::std_create_uart::{
    monotonic_millis, StdCreateUartBase, StdCreateUartObservation, StdCreateUartOpenError,
    MAXIMUM_CREATE_UART_WRITE_WAIT_MS,
};
use serde::Serialize;

use crate::cli::GlobalOpts;
use crate::commands::netherwick_std_indicator::{self, StdIndicatorArgs};
use crate::commands::netherwick_std_speaker::{self, StdSpeakerArgs};

const EVIDENCE_SCHEMA: &str = "conduit.netherwick/std-create-observation-evidence@1";
const MAXIMUM_ID_BYTES: usize = 128;
const MAXIMUM_PATH_BYTES: usize = 4_096;
const MAXIMUM_READ_TIMEOUT_MS: u32 = 5_000;

#[derive(Args, Debug)]
pub struct NetherwickArgs {
    #[command(subcommand)]
    command: NetherwickCommand,
}

#[derive(Subcommand, Debug)]
enum NetherwickCommand {
    /// Observe one bounded correlated Create sensor frame without actuation.
    #[command(name = "std-observe")]
    Observe(StdObserveArgs),
    /// Play one bounded portable melody through an exact std Create UART Base.
    #[command(name = "std-speaker")]
    Speaker(StdSpeakerArgs),
    /// Manifest canonical Signal presentation on the Create power indicator.
    #[command(name = "std-indicator")]
    Indicator(StdIndicatorArgs),
}

#[derive(Args, Clone, Debug)]
struct StdObserveArgs {
    /// Exact Linux character-device path; no discovery is performed.
    #[arg(long)]
    serial_path: PathBuf,
    /// Caller-declared stable identity for this exact serial Base.
    #[arg(long)]
    base_id: String,
    /// Caller-declared Host identity retained as provenance, not discovered truth.
    #[arg(long)]
    host_id: String,
    /// Caller-declared Boot identity retained as provenance, not discovered truth.
    #[arg(long)]
    boot_id: String,
    /// Monotonic read bound for the one correlated observation.
    #[arg(long, default_value_t = 1_000)]
    read_timeout_ms: u32,
    /// New evidence destination. Existing evidence is never overwritten.
    #[arg(long)]
    evidence_out: PathBuf,
}

#[derive(Serialize)]
struct Evidence {
    schema: &'static str,
    proof_class: &'static str,
    declared_host_id: String,
    declared_boot_id: String,
    base: BaseEvidence,
    started_monotonic_ms: u64,
    read_deadline_monotonic_ms: u64,
    completed_monotonic_ms: u64,
    intended_robot_identity_verified: bool,
    outcome: Outcome,
}

#[derive(Serialize)]
struct BaseEvidence {
    base_id: String,
    serial_path: String,
    device_number: Option<u64>,
    baud: u32,
    data_bits: u8,
    stop_bits: u8,
    parity: &'static str,
}

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum Outcome {
    Observed {
        observation: PortableObservation,
    },
    Failed {
        stage: &'static str,
        code: &'static str,
        cleanup_code: Option<&'static str>,
    },
}

#[derive(Serialize)]
struct PortableObservation {
    contact_body_sectors: u8,
    cliff_body_sectors: u8,
    cliff_signal_available: u8,
    cliff_signals: [u16; 4],
    dropped_wheels: u8,
    proximity_body_sectors: u8,
    virtual_wall_present: bool,
    infrared_code: Option<u8>,
    pressed_buttons: u32,
    charging_state: &'static str,
    charging_sources: u8,
    millivolts: u16,
    milliamps: i16,
    temperature_celsius: i8,
    charge_mah: u16,
    capacity_mah: u16,
    battery_charge_permille: Option<u16>,
    distance_delta_mm: i16,
    angle_delta_degrees: i16,
}

pub fn run(args: NetherwickArgs, opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    match args.command {
        NetherwickCommand::Observe(args) => run_std_observe(args, opts),
        NetherwickCommand::Speaker(args) => netherwick_std_speaker::run(args, opts),
        NetherwickCommand::Indicator(args) => netherwick_std_indicator::run(args, opts),
    }
}

fn run_std_observe(
    args: StdObserveArgs,
    opts: &GlobalOpts,
) -> Result<(), Box<dyn std::error::Error>> {
    validate_args(&args)?;
    if opts.dry_run {
        if !opts.quiet {
            println!(
                "would open exact Create UART Base {} at {} and retain one non-actuating observation at {}",
                args.base_id,
                args.serial_path.display(),
                args.evidence_out.display()
            );
        }
        return Ok(());
    }

    let evidence = execute(&args)?;
    write_new_atomic(&args.evidence_out, &serde_json::to_vec_pretty(&evidence)?)?;
    if opts.json {
        println!("{}", serde_json::to_string_pretty(&evidence)?);
    } else if !opts.quiet {
        println!("evidence: {}", args.evidence_out.display());
        println!(
            "intended-robot-identity-verified: {}",
            evidence.intended_robot_identity_verified
        );
    }
    if let Outcome::Failed { stage, code, .. } = evidence.outcome {
        return Err(format!("std Create observation failed at {stage}: {code}").into());
    }
    Ok(())
}

fn execute(args: &StdObserveArgs) -> Result<Evidence, Box<dyn std::error::Error>> {
    let started = monotonic_millis().map_err(|error| format!("monotonic clock: {error:?}"))?;
    let deadline = started
        .checked_add(u64::from(args.read_timeout_ms))
        .ok_or("read deadline overflow")?;
    let mut base = BaseEvidence {
        base_id: args.base_id.clone(),
        serial_path: args.serial_path.to_string_lossy().into_owned(),
        device_number: None,
        baud: 57_600,
        data_bits: 8,
        stop_bits: 1,
        parity: "none",
    };
    let opened = StdCreateUartBase::open(StdCreateUartObservation {
        base_id: args.base_id.clone(),
        device_path: args.serial_path.clone(),
        profile: conduit_netherwick::UartProfile::CREATE_OI,
        maximum_write_wait_ms: MAXIMUM_CREATE_UART_WRITE_WAIT_MS,
    });
    let outcome = match opened {
        Err(error) => Outcome::Failed {
            stage: "base_open",
            code: open_error_code(&error),
            cleanup_code: None,
        },
        Ok(mut provider) => {
            base.device_number = Some(provider.identity().device_number);
            observe(&mut provider, deadline)
        }
    };
    let completed = monotonic_millis().map_err(|error| format!("monotonic clock: {error:?}"))?;
    Ok(Evidence {
        schema: EVIDENCE_SCHEMA,
        proof_class: "live_host_boundary_unverified_robot_identity",
        declared_host_id: args.host_id.clone(),
        declared_boot_id: args.boot_id.clone(),
        base,
        started_monotonic_ms: started,
        read_deadline_monotonic_ms: deadline,
        completed_monotonic_ms: completed,
        intended_robot_identity_verified: false,
        outcome,
    })
}

fn observe(provider: &mut StdCreateUartBase, deadline: u64) -> Outcome {
    let mut session = CreateObservationSession::new();
    if let Err(error) = session.start(provider) {
        return failed("session_start", error, None);
    }
    match session.read(provider, deadline) {
        Ok(observation) => match session.pause(provider) {
            Ok(()) => Outcome::Observed {
                observation: portable_observation(observation),
            },
            Err(error) => failed("session_pause", error, None),
        },
        Err(error) => {
            let cleanup = session.pause(provider).err().map(observation_error_code);
            failed("session_read", error, cleanup)
        }
    }
}

fn portable_observation(value: CreatePortableObservation) -> PortableObservation {
    let group = value.group_zero;
    let charging = group
        .charging
        .with_sources(value.charging_sources)
        .expect("session lowering already validated charging sources");
    let battery = group
        .charging
        .battery()
        .expect("session lowering already validated battery consistency");
    let (cliff_signal_available, cliff_signals) = group.cliff.signals();
    PortableObservation {
        contact_body_sectors: group.contact.active_body_sectors(),
        cliff_body_sectors: group.cliff.active_sectors(),
        cliff_signal_available,
        cliff_signals,
        dropped_wheels: group.wheel_drop.dropped_wheels(),
        proximity_body_sectors: group.proximity.active_body_sectors(),
        virtual_wall_present: group.virtual_wall.is_some(),
        infrared_code: group.infrared.map(|beacon| beacon.code),
        pressed_buttons: group.buttons.pressed(),
        charging_state: charging_state(charging.state),
        charging_sources: charging.sources,
        millivolts: charging.millivolts,
        milliamps: charging.milliamps,
        temperature_celsius: charging.temperature_celsius,
        charge_mah: charging.charge_mah,
        capacity_mah: charging.capacity_mah,
        battery_charge_permille: battery.map(|battery| battery.charge_permille()),
        distance_delta_mm: group.distance_delta_mm,
        angle_delta_degrees: group.angle_delta_degrees,
    }
}

fn failed(
    stage: &'static str,
    error: CreateObservationFailure,
    cleanup_code: Option<&'static str>,
) -> Outcome {
    Outcome::Failed {
        stage,
        code: observation_error_code(error),
        cleanup_code,
    }
}

fn observation_error_code(error: CreateObservationFailure) -> &'static str {
    use conduit_netherwick::CreateOiFailure as Protocol;
    match error {
        CreateObservationFailure::InvalidState => "invalid_session_state",
        CreateObservationFailure::Lowering(CreateSensorLoweringError::WrongPacket { .. }) => {
            "wrong_packet"
        }
        CreateObservationFailure::Lowering(CreateSensorLoweringError::Semantic(_)) => {
            "semantic_value_invalid"
        }
        CreateObservationFailure::Lowering(CreateSensorLoweringError::InconsistentCharge) => {
            "inconsistent_charge"
        }
        CreateObservationFailure::Protocol(Protocol::ProviderUnavailable) => "provider_unavailable",
        CreateObservationFailure::Protocol(Protocol::WrongUartProfile { .. }) => {
            "wrong_uart_profile"
        }
        CreateObservationFailure::Protocol(Protocol::WriteFailed) => "write_failed",
        CreateObservationFailure::Protocol(Protocol::ReadFailed) => "read_failed",
        CreateObservationFailure::Protocol(Protocol::Timeout) => "timeout",
        CreateObservationFailure::Protocol(Protocol::DeviceNoResponse) => "device_no_response",
        CreateObservationFailure::Protocol(Protocol::UnsupportedPacket(_)) => "unsupported_packet",
        CreateObservationFailure::Protocol(Protocol::TruncatedFrame) => "truncated_frame",
        CreateObservationFailure::Protocol(Protocol::MalformedFrame) => "malformed_frame",
    }
}

fn open_error_code(error: &StdCreateUartOpenError) -> &'static str {
    match error {
        StdCreateUartOpenError::MissingBaseIdentity => "missing_base_identity",
        StdCreateUartOpenError::WrongProfile(_) => "wrong_uart_profile",
        StdCreateUartOpenError::InvalidWriteWait => "invalid_write_wait",
        StdCreateUartOpenError::PathContainsNul => "path_contains_nul",
        StdCreateUartOpenError::Metadata(_) => "metadata_failed",
        StdCreateUartOpenError::NotCharacterDevice => "not_character_device",
        StdCreateUartOpenError::Open(_) => "open_failed",
        StdCreateUartOpenError::ObserveDeviceIdentity(_) => "device_identity_observation_failed",
        StdCreateUartOpenError::PathIdentityChanged => "path_identity_changed",
        StdCreateUartOpenError::ObserveTermios(_) => "termios_observation_failed",
        StdCreateUartOpenError::ConfigureTermios(_) => "termios_configuration_failed",
        StdCreateUartOpenError::VerifyTermios => "termios_verification_failed",
    }
}

fn charging_state(state: ChargingState) -> &'static str {
    match state {
        ChargingState::NotCharging => "not_charging",
        ChargingState::Reconditioning => "reconditioning",
        ChargingState::Full => "full",
        ChargingState::Trickle => "trickle",
        ChargingState::Waiting => "waiting",
        ChargingState::Fault => "fault",
    }
}

fn validate_args(args: &StdObserveArgs) -> Result<(), Box<dyn std::error::Error>> {
    for (name, value) in [
        ("base-id", args.base_id.as_str()),
        ("host-id", args.host_id.as_str()),
        ("boot-id", args.boot_id.as_str()),
    ] {
        if value.is_empty() || value.len() > MAXIMUM_ID_BYTES {
            return Err(format!("{name} must contain 1..={MAXIMUM_ID_BYTES} bytes").into());
        }
    }
    if args.serial_path.as_os_str().as_encoded_bytes().is_empty()
        || args.serial_path.as_os_str().as_encoded_bytes().len() > MAXIMUM_PATH_BYTES
        || args.evidence_out.as_os_str().as_encoded_bytes().is_empty()
        || args.evidence_out.as_os_str().as_encoded_bytes().len() > MAXIMUM_PATH_BYTES
    {
        return Err("serial and evidence paths must be nonempty and bounded".into());
    }
    if args.read_timeout_ms == 0 || args.read_timeout_ms > MAXIMUM_READ_TIMEOUT_MS {
        return Err(format!("read-timeout-ms must be 1..={MAXIMUM_READ_TIMEOUT_MS}").into());
    }
    Ok(())
}

fn write_new_atomic(path: &Path, bytes: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    if path.exists() {
        return Err(format!("evidence destination already exists: {}", path.display()).into());
    }
    let mut temporary = path.as_os_str().to_os_string();
    temporary.push(format!(".tmp-{}", std::process::id()));
    let temporary = PathBuf::from(temporary);
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        std::fs::hard_link(&temporary, path)?;
        Ok(())
    })();
    let _ = std::fs::remove_file(&temporary);
    result
}

#[cfg(test)]
#[path = "netherwick_std_observe_tests.rs"]
mod tests;
