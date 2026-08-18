//! Non-bypassable local safety boundary for the Create drive realization.
//!
//! A physical drive offer must dispatch through this type. It accepts portable
//! body velocity meaning only after authority and fresh local safety truth are
//! present, lowers to Create OI wheel commands, and owns the finite stop on
//! expiry or hazard. It is not an author-wirable safety Gear.

use crate::{
    encode_drive_direct, encode_stop, write_command, CreateOiFailure, CreateUartProvider,
    CREATE_OI_MAX_WHEEL_SPEED_MM_S,
};

pub const MINIMUM_MOTION_TTL_MS: u32 = conduit_std_catalog::ROBOTICS_MINIMUM_MOTION_TTL_MS as u32;
pub const MAXIMUM_MOTION_TTL_MS: u32 = conduit_std_catalog::ROBOTICS_MAXIMUM_MOTION_TTL_MS as u32;
/// The admitted local motion clock is exactly one monotonic tick per millisecond.
pub const MOTION_CLOCK_TICKS_PER_SECOND: u32 = 1_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalHazard {
    EmergencyStop,
    WheelDrop,
    Cliff,
    Tilt,
    Impact,
    Charging,
    ControlLost,
    BodyLinkLost,
    WatchdogUnhealthy,
    SafetyGenerationRegressed,
    SafetyClockInvalid,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SafetyObservation {
    pub generation: u32,
    pub observed_at_tick: u64,
    pub maximum_age_ticks: u32,
    pub emergency_stop: bool,
    pub wheel_drop: bool,
    pub cliff: bool,
    pub tilt: bool,
    pub impact: bool,
    pub charging: bool,
    pub control_alive: bool,
    pub body_link_alive: bool,
    pub watchdog_healthy: bool,
}

impl SafetyObservation {
    pub fn first_hazard(self, now_tick: u64) -> Option<LocalHazard> {
        if self.observed_at_tick > now_tick {
            return Some(LocalHazard::SafetyClockInvalid);
        }
        if now_tick.saturating_sub(self.observed_at_tick) > u64::from(self.maximum_age_ticks) {
            return Some(LocalHazard::BodyLinkLost);
        }
        [
            (self.emergency_stop, LocalHazard::EmergencyStop),
            (self.wheel_drop, LocalHazard::WheelDrop),
            (self.cliff, LocalHazard::Cliff),
            (self.tilt, LocalHazard::Tilt),
            (self.impact, LocalHazard::Impact),
            (self.charging, LocalHazard::Charging),
            (!self.control_alive, LocalHazard::ControlLost),
            (!self.body_link_alive, LocalHazard::BodyLinkLost),
            (!self.watchdog_healthy, LocalHazard::WatchdogUnhealthy),
        ]
        .into_iter()
        .find_map(|(active, hazard)| active.then_some(hazard))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MotionAuthority<'a> {
    pub grant_id: &'a str,
    pub valid_until_tick: u64,
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
    SafetyStaleOrInhibited(LocalHazard),
    InvalidTtl,
    VelocityOutsideRealization,
    DeadlineOverflow,
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
pub struct LocalCreateDriveSafety {
    motion_deadline_tick: Option<u64>,
    active_authority_until_tick: Option<u64>,
    safety_generation: u32,
}

impl LocalCreateDriveSafety {
    pub const fn new() -> Self {
        Self {
            motion_deadline_tick: None,
            active_authority_until_tick: None,
            safety_generation: 0,
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
        write_command(provider, &command).map_err(DriveRefusal::Device)?;
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
mod tests {
    use super::*;
    use crate::{UartParity, UartProfile};

    struct Provider {
        available: bool,
        bytes: Vec<u8>,
    }
    impl CreateUartProvider for Provider {
        type Error = ();

        fn is_available(&self) -> bool {
            self.available
        }
        fn profile(&self) -> UartProfile {
            UartProfile {
                baud: 57_600,
                data_bits: 8,
                stop_bits: 1,
                parity: UartParity::None,
            }
        }
        fn write_all(&mut self, bytes: &[u8]) -> Result<(), ()> {
            self.bytes.extend_from_slice(bytes);
            Ok(())
        }
        fn read_byte(&mut self, _: u64) -> Result<Option<u8>, ()> {
            Ok(None)
        }
    }
    fn safe() -> SafetyObservation {
        SafetyObservation {
            generation: 7,
            observed_at_tick: 100,
            maximum_age_ticks: 20,
            emergency_stop: false,
            wheel_drop: false,
            cliff: false,
            tilt: false,
            impact: false,
            charging: false,
            control_alive: true,
            body_link_alive: true,
            watchdog_healthy: true,
        }
    }
    fn authority() -> MotionAuthority<'static> {
        MotionAuthority {
            grant_id: "grant/drive",
            valid_until_tick: 1_000,
        }
    }

    #[test]
    fn authority_and_fresh_safety_are_mandatory_before_any_motion_byte() {
        let mut provider = Provider {
            available: true,
            bytes: vec![],
        };
        let mut drive = LocalCreateDriveSafety::new();
        let request = DifferentialMotionRequest {
            left_mm_s: 20,
            right_mm_s: 20,
            ttl_ms: 100,
        };
        assert_eq!(
            drive.admit_motion(&mut provider, 100, None, safe(), request),
            DriveSafetySign::Refused(DriveRefusal::MissingAuthority)
        );
        assert!(provider.bytes.is_empty());
        let mut stale = safe();
        stale.observed_at_tick = 0;
        assert!(matches!(
            drive.admit_motion(&mut provider, 100, Some(authority()), stale, request),
            DriveSafetySign::Refused(DriveRefusal::SafetyStaleOrInhibited(
                LocalHazard::BodyLinkLost
            ))
        ));
        assert!(provider.bytes.is_empty());
    }

    #[test]
    fn ttl_expiry_and_hazard_force_exact_zero_wheel_command() {
        let mut provider = Provider {
            available: true,
            bytes: vec![],
        };
        let mut drive = LocalCreateDriveSafety::new();
        let request = DifferentialMotionRequest {
            left_mm_s: 20,
            right_mm_s: -20,
            ttl_ms: 100,
        };
        assert!(matches!(
            drive.admit_motion(&mut provider, 100, Some(authority()), safe(), request),
            DriveSafetySign::MotionAdmitted {
                deadline_tick: 200,
                ..
            }
        ));
        assert!(matches!(
            drive.supervise(
                &mut provider,
                200,
                SafetyObservation {
                    observed_at_tick: 200,
                    ..safe()
                }
            ),
            Some(DriveSafetySign::SafeDisposition {
                cause: SafeDispositionCause::DeadlineExpired,
                ..
            })
        ));
        assert_eq!(&provider.bytes[5..], &[145, 0, 0, 0, 0]);

        let mut hazard = safe();
        hazard.observed_at_tick = 210;
        hazard.wheel_drop = true;
        hazard.generation = 8;
        assert!(matches!(
            drive.supervise(&mut provider, 210, hazard),
            Some(DriveSafetySign::SafeDisposition {
                cause: SafeDispositionCause::Hazard(LocalHazard::WheelDrop),
                safety_generation: 8
            })
        ));
    }

    #[test]
    fn future_clock_and_regressed_safety_generation_fail_closed() {
        let mut provider = Provider {
            available: true,
            bytes: vec![],
        };
        let mut drive = LocalCreateDriveSafety::new();
        let request = DifferentialMotionRequest {
            left_mm_s: 20,
            right_mm_s: 20,
            ttl_ms: 100,
        };
        let mut future = safe();
        future.observed_at_tick = 101;
        assert_eq!(
            drive.admit_motion(&mut provider, 100, Some(authority()), future, request),
            DriveSafetySign::Refused(DriveRefusal::SafetyStaleOrInhibited(
                LocalHazard::SafetyClockInvalid
            ))
        );
        assert!(provider.bytes.is_empty());

        assert!(matches!(
            drive.admit_motion(&mut provider, 100, Some(authority()), safe(), request),
            DriveSafetySign::MotionAdmitted { .. }
        ));
        let regressed = SafetyObservation {
            generation: 6,
            observed_at_tick: 101,
            ..safe()
        };
        assert!(matches!(
            drive.supervise(&mut provider, 101, regressed),
            Some(DriveSafetySign::SafeDisposition {
                cause: SafeDispositionCause::Hazard(LocalHazard::SafetyGenerationRegressed),
                ..
            })
        ));
        assert_eq!(&provider.bytes[5..], &[145, 0, 0, 0, 0]);
    }
}
