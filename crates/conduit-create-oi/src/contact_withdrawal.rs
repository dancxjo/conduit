//! Mandatory bounded contact withdrawal below ordinary motion authority.
//!
//! This is intentionally not a Gear or recovery policy. A fresh contact edge
//! while a wheel is driving toward that contact may replace the host output
//! with one short straight reverse. Host or LINE loss cannot prolong or cancel
//! it; stronger local truth stops it immediately.

use crate::{encode_drive_direct, encode_stop, write_command, CreateOiFailure, CreateUartProvider};

pub const CONTACT_WITHDRAWAL_SPEED_MM_S: i16 = -80;
pub const CONTACT_WITHDRAWAL_MAXIMUM_TICKS: u64 = 250;
pub const CONTACT_WITHDRAWAL_MAXIMUM_DISTANCE_MM: u16 = 20;
pub const CONTACT_WITHDRAWAL_MAXIMUM_TRIGGER_LATENCY_TICKS: u64 = 10;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContactFrame {
    pub generation: u32,
    pub observed_at_tick: u64,
    pub maximum_age_ticks: u32,
    pub left: bool,
    pub right: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActiveWheelOutput {
    pub command_generation: u64,
    pub left_mm_s: i16,
    pub right_mm_s: i16,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WithdrawalInhibitors {
    pub explicitly_disarmed: bool,
    pub emergency_stop: bool,
    pub wheel_drop: bool,
    pub cliff: bool,
    pub charging: bool,
    pub tilt: bool,
    pub impact: bool,
    pub motor_feedback_invalid: bool,
    pub create_feedback_lost: bool,
    pub watchdog_failed: bool,
}

impl WithdrawalInhibitors {
    fn first(self) -> Option<WithdrawalPreemption> {
        [
            (self.emergency_stop, WithdrawalPreemption::EmergencyStop),
            (self.wheel_drop, WithdrawalPreemption::WheelDrop),
            (self.cliff, WithdrawalPreemption::Cliff),
            (self.charging, WithdrawalPreemption::Charging),
            (
                self.explicitly_disarmed,
                WithdrawalPreemption::ExplicitDisarm,
            ),
            (self.tilt, WithdrawalPreemption::Tilt),
            (self.impact, WithdrawalPreemption::Impact),
            (
                self.motor_feedback_invalid,
                WithdrawalPreemption::MotorFeedbackInvalid,
            ),
            (
                self.create_feedback_lost,
                WithdrawalPreemption::CreateFeedbackLost,
            ),
            (self.watchdog_failed, WithdrawalPreemption::WatchdogFailed),
        ]
        .into_iter()
        .find_map(|(active, cause)| active.then_some(cause))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContactSide {
    Left,
    Right,
    Bilateral,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WithdrawalPreemption {
    EmergencyStop,
    WheelDrop,
    Cliff,
    Charging,
    ExplicitDisarm,
    Tilt,
    Impact,
    MotorFeedbackInvalid,
    CreateFeedbackLost,
    WatchdogFailed,
    ObservationGenerationRegressed,
    ObservationClockInvalid,
    ObservationStale,
    ProviderFailure(CreateOiFailure),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContactWithdrawalSign {
    Started {
        contact_generation: u32,
        contact_side: ContactSide,
        preempted_command_generation: u64,
        started_at_tick: u64,
        deadline_tick: u64,
    },
    Completed {
        contact_generation: u32,
        contact_side: ContactSide,
        preempted_command_generation: u64,
        stopped_at_tick: u64,
        withdrawn_distance_mm: Option<u16>,
    },
    Preempted {
        contact_generation: u32,
        contact_side: ContactSide,
        preempted_command_generation: u64,
        stopped_at_tick: u64,
        cause: WithdrawalPreemption,
        stop_confirmed: bool,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ActiveWithdrawal {
    contact_generation: u32,
    contact_side: ContactSide,
    preempted_command_generation: u64,
    started_at_tick: u64,
    deadline_tick: u64,
    start_distance_mm: Option<i32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LocalContactWithdrawal {
    previous: Option<ContactFrame>,
    active: Option<ActiveWithdrawal>,
}

impl LocalContactWithdrawal {
    pub const fn new() -> Self {
        Self {
            previous: None,
            active: None,
        }
    }

    pub const fn is_active(&self) -> bool {
        self.active.is_some()
    }

    pub fn step<P: CreateUartProvider>(
        &mut self,
        provider: &mut P,
        now_tick: u64,
        frame: ContactFrame,
        active_host_output: Option<ActiveWheelOutput>,
        distance_mm: Option<i32>,
        inhibitors: WithdrawalInhibitors,
    ) -> Option<ContactWithdrawalSign> {
        if let Some(active) = self.active {
            return self.supervise_active(
                provider,
                now_tick,
                frame,
                distance_mm,
                inhibitors,
                active,
            );
        }

        let previous = self.previous.replace(frame)?;
        let invalid = validate_frame(previous, frame, now_tick);
        if invalid.is_some() || inhibitors.first().is_some() {
            return None;
        }
        let left_edge = !previous.left && frame.left;
        let right_edge = !previous.right && frame.right;
        let output = active_host_output.filter(|output| output.command_generation != 0)?;
        if !(left_edge && output.left_mm_s > 0 || right_edge && output.right_mm_s > 0) {
            return None;
        }
        let side = match (left_edge, right_edge) {
            (true, true) => ContactSide::Bilateral,
            (true, false) => ContactSide::Left,
            (false, true) => ContactSide::Right,
            (false, false) => return None,
        };
        let deadline_tick = now_tick.checked_add(CONTACT_WITHDRAWAL_MAXIMUM_TICKS)?;
        let active = ActiveWithdrawal {
            contact_generation: frame.generation,
            contact_side: side,
            preempted_command_generation: output.command_generation,
            started_at_tick: now_tick,
            deadline_tick,
            start_distance_mm: distance_mm,
        };
        if write_command(
            provider,
            &encode_drive_direct(CONTACT_WITHDRAWAL_SPEED_MM_S, CONTACT_WITHDRAWAL_SPEED_MM_S)
                .expect("fixed withdrawal speed fits Create OI"),
        )
        .is_err()
        {
            let stop_confirmed = write_command(provider, &encode_stop()).is_ok();
            return Some(ContactWithdrawalSign::Preempted {
                contact_generation: frame.generation,
                contact_side: side,
                preempted_command_generation: output.command_generation,
                stopped_at_tick: now_tick,
                cause: WithdrawalPreemption::ProviderFailure(CreateOiFailure::WriteFailed),
                stop_confirmed,
            });
        }
        self.active = Some(active);
        Some(ContactWithdrawalSign::Started {
            contact_generation: frame.generation,
            contact_side: side,
            preempted_command_generation: output.command_generation,
            started_at_tick: now_tick,
            deadline_tick,
        })
    }

    fn supervise_active<P: CreateUartProvider>(
        &mut self,
        provider: &mut P,
        now_tick: u64,
        frame: ContactFrame,
        distance_mm: Option<i32>,
        inhibitors: WithdrawalInhibitors,
        active: ActiveWithdrawal,
    ) -> Option<ContactWithdrawalSign> {
        let prior = self
            .previous
            .replace(frame)
            .expect("active withdrawal has baseline");
        let preemption = validate_frame(prior, frame, now_tick).or_else(|| inhibitors.first());
        let withdrawn = match (active.start_distance_mm, distance_mm) {
            (Some(start), Some(current)) => u16::try_from(start.saturating_sub(current)).ok(),
            _ => None,
        };
        let complete = now_tick >= active.deadline_tick
            || withdrawn.is_some_and(|distance| distance >= CONTACT_WITHDRAWAL_MAXIMUM_DISTANCE_MM);
        if preemption.is_none() && !complete {
            return None;
        }
        self.active = None;
        let stop = write_command(provider, &encode_stop());
        if let Some(cause) = preemption {
            return Some(ContactWithdrawalSign::Preempted {
                contact_generation: active.contact_generation,
                contact_side: active.contact_side,
                preempted_command_generation: active.preempted_command_generation,
                stopped_at_tick: now_tick,
                cause: stop
                    .err()
                    .map_or(cause, WithdrawalPreemption::ProviderFailure),
                stop_confirmed: stop.is_ok(),
            });
        }
        match stop {
            Ok(()) => Some(ContactWithdrawalSign::Completed {
                contact_generation: active.contact_generation,
                contact_side: active.contact_side,
                preempted_command_generation: active.preempted_command_generation,
                stopped_at_tick: now_tick,
                withdrawn_distance_mm: withdrawn,
            }),
            Err(failure) => Some(ContactWithdrawalSign::Preempted {
                contact_generation: active.contact_generation,
                contact_side: active.contact_side,
                preempted_command_generation: active.preempted_command_generation,
                stopped_at_tick: now_tick,
                cause: WithdrawalPreemption::ProviderFailure(failure),
                stop_confirmed: false,
            }),
        }
    }
}

impl Default for LocalContactWithdrawal {
    fn default() -> Self {
        Self::new()
    }
}

fn validate_frame(
    previous: ContactFrame,
    current: ContactFrame,
    now_tick: u64,
) -> Option<WithdrawalPreemption> {
    if current.generation <= previous.generation {
        Some(WithdrawalPreemption::ObservationGenerationRegressed)
    } else if current.observed_at_tick > now_tick {
        Some(WithdrawalPreemption::ObservationClockInvalid)
    } else if now_tick.saturating_sub(current.observed_at_tick)
        > u64::from(current.maximum_age_ticks)
        || now_tick.saturating_sub(current.observed_at_tick)
            > CONTACT_WITHDRAWAL_MAXIMUM_TRIGGER_LATENCY_TICKS
    {
        Some(WithdrawalPreemption::ObservationStale)
    } else {
        None
    }
}

#[cfg(test)]
#[path = "contact_withdrawal_tests.rs"]
mod tests;
