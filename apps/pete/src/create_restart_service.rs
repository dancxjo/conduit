//! Exact attended replacement for the historical `restart_create` RPC verb.
//!
//! This is a finite coordinator over the existing stop, power, and OI-mode
//! service Signs. It owns no UART or GPIO provider and cannot bypass any of
//! those services' admission or verification contracts.

use crate::create_restart_validation::validate_create_restart_start;
use crate::{
    CreateModeServiceSign, CreatePowerServiceSign, CreatePowerState, CreateRestartAction,
    CreateRestartAuthority, CreateRestartBinding, CreateRestartModeObservation,
    CreateRestartModeStage, CreateRestartPowerStage, CreateRestartRefusal, CreateRestartRequest,
    CreateRestartSign, DriveSafetySign, OiMode, SafeDispositionCause,
    CREATE_MODE_SERVICE_AUTHORITY, CREATE_POWER_SERVICE_AUTHORITY,
};
use conduit_core::{BootId, HostId, OfferGeneration};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RestartPhase {
    AwaitingStop,
    AwaitingPowerOff,
    AwaitingPowerOn,
    AwaitingModeObservation,
    AwaitingModeSign,
    Completed,
    Failed,
}

pub struct PreparedCreateRestart {
    phase: RestartPhase,
    request_id: String,
    authority_grant_id: String,
    host_id: HostId,
    boot_id: BootId,
    offer_generation: OfferGeneration,
    robot_identity: String,
    power_implementation_id: String,
    power_attachment_id: String,
    mode_implementation_id: String,
    serial_base_id: String,
    mode_attachment_id: String,
    safe_disposition_generation: u32,
    initial_power_state: CreatePowerState,
    power_generation: u32,
    power_off_generation: u32,
    expected_mode: OiMode,
    expected_mode_generation: u32,
    target_mode: OiMode,
    deadline_tick: u64,
}

pub fn start_create_restart(
    binding: CreateRestartBinding<'_>,
    request: CreateRestartRequest<'_>,
    authority: Option<CreateRestartAuthority<'_>>,
    now_tick: u64,
) -> Result<(PreparedCreateRestart, CreateRestartAction), CreateRestartRefusal> {
    validate_create_restart_start(&binding, request, authority.as_ref(), now_tick)?;
    let authority = authority.expect("validated restart authority");
    let execution = PreparedCreateRestart {
        phase: RestartPhase::AwaitingStop,
        request_id: request.request_id.to_string(),
        authority_grant_id: authority.grant_id.to_string(),
        host_id: binding.host_id.clone(),
        boot_id: binding.boot_id.clone(),
        offer_generation: binding.offer_generation,
        robot_identity: binding.robot_identity.to_string(),
        power_implementation_id: binding.power_implementation_id.to_string(),
        power_attachment_id: binding.power_attachment_id.to_string(),
        mode_implementation_id: binding.mode_implementation_id.to_string(),
        serial_base_id: binding.serial_base_id.to_string(),
        mode_attachment_id: binding.mode_attachment_id.to_string(),
        safe_disposition_generation: binding.safe_disposition_generation,
        initial_power_state: binding.power_state,
        power_generation: binding.power_observation_generation,
        power_off_generation: 0,
        expected_mode: OiMode::Off,
        expected_mode_generation: 0,
        target_mode: request.target_mode,
        deadline_tick: request.deadline_tick,
    };
    let action = CreateRestartAction::AwaitSafeDisposition {
        expected_generation: binding.safe_disposition_generation,
    };
    Ok((execution, action))
}

impl PreparedCreateRestart {
    pub fn accept_safe_disposition(
        &mut self,
        sign: DriveSafetySign<'_>,
        now_tick: u64,
    ) -> Result<CreateRestartAction, CreateRestartRefusal> {
        self.require_phase(RestartPhase::AwaitingStop, now_tick)?;
        match sign {
            DriveSafetySign::SafeDisposition {
                cause: SafeDispositionCause::RequestedStop,
                safety_generation,
            } if safety_generation == self.safe_disposition_generation => {}
            DriveSafetySign::SafeDisposition {
                cause: SafeDispositionCause::ProviderFailure(_),
                ..
            } => return self.fail(CreateRestartRefusal::StopFailed),
            DriveSafetySign::SafeDisposition {
                cause: SafeDispositionCause::RequestedStop,
                ..
            } => return self.fail(CreateRestartRefusal::SafeDispositionGenerationMismatch),
            DriveSafetySign::SafeDisposition { .. } => {
                return self.fail(CreateRestartRefusal::UnexpectedStopCause)
            }
            _ => return self.fail(CreateRestartRefusal::StopFailed),
        }
        self.phase = RestartPhase::AwaitingPowerOff;
        Ok(CreateRestartAction::PowerOff(self.power_stage(
            "power-off",
            CreatePowerState::Off,
            self.power_generation,
        )))
    }

    pub fn accept_power_sign(
        &mut self,
        sign: &CreatePowerServiceSign,
        now_tick: u64,
    ) -> Result<CreateRestartAction, CreateRestartRefusal> {
        self.check_deadline(now_tick)?;
        match self.phase {
            RestartPhase::AwaitingPowerOff => {
                self.validate_power_sign(sign, "power-off", CreatePowerState::Off)?;
                if self.initial_power_state == CreatePowerState::On && !sign.pulse_emitted {
                    return self.fail(CreateRestartRefusal::RequiredPulseMissing);
                }
                self.power_off_generation = sign.observed_generation;
                self.power_generation = sign.observed_generation;
                self.phase = RestartPhase::AwaitingPowerOn;
                Ok(CreateRestartAction::PowerOn(self.power_stage(
                    "power-on",
                    CreatePowerState::On,
                    self.power_generation,
                )))
            }
            RestartPhase::AwaitingPowerOn => {
                self.validate_power_sign(sign, "power-on", CreatePowerState::On)?;
                if !sign.pulse_emitted {
                    return self.fail(CreateRestartRefusal::RequiredPulseMissing);
                }
                self.power_generation = sign.observed_generation;
                self.phase = RestartPhase::AwaitingModeObservation;
                Ok(CreateRestartAction::AwaitFreshModeObservation)
            }
            _ => self.fail(CreateRestartRefusal::WrongStage),
        }
    }

    pub fn accept_mode_observation(
        &mut self,
        observation: CreateRestartModeObservation<'_>,
        now_tick: u64,
    ) -> Result<CreateRestartAction, CreateRestartRefusal> {
        self.require_phase(RestartPhase::AwaitingModeObservation, now_tick)?;
        self.validate_mode_identity(&observation)?;
        if observation.generation == 0 || observation.maximum_age_ticks == 0 {
            return self.fail(CreateRestartRefusal::InvalidModeFreshness);
        }
        if now_tick.saturating_sub(observation.observed_at_tick)
            > u64::from(observation.maximum_age_ticks)
        {
            return self.fail(CreateRestartRefusal::StaleModeObservation);
        }
        if observation.mode == OiMode::Off {
            return self.fail(CreateRestartRefusal::ModeOff);
        }
        self.expected_mode = observation.mode;
        self.expected_mode_generation = observation.generation;
        self.phase = RestartPhase::AwaitingModeSign;
        Ok(CreateRestartAction::RestoreMode(CreateRestartModeStage {
            request_id: self.stage_request_id("restore-mode"),
            expected_current_mode: observation.mode,
            expected_mode_observation_generation: observation.generation,
            target_mode: self.target_mode,
            deadline_tick: self.deadline_tick,
        }))
    }

    pub fn accept_mode_sign(
        &mut self,
        sign: &CreateModeServiceSign,
        now_tick: u64,
    ) -> Result<CreateRestartSign, CreateRestartRefusal> {
        self.require_phase(RestartPhase::AwaitingModeSign, now_tick)?;
        if sign.authority_grant_id != CREATE_MODE_SERVICE_AUTHORITY {
            return self.fail(CreateRestartRefusal::DownstreamAuthorityMismatch);
        }
        if sign.request_id != self.stage_request_id("restore-mode") {
            return self.fail(CreateRestartRefusal::RequestMismatch);
        }
        if sign.host_id != self.host_id {
            return self.fail(CreateRestartRefusal::HostMismatch);
        }
        if sign.boot_id != self.boot_id {
            return self.fail(CreateRestartRefusal::BootMismatch);
        }
        if sign.offer_generation != self.offer_generation {
            return self.fail(CreateRestartRefusal::OfferGenerationMismatch);
        }
        if sign.implementation_id != self.mode_implementation_id
            || sign.serial_base_id != self.serial_base_id
        {
            return self.fail(CreateRestartRefusal::ImplementationMismatch);
        }
        if sign.robot_identity != self.robot_identity
            || sign.service_attachment_id != self.mode_attachment_id
        {
            return self.fail(CreateRestartRefusal::AttachmentMismatch);
        }
        if sign.prior_mode != self.expected_mode
            || sign.prior_mode_observation_generation != self.expected_mode_generation
        {
            return self.fail(CreateRestartRefusal::ModeGenerationMismatch);
        }
        if sign.deadline_tick != self.deadline_tick {
            return self.fail(CreateRestartRefusal::RequestMismatch);
        }
        if sign.observed_mode != self.target_mode {
            return self.fail(CreateRestartRefusal::ModeMismatch);
        }
        self.phase = RestartPhase::Completed;
        Ok(CreateRestartSign {
            request_id: self.request_id.clone(),
            authority_grant_id: self.authority_grant_id.clone(),
            host_id: self.host_id.clone(),
            boot_id: self.boot_id.clone(),
            offer_generation: self.offer_generation,
            robot_identity: self.robot_identity.clone(),
            safe_disposition_generation: self.safe_disposition_generation,
            power_off_generation: self.power_off_generation,
            power_on_generation: self.power_generation,
            observed_mode: sign.observed_mode,
            deadline_tick: self.deadline_tick,
        })
    }

    fn validate_power_sign(
        &mut self,
        sign: &CreatePowerServiceSign,
        stage: &str,
        target: CreatePowerState,
    ) -> Result<(), CreateRestartRefusal> {
        if sign.authority_grant_id != CREATE_POWER_SERVICE_AUTHORITY {
            return self.fail(CreateRestartRefusal::DownstreamAuthorityMismatch);
        }
        if sign.request_id != self.stage_request_id(stage) {
            return self.fail(CreateRestartRefusal::RequestMismatch);
        }
        if sign.host_id != self.host_id {
            return self.fail(CreateRestartRefusal::HostMismatch);
        }
        if sign.boot_id != self.boot_id {
            return self.fail(CreateRestartRefusal::BootMismatch);
        }
        if sign.offer_generation != self.offer_generation {
            return self.fail(CreateRestartRefusal::OfferGenerationMismatch);
        }
        if sign.implementation_id != self.power_implementation_id {
            return self.fail(CreateRestartRefusal::ImplementationMismatch);
        }
        if sign.robot_identity != self.robot_identity
            || sign.attachment_id != self.power_attachment_id
        {
            return self.fail(CreateRestartRefusal::AttachmentMismatch);
        }
        if sign.safe_disposition_generation != self.safe_disposition_generation {
            return self.fail(CreateRestartRefusal::SafeDispositionGenerationMismatch);
        }
        if sign.prior_observation_generation != self.power_generation {
            return self.fail(CreateRestartRefusal::PowerGenerationMismatch);
        }
        let expected_prior = if target == CreatePowerState::Off {
            self.initial_power_state
        } else {
            CreatePowerState::Off
        };
        if sign.prior_state != expected_prior {
            return self.fail(CreateRestartRefusal::PowerPriorStateMismatch);
        }
        if sign.observed_state != target {
            return self.fail(CreateRestartRefusal::PowerStateMismatch);
        }
        if sign.pulse_emitted && sign.observed_generation <= sign.prior_observation_generation {
            return self.fail(CreateRestartRefusal::PowerGenerationMismatch);
        }
        Ok(())
    }

    fn validate_mode_identity(
        &mut self,
        observation: &CreateRestartModeObservation<'_>,
    ) -> Result<(), CreateRestartRefusal> {
        if observation.host_id != &self.host_id {
            return self.fail(CreateRestartRefusal::HostMismatch);
        }
        if observation.boot_id != &self.boot_id {
            return self.fail(CreateRestartRefusal::BootMismatch);
        }
        if observation.offer_generation != self.offer_generation {
            return self.fail(CreateRestartRefusal::OfferGenerationMismatch);
        }
        if observation.implementation_id != self.mode_implementation_id
            || observation.serial_base_id != self.serial_base_id
        {
            return self.fail(CreateRestartRefusal::ImplementationMismatch);
        }
        if observation.robot_identity != self.robot_identity
            || observation.service_attachment_id != self.mode_attachment_id
        {
            return self.fail(CreateRestartRefusal::AttachmentMismatch);
        }
        Ok(())
    }

    fn power_stage(
        &self,
        stage: &str,
        target: CreatePowerState,
        generation: u32,
    ) -> CreateRestartPowerStage {
        CreateRestartPowerStage {
            request_id: self.stage_request_id(stage),
            target,
            expected_observation_generation: generation,
            expected_safe_disposition_generation: self.safe_disposition_generation,
            deadline_tick: self.deadline_tick,
        }
    }

    fn stage_request_id(&self, stage: &str) -> String {
        format!("{}/{stage}", self.request_id)
    }

    fn require_phase(
        &mut self,
        phase: RestartPhase,
        now_tick: u64,
    ) -> Result<(), CreateRestartRefusal> {
        self.check_deadline(now_tick)?;
        if self.phase != phase {
            return self.fail(CreateRestartRefusal::WrongStage);
        }
        Ok(())
    }

    fn check_deadline(&mut self, now_tick: u64) -> Result<(), CreateRestartRefusal> {
        if now_tick > self.deadline_tick {
            return self.fail(CreateRestartRefusal::DeadlineExpired);
        }
        Ok(())
    }

    fn fail<T>(&mut self, refusal: CreateRestartRefusal) -> Result<T, CreateRestartRefusal> {
        self.phase = RestartPhase::Failed;
        Err(refusal)
    }
}

#[cfg(test)]
#[path = "create_restart_service_tests.rs"]
mod tests;
