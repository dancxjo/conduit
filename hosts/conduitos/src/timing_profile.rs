//! One exact, deterministic deadline-bounded local Plan profile.
//!
//! The authored requirement contains only semantic timing. Machine and Base
//! facts enter through [`TimingOffer`], are sealed into [`TimingBasis`] before
//! Play, and are revalidated at the execution boundary. Execution delegates to
//! the existing `conduit-kernel` scheduler installed by `timing_plan`.

use conduit_kernel::scheduler::{SchedulerError, SchedulerStatus};

use crate::{
    identity::BootIdentities,
    offer::HostOffer,
    planned_kernel::PlannedKernel,
    timing_plan::{self, PreparedTimingPlay},
};

pub const TIMING_PROFILE: &str = "conduit.timing/local-deadline@1";
pub const PROOF_CLASS: &str = "deterministic-emulator";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimingRequirement {
    pub deadline_us: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimingOffer {
    pub host_id: [u8; 32],
    pub boot_id: [u8; 32],
    pub generation: u64,
    pub clock_basis: u64,
    pub resolution_us: u32,
    pub maximum_clock_error_us: u32,
    pub maximum_wake_latency_us: u32,
    pub maximum_kernel_step_us: u32,
    pub maximum_timer_completion_us: u32,
    pub maximum_presentation_us: u32,
    pub kernel_step_budget: u16,
    pub arena_bytes: u32,
    pub cord_items: u16,
    pub cord_bytes: u32,
    pub wake_slots: u16,
    pub timer_slots: u16,
    pub base_scratch_bytes: u32,
    pub mandatory_sign_items: u16,
    pub mandatory_sign_bytes: u32,
    pub fault_reserve_bytes: u32,
}

impl TimingOffer {
    pub fn deterministic(host: &HostOffer<'_>, clock_basis: u64) -> Self {
        Self {
            host_id: host.host_id,
            boot_id: host.boot_id,
            generation: host.generation,
            clock_basis,
            resolution_us: 1,
            maximum_clock_error_us: 5,
            maximum_wake_latency_us: 100,
            maximum_kernel_step_us: 25,
            maximum_timer_completion_us: 25,
            maximum_presentation_us: 200,
            kernel_step_budget: 16,
            arena_bytes: u32::try_from(host.runtime_arena_bytes).unwrap_or(u32::MAX),
            cord_items: 1,
            cord_bytes: 8,
            wake_slots: 1,
            timer_slots: 1,
            base_scratch_bytes: 256,
            mandatory_sign_items: host.sign_item_capacity,
            mandatory_sign_bytes: 4_096,
            fault_reserve_bytes: 512,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimingBasis {
    pub profile: &'static str,
    pub proof_class: &'static str,
    pub host_id: [u8; 32],
    pub boot_id: [u8; 32],
    pub offer_generation: u64,
    pub clock_basis: u64,
    pub timing_offer_seal: u64,
    pub deadline_us: u32,
    pub proven_worst_case_us: u32,
    pub arena_bytes: u32,
    pub cord_items: u16,
    pub cord_bytes: u32,
    pub wake_slots: u16,
    pub timer_slots: u16,
    pub base_scratch_bytes: u32,
    pub mandatory_sign_items: u16,
    pub mandatory_sign_bytes: u32,
    pub fault_reserve_bytes: u32,
    pub inspection_included: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Refusal {
    InvalidRequirement,
    IncompleteOffer,
    ResourceCapacity,
    Unschedulable { required_us: u32, deadline_us: u32 },
    PlanPreparation,
}

pub struct AdmittedTimingPlan {
    pub prepared: PreparedTimingPlay,
    pub basis: TimingBasis,
}

pub fn admit(
    identities: &BootIdentities,
    host: &HostOffer<'_>,
    timing: TimingOffer,
    requirement: TimingRequirement,
    build_id: &str,
) -> Result<AdmittedTimingPlan, Refusal> {
    if requirement.deadline_us == 0 {
        return Err(Refusal::InvalidRequirement);
    }
    host.validate().map_err(|_| Refusal::IncompleteOffer)?;
    if timing.host_id != host.host_id
        || timing.boot_id != host.boot_id
        || timing.generation != host.generation
        || timing.clock_basis == 0
        || timing.resolution_us == 0
        || timing.kernel_step_budget == 0
    {
        return Err(Refusal::IncompleteOffer);
    }
    let prepared = timing_plan::prepare_timing(identities, host, build_id)
        .map_err(|_| Refusal::PlanPreparation)?;
    if timing.arena_bytes > host.runtime_arena_bytes as u32
        || timing.cord_items < 1
        || timing.cord_bytes < 8
        || timing.wake_slots < 1
        || timing.timer_slots < 1
        || timing.mandatory_sign_items < prepared.planned_sign_items
        || timing.mandatory_sign_bytes < prepared.planned_sign_bytes
        || timing.base_scratch_bytes == 0
        || timing.fault_reserve_bytes == 0
    {
        return Err(Refusal::ResourceCapacity);
    }
    let kernel_us = u32::from(timing.kernel_step_budget)
        .checked_mul(timing.maximum_kernel_step_us)
        .ok_or(Refusal::IncompleteOffer)?;
    let required_us = timing
        .maximum_clock_error_us
        .checked_add(timing.maximum_wake_latency_us)
        .and_then(|value| value.checked_add(timing.maximum_timer_completion_us))
        .and_then(|value| value.checked_add(kernel_us))
        .and_then(|value| value.checked_add(timing.maximum_presentation_us))
        .ok_or(Refusal::IncompleteOffer)?;
    if required_us > requirement.deadline_us {
        return Err(Refusal::Unschedulable {
            required_us,
            deadline_us: requirement.deadline_us,
        });
    }
    Ok(AdmittedTimingPlan {
        prepared,
        basis: TimingBasis {
            profile: TIMING_PROFILE,
            proof_class: PROOF_CLASS,
            host_id: timing.host_id,
            boot_id: timing.boot_id,
            offer_generation: timing.generation,
            clock_basis: timing.clock_basis,
            timing_offer_seal: seal_offer(timing),
            deadline_us: requirement.deadline_us,
            proven_worst_case_us: required_us,
            arena_bytes: timing.arena_bytes,
            cord_items: timing.cord_items,
            cord_bytes: timing.cord_bytes,
            wake_slots: timing.wake_slots,
            timer_slots: timing.timer_slots,
            base_scratch_bytes: timing.base_scratch_bytes,
            mandatory_sign_items: timing.mandatory_sign_items,
            mandatory_sign_bytes: timing.mandatory_sign_bytes,
            fault_reserve_bytes: timing.fault_reserve_bytes,
            inspection_included: false,
        },
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Injection {
    None,
    Overrun,
    TimerBaseLoss,
    Cancel,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimingOutcome {
    DeadlineMet { elapsed_us: u32 },
    DeadlineMiss { elapsed_us: u32, deadline_us: u32 },
    TimerBaseLoss,
    Cancelled,
    StaleTimingBasis,
    KernelFailure,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimingSign<'a> {
    pub plan_id: &'a str,
    pub active_play_id: &'a str,
    pub outcome: TimingOutcome,
}

pub fn execute<'a>(
    admitted: &'a mut AdmittedTimingPlan,
    current: TimingOffer,
    injection: Injection,
) -> TimingSign<'a> {
    let outcome = if admitted.basis.host_id != current.host_id
        || admitted.basis.boot_id != current.boot_id
        || admitted.basis.offer_generation != current.generation
        || admitted.basis.clock_basis != current.clock_basis
        || admitted.basis.timing_offer_seal != seal_offer(current)
    {
        TimingOutcome::StaleTimingBasis
    } else {
        run_kernel(
            &mut admitted.prepared.kernel,
            admitted.basis.deadline_us,
            current,
            injection,
        )
    };
    TimingSign {
        plan_id: admitted.prepared.plan_id.as_str(),
        active_play_id: admitted.prepared.active_play.active_play_id.as_str(),
        outcome,
    }
}

fn seal_offer(offer: TimingOffer) -> u64 {
    let mut seal = offer.clock_basis ^ offer.generation.rotate_left(7);
    for value in [
        u64::from(offer.resolution_us),
        u64::from(offer.maximum_clock_error_us),
        u64::from(offer.maximum_wake_latency_us),
        u64::from(offer.maximum_kernel_step_us),
        u64::from(offer.maximum_timer_completion_us),
        u64::from(offer.maximum_presentation_us),
        u64::from(offer.kernel_step_budget),
        u64::from(offer.arena_bytes),
        u64::from(offer.cord_items),
        u64::from(offer.cord_bytes),
        u64::from(offer.wake_slots),
        u64::from(offer.timer_slots),
        u64::from(offer.base_scratch_bytes),
        u64::from(offer.mandatory_sign_items),
        u64::from(offer.mandatory_sign_bytes),
        u64::from(offer.fault_reserve_bytes),
    ] {
        seal = seal.rotate_left(9) ^ value.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    }
    for byte in offer.host_id.into_iter().chain(offer.boot_id) {
        seal = seal.rotate_left(5) ^ u64::from(byte);
    }
    seal
}

fn run_kernel(
    kernel: &mut PlannedKernel,
    deadline_us: u32,
    offer: TimingOffer,
    injection: Injection,
) -> TimingOutcome {
    let mut elapsed = offer.maximum_clock_error_us;
    let mut steps = 0_u16;
    loop {
        if let Some(request) = kernel.next_host_request() {
            if request.node == crate::planned_kernel::TIMER_NODE {
                let Ok(interest) = PlannedKernel::timer_interest(request) else {
                    return TimingOutcome::KernelFailure;
                };
                if injection == Injection::Cancel {
                    return match kernel.cancel() {
                        Ok(()) => TimingOutcome::Cancelled,
                        Err(_) => TimingOutcome::KernelFailure,
                    };
                }
                if injection == Injection::TimerBaseLoss {
                    return TimingOutcome::TimerBaseLoss;
                }
                elapsed = elapsed
                    .saturating_add(offer.maximum_wake_latency_us)
                    .saturating_add(offer.maximum_timer_completion_us);
                if kernel.complete_timer(interest).is_err() {
                    return TimingOutcome::KernelFailure;
                }
            } else {
                elapsed = elapsed.saturating_add(offer.maximum_presentation_us);
                if kernel.complete_presentation(request).is_err() {
                    return TimingOutcome::KernelFailure;
                }
            }
        }
        steps = steps.saturating_add(1);
        if steps > offer.kernel_step_budget {
            return TimingOutcome::KernelFailure;
        }
        elapsed = elapsed.saturating_add(offer.maximum_kernel_step_us);
        match kernel.step() {
            Ok(SchedulerStatus::Complete) => {
                if injection == Injection::Overrun {
                    elapsed = deadline_us.saturating_add(1);
                }
                return if elapsed <= deadline_us {
                    TimingOutcome::DeadlineMet {
                        elapsed_us: elapsed,
                    }
                } else {
                    TimingOutcome::DeadlineMiss {
                        elapsed_us: elapsed,
                        deadline_us,
                    }
                };
            }
            Ok(SchedulerStatus::Progress { .. }) | Ok(SchedulerStatus::Idle) => {}
            Ok(SchedulerStatus::Cancelled) => return TimingOutcome::Cancelled,
            Err(SchedulerError::OperationFailed(_)) | Err(_) => {
                return TimingOutcome::KernelFailure;
            }
        }
    }
}
