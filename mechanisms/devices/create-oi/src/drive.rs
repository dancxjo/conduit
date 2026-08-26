//! Non-bypassable local safety boundary for the Create drive realization.
//!
//! A physical drive offer must dispatch through this type. It accepts portable
//! body velocity meaning only after authority and fresh local safety truth are
//! present, lowers to Create OI wheel commands, and owns the finite stop on
//! expiry or hazard. It is not an author-wirable safety Gear.

use crate::{
    encode_drive_direct, encode_stop, write_command, ActiveWheelOutput, ContactFrame,
    ContactWithdrawalSign, CreateOiFailure, CreateUartProvider, IndependentWatchdogObservation,
    LocalContactWithdrawal, LocalHazard, SafetyInputObservation, SafetyObservation,
    WithdrawalInhibitors, CREATE_OI_MAX_WHEEL_SPEED_MM_S,
};

/// The exact portable-profile lower TTL bound accepted by this realization.
pub const MINIMUM_MOTION_TTL_MS: u32 = 10;
/// The exact portable-profile upper TTL bound accepted by this realization.
pub const MAXIMUM_MOTION_TTL_MS: u32 = 60_000;
/// The admitted local motion clock is exactly one monotonic tick per millisecond.
pub const MOTION_CLOCK_TICKS_PER_SECOND: u32 = 1_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MotionAuthority<'a> {
    pub grant_id: &'a str,
    pub valid_until_tick: u64,
    pub safety_class: MotionSafetyAuthority,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MotionSafetyAuthority {
    IndependentWatchdog,
    ReducedWheelsOffFloor,
    ReducedFloorAcknowledged,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DifferentialMotionRequest {
    pub left_mm_s: i16,
    pub right_mm_s: i16,
    pub ttl_ms: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DriveRefusal {
    MissingAuthority,
    AuthorityExpired,
    SafetyAuthorityMismatch,
    SafetyStaleOrInhibited(LocalHazard),
    InvalidTtl,
    VelocityOutsideRealization,
    DeadlineOverflow,
    CommandGenerationExhausted,
    Device(CreateOiFailure),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DriveSafetySign<'a> {
    MotionAdmitted {
        authority_grant_id: &'a str,
        safety_generation: u32,
        deadline_tick: u64,
    },
    SafeDisposition {
        cause: SafeDispositionCause,
        safety_generation: u32,
    },
    Refused(DriveRefusal),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SafeDispositionCause {
    RequestedStop,
    DeadlineExpired,
    Hazard(LocalHazard),
    AuthorityExpired,
    ProviderFailure(CreateOiFailure),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreateActuatorSupervisionSign<'a> {
    Drive(DriveSafetySign<'a>),
    ContactWithdrawal(ContactWithdrawalSign),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhysicalActuatorObservation {
    pub safety: SafetyObservation,
    pub contact: ContactFrame,
    pub distance_mm: Option<i32>,
    pub explicitly_disarmed: bool,
    pub motor_feedback_invalid: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LocalCreateDriveSafety {
    motion_deadline_tick: Option<u64>,
    active_authority_until_tick: Option<u64>,
    safety_generation: u32,
    command_generation: u64,
    active_output: Option<ActiveWheelOutput>,
    contact_withdrawal: LocalContactWithdrawal,
}

impl LocalCreateDriveSafety {
    pub const fn new() -> Self {
        Self {
            motion_deadline_tick: None,
            active_authority_until_tick: None,
            safety_generation: 0,
            command_generation: 0,
            active_output: None,
            contact_withdrawal: LocalContactWithdrawal::new(),
        }
    }

    pub fn admit_motion<'a, P: CreateUartProvider>(
        &mut self,
        provider: &mut P,
        now_tick: u64,
        authority: Option<MotionAuthority<'a>>,
        safety: SafetyObservation,
        request: DifferentialMotionRequest,
    ) -> DriveSafetySign<'a> {
        let result = self.try_admit_motion(provider, now_tick, authority, safety, request);
        match result {
            Ok((authority_grant_id, deadline_tick)) => DriveSafetySign::MotionAdmitted {
                authority_grant_id,
                safety_generation: safety.generation,
                deadline_tick,
            },
            Err(refusal) => DriveSafetySign::Refused(refusal),
        }
    }

    fn try_admit_motion<'a, P: CreateUartProvider>(
        &mut self,
        provider: &mut P,
        now_tick: u64,
        authority: Option<MotionAuthority<'a>>,
        safety: SafetyObservation,
        request: DifferentialMotionRequest,
    ) -> Result<(&'a str, u64), DriveRefusal> {
        let authority = authority.ok_or(DriveRefusal::MissingAuthority)?;
        if authority.grant_id.is_empty() {
            return Err(DriveRefusal::MissingAuthority);
        }
        if authority.valid_until_tick <= now_tick {
            return Err(DriveRefusal::AuthorityExpired);
        }
        if let Some(hazard) = safety.first_hazard(now_tick) {
            return Err(DriveRefusal::SafetyStaleOrInhibited(hazard));
        }
        match (
            safety.has_complete_independent_envelope(),
            authority.safety_class,
        ) {
            (true, MotionSafetyAuthority::IndependentWatchdog)
            | (
                false,
                MotionSafetyAuthority::ReducedWheelsOffFloor
                | MotionSafetyAuthority::ReducedFloorAcknowledged,
            ) => {}
            _ => return Err(DriveRefusal::SafetyAuthorityMismatch),
        }
        if !(MINIMUM_MOTION_TTL_MS..=MAXIMUM_MOTION_TTL_MS).contains(&request.ttl_ms) {
            return Err(DriveRefusal::InvalidTtl);
        }
        if request.left_mm_s.unsigned_abs() > CREATE_OI_MAX_WHEEL_SPEED_MM_S as u16
            || request.right_mm_s.unsigned_abs() > CREATE_OI_MAX_WHEEL_SPEED_MM_S as u16
        {
            return Err(DriveRefusal::VelocityOutsideRealization);
        }
        let ttl_ticks = u64::from(request.ttl_ms)
            .checked_mul(u64::from(MOTION_CLOCK_TICKS_PER_SECOND))
            .and_then(|ticks| ticks.checked_div(1_000))
            .ok_or(DriveRefusal::DeadlineOverflow)?;
        let deadline_tick = now_tick
            .checked_add(ttl_ticks)
            .ok_or(DriveRefusal::DeadlineOverflow)?;
        if deadline_tick > authority.valid_until_tick {
            return Err(DriveRefusal::AuthorityExpired);
        }
        let command = encode_drive_direct(request.left_mm_s, request.right_mm_s)
            .map_err(|_| DriveRefusal::VelocityOutsideRealization)?;
        let command_generation = self
            .command_generation
            .checked_add(1)
            .ok_or(DriveRefusal::CommandGenerationExhausted)?;
        write_command(provider, &command).map_err(DriveRefusal::Device)?;
        self.command_generation = command_generation;
        self.active_output = Some(ActiveWheelOutput {
            command_generation,
            left_mm_s: request.left_mm_s,
            right_mm_s: request.right_mm_s,
        });
        self.motion_deadline_tick = Some(deadline_tick);
        self.active_authority_until_tick = Some(authority.valid_until_tick);
        self.safety_generation = safety.generation;
        Ok((authority.grant_id, deadline_tick))
    }

    pub fn supervise<P: CreateUartProvider>(
        &mut self,
        provider: &mut P,
        now_tick: u64,
        safety: SafetyObservation,
    ) -> Option<DriveSafetySign<'static>> {
        let cause = if safety.generation < self.safety_generation {
            Some(SafeDispositionCause::Hazard(
                LocalHazard::SafetyGenerationRegressed,
            ))
        } else if let Some(hazard) = safety.first_hazard(now_tick) {
            Some(SafeDispositionCause::Hazard(hazard))
        } else if self
            .active_authority_until_tick
            .is_some_and(|deadline| now_tick >= deadline)
        {
            Some(SafeDispositionCause::AuthorityExpired)
        } else if self
            .motion_deadline_tick
            .is_some_and(|deadline| now_tick >= deadline)
        {
            Some(SafeDispositionCause::DeadlineExpired)
        } else {
            None
        }?;
        Some(self.stop_with_cause(provider, cause, safety.generation))
    }

    /// Supervises the physical actuator with the mandatory contact reflex.
    ///
    /// Embedded realizations call this before ordinary hazard supervision so a
    /// fresh contact edge can atomically replace toward-contact output. Once
    /// started, ordinary control loss cannot cancel the reflex; exact stronger
    /// local truth still preempts it.
    pub fn supervise_physical<P: CreateUartProvider>(
        &mut self,
        provider: &mut P,
        now_tick: u64,
        observation: PhysicalActuatorObservation,
    ) -> Option<CreateActuatorSupervisionSign<'static>> {
        let PhysicalActuatorObservation {
            safety,
            contact,
            distance_mm,
            explicitly_disarmed,
            motor_feedback_invalid,
        } = observation;
        let inhibitors = WithdrawalInhibitors {
            explicitly_disarmed,
            emergency_stop: safety.emergency_stop == SafetyInputObservation::Active
                || safety.latched_hazards.contains(LocalHazard::EmergencyStop),
            wheel_drop: safety.wheel_drop
                || safety.latched_hazards.contains(LocalHazard::WheelDrop),
            cliff: safety.cliff || safety.latched_hazards.contains(LocalHazard::Cliff),
            charging: safety.charging || safety.latched_hazards.contains(LocalHazard::Charging),
            tilt: safety.tilt == SafetyInputObservation::Active
                || safety.latched_hazards.contains(LocalHazard::Tilt),
            impact: safety.impact == SafetyInputObservation::Active
                || safety.latched_hazards.contains(LocalHazard::Impact),
            motor_feedback_invalid,
            create_feedback_lost: !safety.body_link_alive
                || safety.latched_hazards.contains(LocalHazard::BodyLinkLost),
            watchdog_failed: safety.independent_watchdog == IndependentWatchdogObservation::Failed
                || safety
                    .latched_hazards
                    .contains(LocalHazard::WatchdogUnhealthy),
        };
        if let Some(sign) = self.contact_withdrawal.step(
            provider,
            now_tick,
            contact,
            self.active_output,
            distance_mm,
            inhibitors,
        ) {
            self.motion_deadline_tick = None;
            self.active_authority_until_tick = None;
            self.active_output = None;
            return Some(CreateActuatorSupervisionSign::ContactWithdrawal(sign));
        }
        if self.contact_withdrawal.is_active() {
            return None;
        }
        self.supervise(provider, now_tick, safety)
            .map(CreateActuatorSupervisionSign::Drive)
    }

    pub fn stop<P: CreateUartProvider>(
        &mut self,
        provider: &mut P,
        safety_generation: u32,
    ) -> DriveSafetySign<'static> {
        self.stop_with_cause(
            provider,
            SafeDispositionCause::RequestedStop,
            safety_generation,
        )
    }

    fn stop_with_cause<P: CreateUartProvider>(
        &mut self,
        provider: &mut P,
        cause: SafeDispositionCause,
        safety_generation: u32,
    ) -> DriveSafetySign<'static> {
        self.motion_deadline_tick = None;
        self.active_authority_until_tick = None;
        self.active_output = None;
        // An explicit outer stop, provider-loss disposition, or authority
        // stop supersedes the finite contact action too. Keep one zero write
        // and prevent a stale withdrawal from surviving into a later command.
        self.contact_withdrawal.clear_after_outer_stop();
        self.safety_generation = safety_generation;
        match write_command(provider, &encode_stop()) {
            Ok(()) => DriveSafetySign::SafeDisposition {
                cause,
                safety_generation,
            },
            Err(failure) => DriveSafetySign::SafeDisposition {
                cause: SafeDispositionCause::ProviderFailure(failure),
                safety_generation,
            },
        }
    }
}

impl Default for LocalCreateDriveSafety {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "drive_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "drive_physical_tests.rs"]
mod physical_tests;
