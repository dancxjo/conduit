//! Bounded std UART physical entrance for Create indicator presentation.

use std::io::Write;
use std::path::PathBuf;

use clap::Args;
use conduit_core::{BootId, HostId, OfferGeneration};
use conduit_create_oi::{CreateUartProvider, UartProfile};
use conduit_pete::{
    create_indicator_plan, encode_indicator, indicator_authority_admits,
    live_indicator_advertisement, CreateIndicatorObservation, CREATE_INDICATOR_FORM,
    INDICATOR_IMPLEMENTATION,
};
use conduit_std_host::external_signal::SignalManifestation;
use conduit_std_host::std_create_uart::{
    StdCreateUartBase, StdCreateUartObservation, MAXIMUM_CREATE_UART_WRITE_WAIT_MS,
};
use conduit_std_host::{RunControl, StdHost, ThreadTimer};
use serde::Serialize;

use crate::cli::GlobalOpts;
use crate::commands::pete_std_create::{establish_safe, write_new_atomic};

const EVIDENCE_SCHEMA: &str = "conduit.pete/std-create-indicator-evidence@1";
const MAXIMUM_ID_BYTES: usize = 128;
const MAXIMUM_PATH_BYTES: usize = 4_096;
const MAXIMUM_READ_TIMEOUT_MS: u32 = 5_000;

#[derive(Args, Clone, Debug)]
pub struct StdIndicatorArgs {
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
    /// Deadline for the exact OI mode response.
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
    active_play_id: Option<String>,
    declared_host_id: String,
    declared_boot_id: String,
    robot_identity: RobotIdentityEvidence,
    base: BaseEvidence,
    observed_oi_mode: &'static str,
    indicator_implementation: &'static str,
    presentation_authority_granted: bool,
    motion_authority_granted: bool,
    receipts: Vec<SignalReceiptEvidence>,
    kernel_decisions: u32,
    kernel_signs: u16,
    indicator_commands: u16,
    final_off: FinalOffEvidence,
    operator_visibility: &'static str,
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
struct SignalReceiptEvidence {
    sequence: u64,
    level: bool,
}

#[derive(Serialize)]
struct FinalOffEvidence {
    attempted: bool,
    committed: bool,
}

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum Outcome {
    Completed,
    Failed { stage: &'static str, code: String },
}

pub fn run(args: StdIndicatorArgs, opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    validate(&args)?;
    if opts.dry_run {
        if !opts.quiet {
            println!(
                "would attest robot {}, establish SAFE OI over {}, and manifest canonical Signal on its indicator",
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
        println!("operator-visibility: {}", evidence.operator_visibility);
    }
    if let Outcome::Failed { stage, ref code } = evidence.outcome {
        return Err(format!("std Create indicator failed at {stage}: {code}").into());
    }
    Ok(())
}

fn execute(args: &StdIndicatorArgs) -> Result<Evidence, Box<dyn std::error::Error>> {
    let mut provider = StdCreateUartBase::open(StdCreateUartObservation {
        base_id: args.base_id.clone(),
        device_path: args.serial_path.clone(),
        profile: UartProfile::CREATE_OI,
        maximum_write_wait_ms: MAXIMUM_CREATE_UART_WRITE_WAIT_MS,
    })
    .map_err(|error| format!("base open: {error:?}"))?;
    let identity = provider.identity().clone();
    let mode = establish_safe(&mut provider, args.read_timeout_ms)?;
    let observation = CreateIndicatorObservation {
        host_id: HostId::from(args.host_id.clone()),
        boot_id: BootId::from(args.boot_id.clone()),
        offer_generation: OfferGeneration(1),
        serial_base_id: args.base_id.clone(),
        robot_identity: args.robot_id.clone(),
        robot_identity_verified: args.attest_robot_identity,
        indicator_resource_id: format!("{}/power-indicator", args.robot_id),
        timer_resource_id: format!("{}/indicator-timer", args.host_id),
        mode,
        currently_usable: true,
    };
    let advertisement = live_indicator_advertisement(&observation)
        .map_err(|error| format!("indicator advertisement: {error:?}"))?;
    let plan = create_indicator_plan(&observation, true)
        .map_err(|error| format!("indicator planning: {error:?}"))?;
    let fragment = plan
        .fragments
        .iter()
        .find(|fragment| fragment.host_id == observation.host_id)
        .cloned()
        .ok_or("indicator Plan has no exact local fragment")?;
    let mut host = StdHost::from_advertisement(advertisement)
        .map_err(|error| format!("indicator Host: {error}"))?;
    let mut output = Vec::new();
    let mut manifestation = CreateIndicatorManifestation {
        provider: &mut provider,
        last_sequence: None,
        commands: 0,
    };
    let report = host.run_signal_fragment_with_manifestation(
        fragment,
        &mut output,
        &mut ThreadTimer,
        &RunControl::default(),
        &mut manifestation,
    );
    let commands_before_cleanup = manifestation.commands;
    let final_off = manifestation.write_level(false).is_ok();
    let (active_play_id, receipts, kernel_decisions, kernel_signs, outcome) = match report {
        Ok(report) if final_off => {
            let kernel = report
                .kernel
                .as_ref()
                .ok_or("Signal run omitted kernel evidence")?;
            (
                Some(kernel.active_play_id.as_str().to_string()),
                report
                    .receipts
                    .iter()
                    .map(|receipt| SignalReceiptEvidence {
                        sequence: receipt.sequence,
                        level: receipt.level,
                    })
                    .collect(),
                kernel.decisions,
                kernel.kernel_events,
                Outcome::Completed,
            )
        }
        Ok(report) => {
            let kernel = report
                .kernel
                .as_ref()
                .ok_or("Signal run omitted kernel evidence")?;
            (
                Some(kernel.active_play_id.as_str().to_string()),
                report
                    .receipts
                    .iter()
                    .map(|receipt| SignalReceiptEvidence {
                        sequence: receipt.sequence,
                        level: receipt.level,
                    })
                    .collect(),
                kernel.decisions,
                kernel.kernel_events,
                Outcome::Failed {
                    stage: "final_off_cleanup",
                    code: "provider_write_failed".into(),
                },
            )
        }
        Err(error) => (
            None,
            Vec::new(),
            0,
            0,
            Outcome::Failed {
                stage: "kernel_signal_play",
                code: error,
            },
        ),
    };
    Ok(Evidence {
        schema: EVIDENCE_SCHEMA,
        proof_class: "live_std_create_indicator_machine_evidence",
        portable_form: CREATE_INDICATOR_FORM,
        plan_id: plan.plan_id.as_str().to_string(),
        active_play_id,
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
        observed_oi_mode: "safe",
        indicator_implementation: INDICATOR_IMPLEMENTATION,
        presentation_authority_granted: true,
        motion_authority_granted: false,
        receipts,
        kernel_decisions,
        kernel_signs,
        indicator_commands: commands_before_cleanup,
        final_off: FinalOffEvidence {
            attempted: true,
            committed: final_off,
        },
        operator_visibility: "pending_operator_confirmation",
        outcome,
    })
}

struct CreateIndicatorManifestation<'a> {
    provider: &'a mut StdCreateUartBase,
    last_sequence: Option<u64>,
    commands: u16,
}

impl CreateIndicatorManifestation<'_> {
    fn write_level(&mut self, level: bool) -> Result<(), String> {
        let command = encode_indicator(level);
        if !indicator_authority_admits(&command) {
            return Err("indicator authority refused its sealed command".into());
        }
        self.provider
            .write_all(&command)
            .map_err(|error| format!("indicator provider: {error:?}"))?;
        self.commands = self
            .commands
            .checked_add(1)
            .ok_or_else(|| "indicator command count exhausted".to_string())?;
        Ok(())
    }
}

impl SignalManifestation for CreateIndicatorManifestation<'_> {
    fn manifest(
        &mut self,
        signal: &conduit_signal::Signal,
        operator_output: &mut dyn Write,
    ) -> Result<(), String> {
        let expected = self
            .last_sequence
            .map_or(0, |value| value.saturating_add(1));
        if signal.sequence != expected {
            return Err(format!(
                "non-monotonic Signal sequence: expected {expected}, received {}",
                signal.sequence
            ));
        }
        self.write_level(signal.level)?;
        self.last_sequence = Some(signal.sequence);
        writeln!(
            operator_output,
            "create-indicator sequence={} level={} committed=true",
            signal.sequence, signal.level
        )
        .map_err(|error| error.to_string())
    }
}

fn validate(args: &StdIndicatorArgs) -> Result<(), Box<dyn std::error::Error>> {
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
#[path = "pete_std_indicator_tests.rs"]
mod tests;
