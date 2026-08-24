//! Physical execution of the sealed Pete capstone through Conduit's
//! production kernel and local Create safety realization.

use conduit_create_oi::{
    CreateActuatorSupervisionSign, CreateUartProvider, DifferentialMotionRequest,
    DriveSafetySign, IndependentWatchdogObservation, LocalCreateDriveSafety, LocalSafetyEnvelope,
    MotionAuthority, MotionSafetyAuthority, PhysicalActuatorObservation, SafetyInputObservation,
    SafetyInputs, SafeDispositionCause,
};
use conduit_core::{InfoBool, Scalar};
use conduit_kernel::{
    scheduler::{HostOperationRequest, SchedulerStatus},
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationOutcome,
    SignSink,
};
use portable_atomic::{AtomicI32, Ordering};

use super::{create_play, imu_control};
use create_play::{RequestKind, RequestState};
use crate::capstone_kernel::{
    prepare_scheduler, CapstoneScheduler, DRIVE_NODE, OBSERVATION_NODE, OBSERVATION_REQUEST,
};

const SAFETY_MAXIMUM_AGE_MS: u32 = 100;
static DISTANCE_MM: AtomicI32 = AtomicI32::new(0);

pub struct Runtime {
    envelope: LocalSafetyEnvelope,
    drive: LocalCreateDriveSafety,
    observation_generation: u32,
    latest_safety: Option<conduit_create_oi::SafetyObservation>,
    scheduler: Option<CapstoneScheduler>,
    drive_request: Option<HostOperationRequest>,
}

impl Runtime {
    pub const fn new() -> Self {
        Self {
            envelope: LocalSafetyEnvelope::new(),
            drive: LocalCreateDriveSafety::new(),
            observation_generation: 0,
            latest_safety: None,
            scheduler: None,
            drive_request: None,
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(
            create_play::snapshot().state,
            RequestState::Active | RequestState::Withdrawal
        )
    }

    pub fn observe<P: CreateUartProvider>(
        &mut self,
        provider: &mut P,
        now_ms: u32,
        group_zero: &[u8],
        charging_sources: u8,
    ) {
        self.observation_generation = self.observation_generation.wrapping_add(1).max(1);
        let imu = imu_control::snapshot();
        let imu_fresh = imu_control::is_fresh(&imu, now_ms);
        let bump_bits = group_zero[0];
        let distance_delta = i16::from_be_bytes([group_zero[12], group_zero[13]]);
        let distance = DISTANCE_MM
            .fetch_add(i32::from(distance_delta), Ordering::Relaxed)
            .wrapping_add(i32::from(distance_delta));
        let contact = bump_bits & 0b11 != 0;
        let wheel_drop = bump_bits & 0b1_1100 != 0;
        let cliff = group_zero[2..=5].iter().any(|value| *value != 0);
        let charging = group_zero[16] != 0 || charging_sources != 0;
        let imu_input = |active| {
            if !imu_fresh {
                SafetyInputObservation::Unavailable
            } else if active {
                SafetyInputObservation::Active
            } else {
                SafetyInputObservation::Clear
            }
        };
        let inputs = SafetyInputs {
            generation: self.observation_generation,
            observed_at_tick: u64::from(now_ms),
            maximum_age_ticks: SAFETY_MAXIMUM_AGE_MS,
            // The current carrier has no independent physical E-stop input.
            // This HIL request therefore uses the explicit reduced
            // wheels-off-floor authority even though the hardware watchdog is
            // present and healthy.
            emergency_stop: SafetyInputObservation::Unavailable,
            wheel_drop,
            cliff,
            contact,
            tilt: imu_input(imu.tilt_active),
            impact: imu_input(imu.impact_active),
            charging,
            control_alive: true,
            body_link_alive: true,
            independent_watchdog: IndependentWatchdogObservation::Healthy,
        };
        if self.envelope.observe(inputs, u64::from(now_ms)).is_err() {
            self.preempt(provider, self.observation_generation, 5);
            return;
        }
        let Some(safety) = self.envelope.snapshot() else {
            return;
        };
        self.latest_safety = Some(safety);

        if create_play::claim_pending(RequestKind::Motion) {
            self.start_capstone(provider, now_ms, safety, contact);
        }

        if matches!(
            create_play::snapshot().state,
            RequestState::Active | RequestState::Withdrawal
        ) {
            let observation = PhysicalActuatorObservation {
                safety,
                contact: conduit_create_oi::ContactFrame {
                    generation: self.observation_generation,
                    observed_at_tick: u64::from(now_ms),
                    maximum_age_ticks: SAFETY_MAXIMUM_AGE_MS,
                    left: bump_bits & 0b10 != 0,
                    right: bump_bits & 0b01 != 0,
                },
                distance_mm: Some(distance),
                explicitly_disarmed: false,
                motor_feedback_invalid: false,
            };
            if let Some(sign) = self.drive.supervise_physical(
                provider,
                u64::from(now_ms),
                observation,
            ) {
                match sign {
                    CreateActuatorSupervisionSign::Drive(DriveSafetySign::SafeDisposition {
                        cause,
                        safety_generation,
                    }) => {
                        create_play::set_safety_generation(safety_generation);
                        match cause {
                            SafeDispositionCause::DeadlineExpired
                            | SafeDispositionCause::RequestedStop => {
                                self.complete_capstone();
                            }
                            SafeDispositionCause::ProviderFailure(_) => {
                                self.fail_capstone(7, RequestState::Preempted);
                            }
                            SafeDispositionCause::Hazard(_)
                            | SafeDispositionCause::AuthorityExpired => {
                                self.fail_capstone(9, RequestState::Preempted);
                            }
                        }
                    }
                    CreateActuatorSupervisionSign::Drive(DriveSafetySign::Refused(_)) => {
                        self.fail_capstone(2, RequestState::Refused);
                    }
                    CreateActuatorSupervisionSign::ContactWithdrawal(
                        conduit_create_oi::ContactWithdrawalSign::Started { .. },
                    ) => {
                        create_play::set_state(RequestState::Withdrawal);
                    }
                    CreateActuatorSupervisionSign::ContactWithdrawal(
                        conduit_create_oi::ContactWithdrawalSign::Completed { .. },
                    ) => {
                        // The mandatory body-local safety action completed,
                        // but it preempted the requested forward motion. Keep
                        // those terminal truths distinct.
                        self.fail_capstone(10, RequestState::Preempted);
                    }
                    CreateActuatorSupervisionSign::ContactWithdrawal(
                        conduit_create_oi::ContactWithdrawalSign::Preempted { .. },
                    ) => {
                        self.fail_capstone(3, RequestState::Preempted);
                    }
                    CreateActuatorSupervisionSign::Drive(DriveSafetySign::MotionAdmitted {
                        ..
                    }) => {}
                }
            }
        }
    }

    pub fn tick<P: CreateUartProvider>(&mut self, provider: &mut P, now_ms: u32) {
        if create_play::snapshot().state != RequestState::Active {
            return;
        }
        let Some(safety) = self.latest_safety else {
            return;
        };
        if let Some(DriveSafetySign::SafeDisposition {
            cause,
            safety_generation,
        }) = self.drive.supervise(provider, u64::from(now_ms), safety)
        {
            create_play::set_safety_generation(safety_generation);
            match cause {
                SafeDispositionCause::DeadlineExpired | SafeDispositionCause::RequestedStop => {
                    self.complete_capstone();
                }
                SafeDispositionCause::ProviderFailure(_) => {
                    self.fail_capstone(7, RequestState::Preempted);
                }
                SafeDispositionCause::Hazard(_) | SafeDispositionCause::AuthorityExpired => {
                    self.fail_capstone(9, RequestState::Preempted);
                }
            }
        }
    }

    fn start_capstone<P: CreateUartProvider>(
        &mut self,
        provider: &mut P,
        now_ms: u32,
        safety: conduit_create_oi::SafetyObservation,
        contact: bool,
    ) {
        let requested = Scalar::from_raw_microunits(100_000).encode();
        let angular = Scalar::ZERO.encode();
        let stopped = Scalar::ZERO.encode();
        let Ok(mut scheduler) = prepare_scheduler(&requested, &angular, &stopped) else {
            self.fail_capstone(11, RequestState::Refused);
            return;
        };
        let Ok(observation) = next_kernel_request(&mut scheduler) else {
            self.fail_capstone(12, RequestState::Refused);
            return;
        };
        if observation.node != OBSERVATION_NODE || observation.request != OBSERVATION_REQUEST {
            self.fail_capstone(13, RequestState::Refused);
            return;
        }
        let observation_value = match scheduler.store_host_value(&InfoBool::new(contact).encode()) {
            Ok(value) => value,
            Err(_) => {
                self.fail_capstone(14, RequestState::Refused);
                return;
            }
        };
        if scheduler
            .complete_host_operation(
                observation.node,
                observation.request,
                HostOperationOutcome {
                    disposition: HostOperationDisposition::Completed,
                    output: Some(
                        BoundedValueRef::new(
                            observation_value,
                            conduit_core::BOOL_ENCODED_LEN as u32,
                        )
                        .expect("Boolean observation has an exact bound"),
                    ),
                    failure: None,
                },
            )
            .is_err()
        {
            self.fail_capstone(15, RequestState::Refused);
            return;
        }
        let Ok(drive_request) = next_kernel_request(&mut scheduler) else {
            self.fail_capstone(16, RequestState::Refused);
            return;
        };
        if drive_request.node != DRIVE_NODE {
            self.fail_capstone(17, RequestState::Refused);
            return;
        }
        let selected = match scheduler
            .host_value(drive_request.input.value)
            .ok()
            .and_then(|bytes| Scalar::decode(bytes).ok())
        {
            Some(selected) => selected,
            None => {
                self.fail_capstone(18, RequestState::Refused);
                return;
            }
        };
        let selected_raw = selected.raw_microunits();
        if !matches!(selected_raw, 0 | 100_000) {
            self.fail_capstone(19, RequestState::Refused);
            return;
        }
        create_play::set_selected(selected_raw as i32);
        self.scheduler = Some(scheduler);
        self.drive_request = Some(drive_request);

        if selected_raw == 0 {
            let sign = self.drive.stop(provider, safety.generation);
            if matches!(sign, DriveSafetySign::SafeDisposition { .. }) {
                create_play::set_safety_generation(safety.generation);
                self.complete_capstone();
            } else {
                self.fail_capstone(20, RequestState::Refused);
            }
            return;
        }

        let authority = MotionAuthority {
            grant_id: create_play::AUTHORITY_GRANT,
            valid_until_tick: u64::from(now_ms) + u64::from(create_play::TTL_MS) + 100,
            safety_class: MotionSafetyAuthority::ReducedWheelsOffFloor,
        };
        match self.drive.admit_motion(
            provider,
            u64::from(now_ms),
            Some(authority),
            safety,
            DifferentialMotionRequest {
                left_mm_s: create_play::SPEED_MM_S,
                right_mm_s: create_play::SPEED_MM_S,
                ttl_ms: create_play::TTL_MS,
            },
        ) {
            DriveSafetySign::MotionAdmitted {
                safety_generation,
                deadline_tick,
                ..
            } => {
                create_play::set_safety_generation(safety_generation);
                create_play::set_deadline(deadline_tick as u32);
                create_play::set_state(RequestState::Active);
            }
            DriveSafetySign::Refused(_) | DriveSafetySign::SafeDisposition { .. } => {
                self.fail_capstone(1, RequestState::Refused);
            }
        }
    }

    fn complete_capstone(&mut self) {
        let (Some(mut scheduler), Some(request)) =
            (self.scheduler.take(), self.drive_request.take())
        else {
            create_play::set_result(21);
            create_play::set_state(RequestState::Preempted);
            return;
        };
        let completed = scheduler
            .complete_host_operation(
                request.node,
                request.request,
                HostOperationOutcome {
                    disposition: HostOperationDisposition::Completed,
                    output: None,
                    failure: None,
                },
            )
            .is_ok()
            && scheduler.run(128).is_ok();
        create_play::set_kernel_metrics(
            scheduler.decisions(),
            u32::from(scheduler.signs().len()),
        );
        if completed {
            create_play::set_state(RequestState::Completed);
        } else {
            create_play::set_result(22);
            create_play::set_state(RequestState::Preempted);
        }
    }

    fn fail_capstone(&mut self, code: u8, state: RequestState) {
        if let (Some(mut scheduler), Some(request)) =
            (self.scheduler.take(), self.drive_request.take())
        {
            let _ = scheduler.complete_host_operation(
                request.node,
                request.request,
                HostOperationOutcome {
                    disposition: HostOperationDisposition::Failed,
                    output: None,
                    failure: Some(Failure {
                        code: FailureCode::HostOperationFailed,
                        detail: u16::from(code),
                    }),
                },
            );
            create_play::set_kernel_metrics(
                scheduler.decisions(),
                u32::from(scheduler.signs().len()),
            );
        }
        create_play::set_result(code);
        create_play::set_state(state);
    }

    pub fn link_lost<P: CreateUartProvider>(&mut self, provider: &mut P) {
        if matches!(
            create_play::snapshot().state,
            RequestState::Active | RequestState::Withdrawal
        ) {
            let generation = self
                .latest_safety
                .map_or(self.observation_generation, |safety| safety.generation);
            let _ = self.drive.stop(provider, generation);
            self.fail_capstone(4, RequestState::Preempted);
        }
    }

    fn preempt<P: CreateUartProvider>(&mut self, provider: &mut P, generation: u32, code: u8) {
        if matches!(
            create_play::snapshot().state,
            RequestState::Active | RequestState::Withdrawal
        ) {
            let _ = self.drive.stop(provider, generation);
            self.fail_capstone(code, RequestState::Preempted);
        }
    }
}

fn next_kernel_request(
    scheduler: &mut CapstoneScheduler,
) -> Result<HostOperationRequest, ()> {
    for _ in 0..128 {
        if let Some(request) = scheduler.next_host_request() {
            return Ok(request);
        }
        match scheduler.step().map_err(|_| ())? {
            SchedulerStatus::Progress { .. } => {}
            SchedulerStatus::Idle
            | SchedulerStatus::Complete
            | SchedulerStatus::Cancelled => return Err(()),
        }
    }
    Err(())
}
