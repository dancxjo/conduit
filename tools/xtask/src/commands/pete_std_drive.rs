//! Bounded physical std/Create drive proof with an explicit reduced safety class.

use std::path::PathBuf;
use std::time::Duration;

use clap::{Args, ValueEnum};
use conduit_core::{BootId, HostId, OfferGeneration, Scalar};
use conduit_create_oi::{
    IndependentWatchdogObservation, LocalSafetyEnvelope, MotionAuthority, MotionSafetyAuthority,
    SafetyInputObservation, SafetyInputs, SafetyObservation, UartProfile,
};
use conduit_pete::{
    bounded_drive_plan, dispatch_create_drive_execution, prepare_create_drive_execution,
    supervise_create_drive_execution, CreateDriveExecutionRefusal, CreateDriveExecutionReport,
    CreateDriveExecutionTerminal, CreateDriveObservation, CreateObservationSession,
    CreatePortableObservation, DriveRefusal, SafeDispositionCause, BOUNDED_DRIVE_FORM,
    BOUNDED_DRIVE_GRANT, CREATE_DRIVE_IMPLEMENTATION, CREATE_DRIVE_REDUCED_SAFETY_AUTHORITY,
    CREATE_DRIVE_REDUCED_SAFETY_PROFILE,
};
use conduit_robotics::ChargingState;
use conduit_std_host::std_create_uart::{
    monotonic_millis, StdCreateUartBase, StdCreateUartObservation,
    MAXIMUM_CREATE_UART_WRITE_WAIT_MS,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::cli::GlobalOpts;
use crate::commands::pete_std_create::{establish_safe, write_new_atomic};

const EVIDENCE_SCHEMA: &str = "conduit.pete/std-create-drive-evidence@1";
const MAXIMUM_ID_BYTES: usize = 128;
const MAXIMUM_PATH_BYTES: usize = 4_096;
const MAXIMUM_READ_TIMEOUT_MS: u32 = 5_000;
const CREATE_MAXIMUM_WHEEL_SPEED_MM_S: i64 = 500;
const SAFETY_MAXIMUM_AGE_MS: u32 = 1_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum MotionEnvironment {
    WheelsOffFloor,
    Floor,
}

#[derive(Args, Clone, Debug)]
pub struct StdDriveArgs {
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
    /// Physical proof environment. Wheels-off-floor is the safe default.
    #[arg(long, value_enum, default_value_t = MotionEnvironment::WheelsOffFloor)]
    motion_environment: MotionEnvironment,
    /// Assert that the Create is securely supported with every drive wheel off the floor.
    #[arg(long)]
    confirm_wheels_off_floor: bool,
    /// Exact token emitted by a refused floor attempt for this Plan and attachment.
    #[arg(long)]
    reduced_safety_floor_ack: Option<String>,
    /// Deadline for each bounded correlated Create observation.
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
    host_id: String,
    boot_id: String,
    robot_id: String,
    robot_identity_verified: bool,
    serial_base_id: String,
    serial_path: String,
    serial_device_number: u64,
    safety_profile: &'static str,
    independent_watchdog: &'static str,
    unavailable_auxiliary_inputs: [&'static str; 3],
    motion_environment: MotionEnvironment,
    required_floor_ack: Option<String>,
    supplied_floor_ack_matched: bool,
    authority_contract: &'static str,
    authority_grant_id: &'static str,
    implementation: &'static str,
    navigation: NavigationEvidence,
    request: MotionRequestEvidence,
    pre_observation: ObservationEvidence,
    missing_authority_refusal: Option<DriveReportEvidence>,
    dispatch: Option<DriveReportEvidence>,
    terminal: Option<DriveReportEvidence>,
    post_observation: Option<ObservationEvidence>,
    outcome: Outcome,
}

struct EvidenceContext {
    pre_observation: ObservationEvidence,
    navigation: NavigationEvidence,
    linear: Scalar,
    angular: Scalar,
    required_floor_ack: Option<String>,
    supplied_floor_ack_matched: bool,
}

#[derive(Serialize)]
struct MotionRequestEvidence {
    linear_microunits: i64,
    angular_microunits: i64,
    ttl_ms: u32,
}

#[derive(Serialize)]
struct NavigationEvidence {
    route_identity: String,
    goal_identity: String,
    pose_source_identity: String,
    clock_identity: String,
    linear_mm: i32,
    angular_microdegrees: i32,
    interval_ms: u32,
    ttl_ms: u32,
}

#[derive(Clone, Copy, Serialize)]
struct ObservationEvidence {
    observed_at_monotonic_ms: u64,
    contact_body_sectors: u8,
    cliff_body_sectors: u8,
    dropped_wheels: u8,
    charging_sources: u8,
    charging_state: &'static str,
    distance_delta_mm: i16,
    angle_delta_degrees: i16,
}

#[derive(Serialize)]
struct DriveReportEvidence {
    kernel_decisions: u32,
    kernel_signs: u16,
    terminal: String,
}

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum Outcome {
    Completed,
    Refused { stage: &'static str, code: String },
    Failed { stage: &'static str, code: String },
}

pub fn run(args: StdDriveArgs, opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    validate(&args)?;
    if opts.dry_run {
        if !opts.quiet {
            println!(
                "would observe exact Create safety truth, derive one bounded motion intent through portable navigation, refuse it without authority, then request one attended reduced-safety drive over {}",
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
        if let Some(token) = &evidence.required_floor_ack {
            println!("required-floor-ack: {token}");
        }
    }
    match evidence.outcome {
        Outcome::Completed => Ok(()),
        Outcome::Refused { stage, ref code } => {
            Err(format!("std Create drive refused at {stage}: {code}").into())
        }
        Outcome::Failed { stage, ref code } => {
            Err(format!("std Create drive failed at {stage}: {code}").into())
        }
    }
}

fn execute(args: &StdDriveArgs) -> Result<Evidence, Box<dyn std::error::Error>> {
    let mut provider = StdCreateUartBase::open(StdCreateUartObservation {
        base_id: args.base_id.clone(),
        device_path: args.serial_path.clone(),
        profile: UartProfile::CREATE_OI,
        maximum_write_wait_ms: MAXIMUM_CREATE_UART_WRITE_WAIT_MS,
    })
    .map_err(|error| format!("base open: {error:?}"))?;
    let base = provider.identity().clone();
    let mode = establish_safe(&mut provider, args.read_timeout_ms)?;
    let (pre, pre_at) = observe(&mut provider, args.read_timeout_ms)?;
    let safety = safety_from(pre, pre_at)?;
    let drive_observation = CreateDriveObservation {
        host_id: HostId::from(args.host_id.clone()),
        boot_id: BootId::from(args.boot_id.clone()),
        offer_generation: OfferGeneration(1),
        serial_base_id: args.base_id.clone(),
        robot_identity: args.robot_id.clone(),
        drive_resource_id: format!("{}/drive", args.robot_id),
        mode,
        safety,
    };
    let (navigation, linear, angular) = navigation_intent(pre_at, &args.robot_id)?;
    let plan = bounded_drive_plan(&drive_observation, true)
        .map_err(|error| format!("drive planning: {error:?}"))?;
    let required_ack = floor_ack_token(args, plan.plan_id.as_str());
    let ack_matches = args.motion_environment == MotionEnvironment::WheelsOffFloor
        || args.reduced_safety_floor_ack.as_deref() == Some(required_ack.as_str());
    let mut evidence = base_evidence(
        args,
        &base,
        plan.plan_id.as_str(),
        EvidenceContext {
            pre_observation: observation_evidence(pre, pre_at),
            navigation,
            linear,
            angular,
            required_floor_ack: (args.motion_environment == MotionEnvironment::Floor)
                .then_some(required_ack),
            supplied_floor_ack_matched: ack_matches,
        },
    );
    if !ack_matches {
        evidence.outcome = Outcome::Refused {
            stage: "reduced_safety_authority",
            code: "exact_floor_ack_required".into(),
        };
        return Ok(evidence);
    }

    let mut refused_execution =
        prepare_create_drive_execution(&plan, &drive_observation, linear, angular)
            .map_err(|error| format!("drive preparation: {error}"))?;
    let now = monotonic_millis().map_err(|error| format!("monotonic clock: {error:?}"))?;
    let refused = dispatch_create_drive_execution(
        &mut refused_execution,
        &mut provider,
        now,
        None,
        SafetyObservation {
            observed_at_tick: now,
            ..safety
        },
    );
    if refused.terminal
        != CreateDriveExecutionTerminal::Refused(CreateDriveExecutionRefusal::Drive(
            DriveRefusal::MissingAuthority,
        ))
    {
        evidence.outcome = Outcome::Failed {
            stage: "missing_authority_preflight",
            code: format!("{:?}", refused.terminal),
        };
        return Ok(evidence);
    }
    evidence.missing_authority_refusal = Some(report_evidence(&refused));
    let mut execution = prepare_create_drive_execution(&plan, &drive_observation, linear, angular)
        .map_err(|error| format!("authorized drive preparation: {error}"))?;
    let safety_class = match args.motion_environment {
        MotionEnvironment::WheelsOffFloor => MotionSafetyAuthority::ReducedWheelsOffFloor,
        MotionEnvironment::Floor => MotionSafetyAuthority::ReducedFloorAcknowledged,
    };
    let dispatched = dispatch_create_drive_execution(
        &mut execution,
        &mut provider,
        now,
        Some(MotionAuthority {
            grant_id: BOUNDED_DRIVE_GRANT,
            valid_until_tick: now.saturating_add(1_000),
            safety_class,
        }),
        SafetyObservation {
            observed_at_tick: now,
            ..safety
        },
    );
    evidence.dispatch = Some(report_evidence(&dispatched));
    let deadline = match dispatched.terminal {
        CreateDriveExecutionTerminal::MotionAdmitted { deadline_tick, .. } => deadline_tick,
        ref terminal => {
            evidence.outcome = Outcome::Refused {
                stage: "kernel_motion_dispatch",
                code: format!("{terminal:?}"),
            };
            return Ok(evidence);
        }
    };
    sleep_until(deadline)?;
    let terminal = supervise_create_drive_execution(
        &mut execution,
        &mut provider,
        monotonic_millis().map_err(|error| format!("monotonic clock: {error:?}"))?,
        SafetyObservation {
            observed_at_tick: now,
            ..safety
        },
    );
    evidence.terminal = Some(report_evidence(&terminal));
    if !matches!(
        terminal.terminal,
        CreateDriveExecutionTerminal::SafeDisposition {
            cause: SafeDispositionCause::DeadlineExpired,
            ..
        }
    ) {
        evidence.outcome = Outcome::Failed {
            stage: "terminal_safe_disposition",
            code: format!("{:?}", terminal.terminal),
        };
        return Ok(evidence);
    }
    let (post, post_at) = observe(&mut provider, args.read_timeout_ms)?;
    evidence.post_observation = Some(observation_evidence(post, post_at));
    evidence.outcome = Outcome::Completed;
    Ok(evidence)
}

fn observe(
    provider: &mut StdCreateUartBase,
    timeout_ms: u32,
) -> Result<(CreatePortableObservation, u64), Box<dyn std::error::Error>> {
    let mut session = CreateObservationSession::new();
    session
        .start(provider)
        .map_err(|error| format!("observation start: {error:?}"))?;
    let now = monotonic_millis().map_err(|error| format!("monotonic clock: {error:?}"))?;
    let deadline = now
        .checked_add(u64::from(timeout_ms))
        .ok_or("read deadline overflow")?;
    let observed = match session.read(provider, deadline) {
        Ok(value) => value,
        Err(error) => {
            let cleanup = session.pause(provider).err();
            return Err(format!("observation read: {error:?}; cleanup={cleanup:?}").into());
        }
    };
    session
        .pause(provider)
        .map_err(|error| format!("observation pause: {error:?}"))?;
    let observed_at = monotonic_millis().map_err(|error| format!("monotonic clock: {error:?}"))?;
    Ok((observed, observed_at))
}

fn safety_from(
    value: CreatePortableObservation,
    observed_at_tick: u64,
) -> Result<SafetyObservation, Box<dyn std::error::Error>> {
    let group = value.group_zero;
    let inputs = SafetyInputs {
        generation: 1,
        observed_at_tick,
        maximum_age_ticks: SAFETY_MAXIMUM_AGE_MS,
        emergency_stop: SafetyInputObservation::Unavailable,
        wheel_drop: group.wheel_drop.dropped_wheels() != 0,
        cliff: group.cliff.active_sectors() != 0,
        contact: group.contact.active_body_sectors() != 0,
        tilt: SafetyInputObservation::Unavailable,
        impact: SafetyInputObservation::Unavailable,
        charging: value.charging_sources.bits() != 0
            || group.charging.state != ChargingState::NotCharging,
        control_alive: true,
        body_link_alive: true,
        independent_watchdog: IndependentWatchdogObservation::Absent,
    };
    let mut envelope = LocalSafetyEnvelope::new();
    envelope
        .observe(inputs, observed_at_tick)
        .map_err(|error| format!("local safety envelope: {error:?}"))?;
    envelope
        .snapshot()
        .ok_or_else(|| "local safety envelope emitted no observation".into())
}

fn navigation_intent(
    now_ms: u64,
    robot_id: &str,
) -> Result<(NavigationEvidence, Scalar, Scalar), Box<dyn std::error::Error>> {
    use conduit_semantic_catalog::{
        local_control, time_parameterize, ControlDecision, NavigationPose, NavigationRoute,
        NavigationTime, Validity, Waypoint,
    };

    let clock_identity = "pete/create-monotonic-ms".to_owned();
    let pose = NavigationPose {
        source_identity: format!("{robot_id}/odometry"),
        sample_sequence: 1,
        clock_identity: clock_identity.clone(),
        frame: "pete/start-local".into(),
        x_mm: 0,
        y_mm: 0,
        heading_microdegrees: 0,
        validity: Validity {
            observed_at_ms: now_ms,
            valid_until_ms: now_ms.saturating_add(1_000),
        },
    };
    let route = NavigationRoute {
        identity: format!("{robot_id}/reviewed-forward-route"),
        goal_identity: format!("{robot_id}/reviewed-forward-goal"),
        planner_identity: "conduit/navigation/reviewed-straight-route@1".into(),
        planning_input_identity: format!("{robot_id}/attended-action-proposal"),
        frame: pose.frame.clone(),
        target_heading_microdegrees: 0,
        position_tolerance_mm: 1,
        heading_tolerance_microdegrees: 1_000_000,
        waypoints: vec![
            Waypoint { x_mm: 0, y_mm: 0 },
            Waypoint { x_mm: 12, y_mm: 0 },
        ],
    };
    let time = NavigationTime {
        clock_identity: clock_identity.clone(),
        now_ms,
    };
    let trajectory = time_parameterize(&route, &time, 250, 12, 1_000_000)
        .map_err(|error| format!("navigation time parameterization: {error:?}"))?;
    let ControlDecision::Motion(intent) = local_control(&pose, &trajectory, &time)
        .map_err(|error| format!("navigation local control: {error:?}"))?
    else {
        return Err("reviewed navigation did not produce a bounded motion intent".into());
    };
    if intent.ttl_ms != 250 || intent.interval_ms != 250 || intent.angular_microdegrees != 0 {
        return Err("reviewed straight navigation intent violated the drive proof shape".into());
    }
    let linear_raw = i64::from(intent.linear_mm)
        .checked_mul(1_000)
        .and_then(|value| value.checked_mul(Scalar::SCALE))
        .and_then(|value| {
            value.checked_div(i64::from(intent.interval_ms) * CREATE_MAXIMUM_WHEEL_SPEED_MM_S)
        })
        .ok_or("navigation velocity normalization overflow")?;
    let evidence = NavigationEvidence {
        route_identity: route.identity,
        goal_identity: route.goal_identity,
        pose_source_identity: pose.source_identity,
        clock_identity,
        linear_mm: intent.linear_mm,
        angular_microdegrees: intent.angular_microdegrees,
        interval_ms: intent.interval_ms,
        ttl_ms: intent.ttl_ms,
    };
    Ok((
        evidence,
        Scalar::from_raw_microunits(linear_raw),
        Scalar::ZERO,
    ))
}

fn base_evidence(
    args: &StdDriveArgs,
    base: &conduit_std_host::std_create_uart::StdCreateUartIdentity,
    plan_id: &str,
    context: EvidenceContext,
) -> Evidence {
    Evidence {
        schema: EVIDENCE_SCHEMA,
        proof_class: "live_std_create_reduced_safety_motion_machine_evidence",
        portable_form: BOUNDED_DRIVE_FORM,
        plan_id: plan_id.into(),
        host_id: args.host_id.clone(),
        boot_id: args.boot_id.clone(),
        robot_id: args.robot_id.clone(),
        robot_identity_verified: args.attest_robot_identity,
        serial_base_id: base.base_id.clone(),
        serial_path: base.device_path.to_string_lossy().into_owned(),
        serial_device_number: base.device_number,
        safety_profile: CREATE_DRIVE_REDUCED_SAFETY_PROFILE,
        independent_watchdog: "absent",
        unavailable_auxiliary_inputs: ["emergency_stop", "tilt", "impact"],
        motion_environment: args.motion_environment,
        required_floor_ack: context.required_floor_ack,
        supplied_floor_ack_matched: context.supplied_floor_ack_matched,
        authority_contract: CREATE_DRIVE_REDUCED_SAFETY_AUTHORITY,
        authority_grant_id: BOUNDED_DRIVE_GRANT,
        implementation: CREATE_DRIVE_IMPLEMENTATION,
        navigation: context.navigation,
        request: MotionRequestEvidence {
            linear_microunits: context.linear.raw_microunits(),
            angular_microunits: context.angular.raw_microunits(),
            ttl_ms: 250,
        },
        pre_observation: context.pre_observation,
        missing_authority_refusal: None,
        dispatch: None,
        terminal: None,
        post_observation: None,
        outcome: Outcome::Failed {
            stage: "internal",
            code: "incomplete_evidence".into(),
        },
    }
}

fn observation_evidence(value: CreatePortableObservation, observed_at: u64) -> ObservationEvidence {
    let group = value.group_zero;
    ObservationEvidence {
        observed_at_monotonic_ms: observed_at,
        contact_body_sectors: group.contact.active_body_sectors(),
        cliff_body_sectors: group.cliff.active_sectors(),
        dropped_wheels: group.wheel_drop.dropped_wheels(),
        charging_sources: value.charging_sources.bits(),
        charging_state: charging_state(group.charging.state),
        distance_delta_mm: group.distance_delta_mm,
        angle_delta_degrees: group.angle_delta_degrees,
    }
}

fn report_evidence(report: &CreateDriveExecutionReport) -> DriveReportEvidence {
    DriveReportEvidence {
        kernel_decisions: report.kernel_decisions,
        kernel_signs: report.kernel_signs,
        terminal: format!("{:?}", report.terminal),
    }
}

fn floor_ack_token(args: &StdDriveArgs, plan_id: &str) -> String {
    let mut hash = Sha256::new();
    for value in [
        "conduit.pete/std-reduced-floor-ack@1",
        args.host_id.as_str(),
        args.boot_id.as_str(),
        args.base_id.as_str(),
        args.robot_id.as_str(),
        plan_id,
        BOUNDED_DRIVE_GRANT,
    ] {
        hash.update((value.len() as u64).to_be_bytes());
        hash.update(value.as_bytes());
    }
    format!("std-floor-motion:{:x}", hash.finalize())
}

fn sleep_until(deadline: u64) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        let now = monotonic_millis().map_err(|error| format!("monotonic clock: {error:?}"))?;
        if now >= deadline {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis((deadline - now).min(10)));
    }
}

fn charging_state(value: ChargingState) -> &'static str {
    match value {
        ChargingState::NotCharging => "not_charging",
        ChargingState::Reconditioning => "reconditioning",
        ChargingState::Full => "full",
        ChargingState::Trickle => "trickle",
        ChargingState::Waiting => "waiting",
        ChargingState::Fault => "fault",
    }
}

fn validate(args: &StdDriveArgs) -> Result<(), Box<dyn std::error::Error>> {
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
    if args.motion_environment == MotionEnvironment::WheelsOffFloor
        && !args.confirm_wheels_off_floor
    {
        return Err(
            "--confirm-wheels-off-floor is required before the default motion proof".into(),
        );
    }
    if args.motion_environment == MotionEnvironment::Floor && args.confirm_wheels_off_floor {
        return Err("floor and wheels-off-floor attestations are mutually exclusive".into());
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
#[path = "pete_std_drive_tests.rs"]
mod tests;
