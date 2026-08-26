use conduitos::{
    identity::BootIdentities,
    offer::{CpuFeatures, HostOffer},
    timing_profile::{
        admit, execute as execute_play, Injection, Refusal, TimingOffer, TimingOutcome,
        TimingRequirement, PROOF_CLASS,
    },
};
use serde::Serialize;

use crate::cli::GlobalOpts;

use super::ConduitosError;

#[derive(Serialize)]
struct TimingProof {
    schema: &'static str,
    proof_class: &'static str,
    profile: &'static str,
    deadline_us: u32,
    proven_worst_case_us: u32,
    met_elapsed_us: u32,
    refused_required_us: u32,
    refusal: &'static str,
    distinct_outcomes: [&'static str; 5],
    inspection: &'static str,
    physical_claim: bool,
}

pub fn execute(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    if opts.dry_run {
        println!("check exact local Form; seal timing/resource basis; execute production kernel with deterministic clock; verify refusal and distinct terminal outcomes");
        return Ok(());
    }
    let identities = BootIdentities {
        host: [7; 32],
        boot: [8; 32],
    };
    let host = HostOffer::new(
        &identities,
        "timing-proof",
        CpuFeatures {
            sse2: true,
            rdrand: true,
            invariant_tsc: true,
        },
        256 * 1024,
    );
    let timing = TimingOffer::deterministic(&host, 42);
    let mut accepted = admit(
        &identities,
        &host,
        timing,
        TimingRequirement { deadline_us: 1_000 },
        "timing-proof",
    )
    .map_err(refusal_error)?;
    let deadline = accepted.basis.deadline_us;
    let worst_case = accepted.basis.proven_worst_case_us;
    let TimingOutcome::DeadlineMet { elapsed_us } =
        execute_play(&mut accepted, timing, Injection::None).outcome
    else {
        return Err(ConduitosError::refusal(
            "deadline-met-proof-failed",
            "accepted deterministic Play did not meet its exact deadline",
        ));
    };
    let refused_required_us = match admit(
        &identities,
        &host,
        timing,
        TimingRequirement { deadline_us: 100 },
        "timing-proof",
    ) {
        Err(Refusal::Unschedulable { required_us, .. }) => required_us,
        _ => {
            return Err(ConduitosError::refusal(
                "unschedulable-refusal-proof-failed",
                "the impossible request was not refused during planning",
            ));
        }
    };
    verify_outcome(&identities, &host, timing, Injection::Overrun, |outcome| {
        matches!(outcome, TimingOutcome::DeadlineMiss { .. })
    })?;
    verify_outcome(
        &identities,
        &host,
        timing,
        Injection::TimerBaseLoss,
        |outcome| outcome == TimingOutcome::TimerBaseLoss,
    )?;
    verify_outcome(&identities, &host, timing, Injection::Cancel, |outcome| {
        outcome == TimingOutcome::Cancelled
    })?;
    let mut stale_plan = admit(
        &identities,
        &host,
        timing,
        TimingRequirement { deadline_us: 1_000 },
        "timing-proof",
    )
    .map_err(refusal_error)?;
    let mut changed_offer = timing;
    changed_offer.maximum_wake_latency_us += 1;
    if execute_play(&mut stale_plan, changed_offer, Injection::None).outcome
        != TimingOutcome::StaleTimingBasis
    {
        return Err(ConduitosError::refusal(
            "stale-timing-basis-proof-failed",
            "changed timing facts did not refuse before execution",
        ));
    }
    let report = TimingProof {
        schema: "conduit.timing/deterministic-proof@1",
        proof_class: PROOF_CLASS,
        profile: conduitos::timing_profile::TIMING_PROFILE,
        deadline_us: deadline,
        proven_worst_case_us: worst_case,
        met_elapsed_us: elapsed_us,
        refused_required_us,
        refusal: "unschedulable-at-plan-time",
        distinct_outcomes: [
            "deadline-met",
            "deadline-miss",
            "timer-base-loss",
            "cancelled",
            "stale-timing-basis",
        ],
        inspection: "excluded-from-strict-path",
        physical_claim: false,
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&report).map_err(|error| {
            ConduitosError::refusal("timing-proof-encoding-failed", error.to_string())
        })?
    );
    Ok(())
}

fn verify_outcome(
    identities: &BootIdentities,
    host: &HostOffer<'_>,
    timing: TimingOffer,
    injection: Injection,
    expected: impl FnOnce(TimingOutcome) -> bool,
) -> Result<(), ConduitosError> {
    let mut plan = admit(
        identities,
        host,
        timing,
        TimingRequirement { deadline_us: 1_000 },
        "timing-proof",
    )
    .map_err(refusal_error)?;
    let outcome = execute_play(&mut plan, timing, injection).outcome;
    if !expected(outcome) {
        return Err(ConduitosError::refusal(
            "timing-terminal-proof-failed",
            format!("unexpected outcome: {outcome:?}"),
        ));
    }
    Ok(())
}

fn refusal_error(error: Refusal) -> ConduitosError {
    ConduitosError::refusal("timing-plan-refused", format!("{error:?}"))
}
