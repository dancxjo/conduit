//! Admission validation for the exact attended Create restart service.

use crate::{
    CreatePowerState, CreateRestartAuthority, CreateRestartBinding, CreateRestartRefusal,
    CreateRestartRequest, OiMode, CREATE_RESTART_SERVICE_AUTHORITY,
};

pub(crate) fn validate_create_restart_start(
    binding: &CreateRestartBinding<'_>,
    request: CreateRestartRequest<'_>,
    authority: Option<&CreateRestartAuthority<'_>>,
    now_tick: u64,
) -> Result<(), CreateRestartRefusal> {
    if binding.robot_identity.is_empty()
        || binding.power_implementation_id.is_empty()
        || binding.power_attachment_id.is_empty()
        || binding.mode_implementation_id.is_empty()
        || binding.serial_base_id.is_empty()
        || binding.mode_attachment_id.is_empty()
    {
        return Err(CreateRestartRefusal::MissingIdentity);
    }
    if binding.power_state == CreatePowerState::Unknown || binding.power_observation_generation == 0
    {
        return Err(CreateRestartRefusal::UnknownInitialPower);
    }
    if binding.safe_disposition_generation == 0 {
        return Err(CreateRestartRefusal::MissingSafeDisposition);
    }
    if !matches!(request.target_mode, OiMode::Safe | OiMode::Full) {
        return Err(CreateRestartRefusal::UnsupportedTargetMode);
    }
    if request.request_id.is_empty() {
        return Err(CreateRestartRefusal::MissingRequestIdentity);
    }
    if request.deadline_tick <= now_tick {
        return Err(CreateRestartRefusal::InvalidDeadline);
    }
    let authority = authority.ok_or(CreateRestartRefusal::MissingAuthority)?;
    if authority.grant_id != CREATE_RESTART_SERVICE_AUTHORITY {
        return Err(CreateRestartRefusal::WrongAuthority);
    }
    if authority.valid_until_tick <= now_tick {
        return Err(CreateRestartRefusal::AuthorityExpired);
    }
    if authority.valid_until_tick < request.deadline_tick {
        return Err(CreateRestartRefusal::OperationOutlivesAuthority);
    }
    if authority.host_id != binding.host_id {
        return Err(CreateRestartRefusal::HostMismatch);
    }
    if authority.boot_id != binding.boot_id {
        return Err(CreateRestartRefusal::BootMismatch);
    }
    if authority.offer_generation != binding.offer_generation {
        return Err(CreateRestartRefusal::OfferGenerationMismatch);
    }
    if authority.robot_identity != binding.robot_identity {
        return Err(CreateRestartRefusal::RobotIdentityMismatch);
    }
    Ok(())
}
