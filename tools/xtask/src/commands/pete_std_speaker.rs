//! Bounded std UART physical entrance for the existing Create speaker Plan.

use std::path::PathBuf;
use std::time::Duration;

use clap::Args;
use conduit_core::{BootId, HostId, OfferGeneration};
use conduit_create_oi::{
    encode_query_sensor, read_query_sensor_packet, write_command, CreateOiFailure,
    CreateUartProvider, UartProfile,
};
use conduit_pete::{
    encode_song, prepare_speaker_execution, run_speaker_execution, simple_melody_plan,
    speaker_authority_admits, CreateSpeakerObservation, CreateSpeakerSerial, OiPitch, OiSongEvent,
    SerialFailure, SpeakerPlayReport, SpeakerTerminal, DURATION_TICKS_PER_SECOND,
    MAXIMUM_ADMITTED_SERIAL_BYTES, SIMPLE_MELODY_FORM, SPEAKER_AUTHORITY, SPEAKER_IMPLEMENTATION,
};
use conduit_std_host::std_create_uart::{
    monotonic_millis, StdCreateUartBase, StdCreateUartObservation,
    MAXIMUM_CREATE_UART_WRITE_WAIT_MS,
};
use serde::Serialize;

use crate::cli::GlobalOpts;
use crate::commands::pete_std_create::{establish_full, establish_safe, write_new_atomic};

const EVIDENCE_SCHEMA: &str = "conduit.pete/std-create-speaker-evidence@1";
const MAXIMUM_ID_BYTES: usize = 128;
const MAXIMUM_PATH_BYTES: usize = 4_096;
const MAXIMUM_READ_TIMEOUT_MS: u32 = 5_000;
const SONG_NUMBER_PACKET: u8 = 36;
const SONG_PLAYING_PACKET: u8 = 37;

#[derive(Args, Clone, Debug)]
pub struct StdSpeakerArgs {
    /// Exact Linux character-device path; no discovery is performed.
    #[arg(long)]
    serial_path: PathBuf,
    /// Caller-declared stable identity for this exact serial Base.
    #[arg(long)]
    base_id: String,
    /// Caller-declared Host identity retained as provenance.
    #[arg(long)]
    host_id: String,
    /// Caller-declared Boot identity retained as provenance.
    #[arg(long)]
    boot_id: String,
    /// Exact current physical robot identity asserted by the operator.
    #[arg(long)]
    robot_id: String,
    /// Assert that robot-id identifies the currently attached physical robot.
    #[arg(long)]
    attest_robot_identity: bool,
    /// Deadline for each exact OI sensor response.
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
    portable_form: &'static str,
    plan_id: String,
    declared_host_id: String,
    declared_boot_id: String,
    robot_identity: RobotIdentityEvidence,
    base: BaseEvidence,
    observed_oi_mode: &'static str,
    final_oi_mode: &'static str,
    safe_cleanup_completed: bool,
    speaker_authority: &'static str,
    speaker_implementation: &'static str,
    motion_authority_granted: bool,
    kernel_decisions: u32,
    kernel_signs: u16,
    define_bytes: u16,
    play_bytes: u16,
    maximum_song_ticks: u16,
    post_bound_song_playing: Option<bool>,
    audibility: &'static str,
    outcome: Outcome,
}

#[derive(Serialize)]
struct RobotIdentityEvidence {
    id: String,
    basis: &'static str,
    verified: bool,
}

#[derive(Serialize)]
struct BaseEvidence {
    id: String,
    path: String,
    device_number: u64,
    baud: u32,
    data_bits: u8,
    stop_bits: u8,
    parity: &'static str,
}

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum Outcome {
    Completed,
    Failed { stage: &'static str, code: String },
}

pub fn run(args: StdSpeakerArgs, opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    validate(&args)?;
    if opts.dry_run {
        if !opts.quiet {
            println!(
                "would attest robot {}, establish FULL OI over {}, play one bounded portable melody, and verify SAFE cleanup",
                args.robot_id,
                args.serial_path.display()
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
        println!("audibility: {}", evidence.audibility);
    }
    if let Outcome::Failed { stage, ref code } = evidence.outcome {
        return Err(format!("std Create speaker failed at {stage}: {code}").into());
    }
    Ok(())
}

fn execute(args: &StdSpeakerArgs) -> Result<Evidence, Box<dyn std::error::Error>> {
    let mut provider = StdCreateUartBase::open(StdCreateUartObservation {
        base_id: args.base_id.clone(),
        device_path: args.serial_path.clone(),
        profile: UartProfile::CREATE_OI,
        maximum_write_wait_ms: MAXIMUM_CREATE_UART_WRITE_WAIT_MS,
    })
    .map_err(|error| format!("base open: {error:?}"))?;
    let identity = provider.identity().clone();
    // Pete's physical Create 1 retains a queued song without sounding it in
    // SAFE. FULL is therefore an exact mechanism requirement of this speaker
    // implementation, while the admitted operation remains speaker-only.
    let mode = match establish_full(&mut provider, args.read_timeout_ms) {
        Ok(mode) => mode,
        Err(mode_error) => {
            // A failed observation does not prove that the preceding FULL
            // command was ignored. Restore and observe SAFE before returning
            // the acquisition failure so an unreadable response cannot leave
            // the physical robot in Full mode silently.
            return match establish_safe(&mut provider, args.read_timeout_ms) {
                Ok(_) => Err(format!(
                    "speaker mode acquisition failed: {mode_error}; Safe cleanup completed"
                )
                .into()),
                Err(cleanup_error) => Err(format!(
                    "speaker mode acquisition failed: {mode_error}; Safe cleanup failed: {cleanup_error}"
                )
                .into()),
            };
        }
    };
    let observation = CreateSpeakerObservation {
        host_id: HostId::from(args.host_id.clone()),
        boot_id: BootId::from(args.boot_id.clone()),
        offer_generation: OfferGeneration(1),
        serial_base_id: args.base_id.clone(),
        robot_identity: args.robot_id.clone(),
        robot_identity_verified: args.attest_robot_identity,
        speaker_resource_id: format!("{}/speaker", args.robot_id),
        mode,
        currently_usable: true,
    };
    let plan = simple_melody_plan(&observation, true)
        .map_err(|error| format!("speaker planning: {error:?}"))?;
    let song = encode_song(
        2,
        &[
            OiSongEvent {
                pitch: OiPitch::Note(60),
                duration_ticks: 32,
            },
            OiSongEvent {
                pitch: OiPitch::Note(64),
                duration_ticks: 32,
            },
            OiSongEvent {
                pitch: OiPitch::Rest,
                duration_ticks: 12,
            },
            OiSongEvent {
                pitch: OiPitch::Note(67),
                duration_ticks: 40,
            },
        ],
        MAXIMUM_ADMITTED_SERIAL_BYTES,
    )
    .expect("fixed melody is within the pinned Create 1 profile");
    let mut execution = prepare_speaker_execution(&plan, &song)
        .map_err(|error| format!("speaker preparation: {error}"))?;
    let (report, mut outcome, post_bound) = {
        let mut speaker = StdSpeakerSerial {
            provider: &mut provider,
            read_timeout_ms: args.read_timeout_ms,
        };
        let report = run_speaker_execution(&mut execution, &mut speaker);
        let mut outcome = terminal_outcome(report);
        let mut post_bound = None;
        if matches!(report.terminal, SpeakerTerminal::Completed) {
            let millis = u64::from(song.maximum_completion_ticks)
                .saturating_mul(1_000)
                .div_ceil(u64::from(DURATION_TICKS_PER_SECOND))
                .saturating_add(100);
            std::thread::sleep(Duration::from_millis(millis));
            let still_playing = speaker
                .query_boolean(SONG_PLAYING_PACKET)
                .map_err(|error| format!("post-bound song observation: {error:?}"))?;
            post_bound = Some(still_playing);
            if still_playing {
                outcome = Outcome::Failed {
                    stage: "post_bound_cleanup",
                    code: "song_still_playing".into(),
                };
            }
        }
        (report, outcome, post_bound)
    };
    // FULL is required to sound Pete's physical Create 1 speaker, but it must
    // never outlive this narrow operation. Restore and observe SAFE even when
    // dispatch, observation, or post-bound cleanup failed.
    let safe_cleanup = establish_safe(&mut provider, args.read_timeout_ms);
    if let Err(ref error) = safe_cleanup {
        outcome = Outcome::Failed {
            stage: "restore_safe",
            code: error.clone(),
        };
    }
    Ok(Evidence {
        schema: EVIDENCE_SCHEMA,
        proof_class: "live_std_create_speaker_machine_evidence",
        portable_form: SIMPLE_MELODY_FORM,
        plan_id: plan.plan_id.as_str().to_string(),
        declared_host_id: args.host_id.clone(),
        declared_boot_id: args.boot_id.clone(),
        robot_identity: RobotIdentityEvidence {
            id: args.robot_id.clone(),
            basis: "operator_attested_current_attachment",
            verified: args.attest_robot_identity,
        },
        base: BaseEvidence {
            id: identity.base_id,
            path: identity.device_path.to_string_lossy().into_owned(),
            device_number: identity.device_number,
            baud: identity.profile.baud,
            data_bits: identity.profile.data_bits,
            stop_bits: identity.profile.stop_bits,
            parity: "none",
        },
        observed_oi_mode: "full",
        final_oi_mode: if safe_cleanup.is_ok() {
            "safe"
        } else {
            "unknown"
        },
        safe_cleanup_completed: safe_cleanup.is_ok(),
        speaker_authority: SPEAKER_AUTHORITY,
        speaker_implementation: SPEAKER_IMPLEMENTATION,
        motion_authority_granted: false,
        kernel_decisions: report.kernel_decisions,
        kernel_signs: report.kernel_signs,
        define_bytes: report.define_bytes,
        play_bytes: report.play_bytes,
        maximum_song_ticks: song.maximum_completion_ticks,
        post_bound_song_playing: post_bound,
        audibility: "pending_operator_confirmation",
        outcome,
    })
}

struct StdSpeakerSerial<'a> {
    provider: &'a mut StdCreateUartBase,
    read_timeout_ms: u32,
}

impl StdSpeakerSerial<'_> {
    fn query_byte(&mut self, packet: u8) -> Result<u8, SerialFailure> {
        let command = encode_query_sensor(packet).map_err(|_| SerialFailure::Refused)?;
        write_command(self.provider, &command).map_err(map_serial_failure)?;
        let deadline = monotonic_millis()
            .map_err(|_| SerialFailure::ProviderLost)?
            .checked_add(u64::from(self.read_timeout_ms))
            .ok_or(SerialFailure::Refused)?;
        let response = read_query_sensor_packet(self.provider, packet, deadline)
            .map_err(map_serial_failure)?;
        response
            .bytes()
            .first()
            .copied()
            .ok_or(SerialFailure::Refused)
    }

    fn query_boolean(&mut self, packet: u8) -> Result<bool, SerialFailure> {
        match self.query_byte(packet)? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(SerialFailure::Refused),
        }
    }
}

impl CreateSpeakerSerial for StdSpeakerSerial<'_> {
    fn write_exact(&mut self, bytes: &[u8]) -> Result<(), SerialFailure> {
        if !speaker_authority_admits(bytes) {
            return Err(SerialFailure::Refused);
        }
        self.provider
            .write_all(bytes)
            .map_err(|_| SerialFailure::ProviderLost)
    }

    fn observe_song_playing(&mut self, song_number: u8) -> Result<bool, SerialFailure> {
        if self.query_byte(SONG_NUMBER_PACKET)? != song_number {
            return Ok(false);
        }
        self.query_boolean(SONG_PLAYING_PACKET)
    }
}

fn terminal_outcome(report: SpeakerPlayReport) -> Outcome {
    match report.terminal {
        SpeakerTerminal::Completed => Outcome::Completed,
        terminal => Outcome::Failed {
            stage: "kernel_speaker_play",
            code: format!("{terminal:?}"),
        },
    }
}

fn map_serial_failure(error: CreateOiFailure) -> SerialFailure {
    match error {
        CreateOiFailure::ProviderUnavailable
        | CreateOiFailure::WriteFailed
        | CreateOiFailure::ReadFailed => SerialFailure::ProviderLost,
        CreateOiFailure::Timeout | CreateOiFailure::DeviceNoResponse => {
            SerialFailure::DeviceNoResponse
        }
        CreateOiFailure::TruncatedFrame => SerialFailure::TruncatedResponse,
        CreateOiFailure::MalformedFrame | CreateOiFailure::SynchronizationLimit { .. } => {
            SerialFailure::MalformedResponse
        }
        _ => SerialFailure::Refused,
    }
}

fn validate(args: &StdSpeakerArgs) -> Result<(), Box<dyn std::error::Error>> {
    for (name, value) in [
        ("base-id", args.base_id.as_str()),
        ("host-id", args.host_id.as_str()),
        ("boot-id", args.boot_id.as_str()),
        ("robot-id", args.robot_id.as_str()),
    ] {
        if value.is_empty() || value.len() > MAXIMUM_ID_BYTES {
            return Err(format!("{name} must contain 1..={MAXIMUM_ID_BYTES} bytes").into());
        }
    }
    if !args.attest_robot_identity {
        return Err(
            "--attest-robot-identity is required; UART presence is not robot identity".into(),
        );
    }
    for path in [&args.serial_path, &args.evidence_out] {
        let len = path.as_os_str().as_encoded_bytes().len();
        if len == 0 || len > MAXIMUM_PATH_BYTES {
            return Err("serial and evidence paths must be nonempty and bounded".into());
        }
    }
    if args.read_timeout_ms == 0 || args.read_timeout_ms > MAXIMUM_READ_TIMEOUT_MS {
        return Err(format!("read-timeout-ms must be 1..={MAXIMUM_READ_TIMEOUT_MS}").into());
    }
    Ok(())
}

#[cfg(test)]
#[path = "pete_std_speaker_tests.rs"]
mod tests;
