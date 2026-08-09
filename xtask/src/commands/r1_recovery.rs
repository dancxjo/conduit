//! Deterministic R1 new-Plan recovery proof with an explicitly injected Line-loss Clue.

use conduit_core::{
    ActivePlayId, BootId, ClueId, ConnectionBase, ControlLoopEvent, GearId, HostId,
    LinkAvailability, LinkObservation, PlanId,
};
use conduit_system_continuity::{
    exact_r1_signal_plan, R1LedResultClue, R1NewPlanRecovery, R1RecoveryStartClues,
    R1ReplacementClues, R1SignalRouteSet,
};
use serde::Serialize;

use crate::{cli::GlobalOpts, process::StepError, proof::ProofClass};

const STEP_ID: &str = "prove.r1-new-plan-recovery";
const SIMULATED_PICO_BOOT_ID: &str = "r1/simulated-pico-boot";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct SoftwareRecoveryOutcome {
    proof_class: ProofClass,
    fault_injection: &'static str,
    body_id: conduit_body::BodyId,
    wake_id: conduit_body::WakeId,
    plan_a_id: PlanId,
    play_a_id: ActivePlayId,
    plan_a_line: ConnectionBase,
    plan_b_id: PlanId,
    play_b_id: ActivePlayId,
    plan_b_line: ConnectionBase,
    control_events: Vec<ControlLoopEvent>,
    led_result: R1LedResultClue,
    physical_acceptance: bool,
}

pub fn run(opts: &GlobalOpts) -> Result<(), StepError> {
    if opts.dry_run {
        if !opts.quiet {
            println!(
                "==> R1 deterministic recovery would inject an exact WebSocket Line-unavailable Clue"
            );
            println!("    proof class: deterministic-simulation; physical acceptance: false");
        }
        return Ok(());
    }
    let outcome = verify().map_err(|error| StepError::prereq(STEP_ID, error))?;
    if opts.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&outcome)
                .map_err(|error| StepError::prereq(STEP_ID, error.to_string()))?
        );
    } else if !opts.quiet {
        println!("==> R1 typed new-Plan recovery verified with deterministic fault injection");
        println!("    Body: {}", outcome.body_id.as_str());
        println!("    Wake: {}", outcome.wake_id.as_str());
        println!(
            "    Plan A / Play A / Line: {} / {} / {:?}",
            outcome.plan_a_id.as_str(),
            outcome.play_a_id.as_str(),
            outcome.plan_a_line
        );
        println!(
            "    Plan B / Play B / Line: {} / {} / {:?}",
            outcome.plan_b_id.as_str(),
            outcome.play_b_id.as_str(),
            outcome.plan_b_line
        );
        println!("    physical acceptance: false");
    }
    Ok(())
}

fn verify() -> Result<SoftwareRecoveryOutcome, String> {
    let pico_boot = BootId::from(SIMULATED_PICO_BOOT_ID);
    let plan_a = exact_r1_signal_plan(pico_boot.clone(), R1SignalRouteSet::WebSocketOnly)?.plan;
    let plan_b = exact_r1_signal_plan(pico_boot.clone(), R1SignalRouteSet::UsbOnly)?.plan;
    let mut recovery = R1NewPlanRecovery::begin(
        plan_a,
        GearId::from("show"),
        1,
        1,
        HostId::from(conduit_net::R1_STD_HOST_ID),
        BootId::from(conduit_net::R1_STD_BOOT_ID),
        0,
        R1RecoveryStartClues {
            birth: ClueId::from("r1/body-born"),
            wake: ClueId::from("r1/body-woke"),
            plan_ready: ClueId::from("r1/plan-a-ready"),
            play_started: ClueId::from("r1/play-a-started"),
        },
    )
    .map_err(|error| format!("failed beginning recovery: {error:?}"))?;
    let body_id = recovery.body().body_id.clone();
    let wake_id = recovery.wake().wake_id.clone();
    let plan_a_id = recovery.plan_a().plan_id.clone();
    let play_a_id = recovery.play_a().active_play_id.clone();
    let plan_a_line = recovery
        .plan_a_session_binding()
        .map_err(|error| format!("invalid Plan A session: {error:?}"))?
        .attachment
        .base;

    recovery
        .observe_route_unavailable(
            LinkObservation {
                binding_id: conduit_core::LinkBindingId::from(
                    conduit_net::R1_WEBSOCKET_LINK_BINDING_ID,
                ),
                availability: LinkAvailability::Unavailable,
                clue_id: ClueId::from("r1/injected-websocket-line-unavailable"),
            },
            ClueId::from("r1/play-a-unsatisfied"),
        )
        .map_err(|error| format!("failed injecting WebSocket Line loss: {error:?}"))?;
    recovery
        .install_replacement(
            plan_b,
            HostId::from(conduit_net::R1_STD_HOST_ID),
            BootId::from(conduit_net::R1_STD_BOOT_ID),
            HostId::from(conduit_net::R1_STD_HOST_ID),
            BootId::from(conduit_net::R1_STD_BOOT_ID),
            1,
            R1ReplacementClues {
                request: ClueId::from("r1/replan-requested"),
                planned: ClueId::from("r1/plan-b-planned"),
                superseded: ClueId::from("r1/plan-a-superseded"),
                realized: ClueId::from("r1/plan-b-realized"),
                play_started: ClueId::from("r1/play-b-started"),
            },
        )
        .map_err(|error| format!("failed installing replacement: {error:?}"))?;
    let plan_b_id = recovery.plan_b().unwrap().plan_id.clone();
    let play_b_id = recovery.play_b().unwrap().active_play_id.clone();
    let plan_b_line = recovery
        .plan_b_session_binding()
        .map_err(|error| format!("invalid Plan B session: {error:?}"))?
        .attachment
        .base;
    recovery
        .record_led_result(
            HostId::from(conduit_net::R1_PICO_HOST_ID),
            pico_boot,
            plan_b_id.clone(),
            play_b_id.clone(),
            ClueId::from("r1/simulated-plan-b-led-result"),
            true,
        )
        .map_err(|error| format!("failed recording simulated LED result: {error:?}"))?;

    if body_id != recovery.body().body_id
        || wake_id != recovery.wake().wake_id
        || plan_a_id == plan_b_id
        || play_a_id == play_b_id
        || plan_a_line != ConnectionBase::WebSocket
        || plan_b_line != ConnectionBase::UsbCdc
    {
        return Err("recovery identity or Line invariant mismatched".into());
    }
    Ok(SoftwareRecoveryOutcome {
        proof_class: ProofClass::DeterministicSimulation,
        fault_injection: "typed-websocket-line-unavailable-clue",
        body_id,
        wake_id,
        plan_a_id,
        play_a_id,
        plan_a_line,
        plan_b_id,
        play_b_id,
        plan_b_line,
        control_events: recovery.events().to_vec(),
        led_result: recovery.led_results()[0].clone(),
        physical_acceptance: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_runner_is_typed_and_cannot_claim_physical_acceptance() {
        let outcome = verify().unwrap();
        assert_eq!(outcome.proof_class, ProofClass::DeterministicSimulation);
        assert!(!outcome.physical_acceptance);
        assert_eq!(outcome.control_events.len(), 6);
        assert_eq!(outcome.led_result.plan_id, outcome.plan_b_id);
        assert_eq!(outcome.led_result.active_play_id, outcome.play_b_id);
    }
}
