use super::*;
use crate::{
    CreateOiFailure, CREATE_MODE_SERVICE_ATTACHMENT, CREATE_MODE_SERVICE_IMPLEMENTATION,
    CREATE_POWER_SERVICE_ATTACHMENT, CREATE_POWER_SERVICE_IMPLEMENTATION,
    CREATE_RESTART_SERVICE_AUTHORITY,
};

fn host() -> HostId {
    HostId::from("host/pete")
}

fn boot() -> BootId {
    BootId::from("boot/pete-1")
}

fn binding<'a>(host: &'a HostId, boot: &'a BootId) -> CreateRestartBinding<'a> {
    CreateRestartBinding {
        host_id: host,
        boot_id: boot,
        offer_generation: OfferGeneration(12),
        robot_identity: "create/physical-1",
        power_implementation_id: CREATE_POWER_SERVICE_IMPLEMENTATION,
        power_attachment_id: CREATE_POWER_SERVICE_ATTACHMENT,
        mode_implementation_id: CREATE_MODE_SERVICE_IMPLEMENTATION,
        serial_base_id: "base/create-uart-1",
        mode_attachment_id: CREATE_MODE_SERVICE_ATTACHMENT,
        safe_disposition_generation: 8,
        power_state: CreatePowerState::On,
        power_observation_generation: 4,
    }
}

fn request() -> CreateRestartRequest<'static> {
    CreateRestartRequest {
        request_id: "service/restart-1",
        target_mode: OiMode::Full,
        deadline_tick: 200,
    }
}

fn authority<'a>(host: &'a HostId, boot: &'a BootId) -> CreateRestartAuthority<'a> {
    CreateRestartAuthority {
        grant_id: CREATE_RESTART_SERVICE_AUTHORITY,
        host_id: host,
        boot_id: boot,
        offer_generation: OfferGeneration(12),
        robot_identity: "create/physical-1",
        valid_until_tick: 220,
    }
}

fn power_sign(
    host: &HostId,
    boot: &BootId,
    stage: &str,
    prior: CreatePowerState,
    observed: CreatePowerState,
    prior_generation: u32,
    observed_generation: u32,
) -> CreatePowerServiceSign {
    CreatePowerServiceSign {
        request_id: format!("service/restart-1/{stage}"),
        authority_grant_id: CREATE_POWER_SERVICE_AUTHORITY.to_string(),
        host_id: host.clone(),
        boot_id: boot.clone(),
        offer_generation: OfferGeneration(12),
        implementation_id: CREATE_POWER_SERVICE_IMPLEMENTATION.to_string(),
        robot_identity: "create/physical-1".to_string(),
        attachment_id: CREATE_POWER_SERVICE_ATTACHMENT.to_string(),
        prior_state: prior,
        observed_state: observed,
        prior_observation_generation: prior_generation,
        observed_generation,
        safe_disposition_generation: 8,
        pulse_emitted: true,
    }
}

fn mode_observation<'a>(host: &'a HostId, boot: &'a BootId) -> CreateRestartModeObservation<'a> {
    CreateRestartModeObservation {
        host_id: host,
        boot_id: boot,
        offer_generation: OfferGeneration(12),
        implementation_id: CREATE_MODE_SERVICE_IMPLEMENTATION,
        serial_base_id: "base/create-uart-1",
        robot_identity: "create/physical-1",
        service_attachment_id: CREATE_MODE_SERVICE_ATTACHMENT,
        mode: OiMode::Passive,
        generation: 7,
        observed_at_tick: 150,
        maximum_age_ticks: 20,
    }
}

fn mode_sign(host: &HostId, boot: &BootId) -> CreateModeServiceSign {
    CreateModeServiceSign {
        request_id: "service/restart-1/restore-mode".to_string(),
        authority_grant_id: CREATE_MODE_SERVICE_AUTHORITY.to_string(),
        host_id: host.clone(),
        boot_id: boot.clone(),
        offer_generation: OfferGeneration(12),
        implementation_id: CREATE_MODE_SERVICE_IMPLEMENTATION.to_string(),
        serial_base_id: "base/create-uart-1".to_string(),
        robot_identity: "create/physical-1".to_string(),
        service_attachment_id: CREATE_MODE_SERVICE_ATTACHMENT.to_string(),
        prior_mode: OiMode::Passive,
        prior_mode_observation_generation: 7,
        observed_mode: OiMode::Full,
        deadline_tick: 200,
    }
}

fn through_stop(host: &HostId, boot: &BootId) -> PreparedCreateRestart {
    let (mut execution, first) = start_create_restart(
        binding(host, boot),
        request(),
        Some(authority(host, boot)),
        100,
    )
    .unwrap();
    assert_eq!(
        first,
        CreateRestartAction::AwaitSafeDisposition {
            expected_generation: 8
        }
    );
    let off = execution
        .accept_safe_disposition(
            DriveSafetySign::SafeDisposition {
                cause: SafeDispositionCause::RequestedStop,
                safety_generation: 8,
            },
            101,
        )
        .unwrap();
    let CreateRestartAction::PowerOff(off) = off else {
        panic!("expected power-off service stage")
    };
    assert_eq!(off.request().target, CreatePowerState::Off);
    assert_eq!(off.request().expected_observation_generation, 4);
    execution
}

#[test]
fn exact_existing_signs_complete_the_finite_restart_transaction() {
    let host = host();
    let boot = boot();
    let mut execution = through_stop(&host, &boot);

    let on = execution
        .accept_power_sign(
            &power_sign(
                &host,
                &boot,
                "power-off",
                CreatePowerState::On,
                CreatePowerState::Off,
                4,
                5,
            ),
            120,
        )
        .unwrap();
    let CreateRestartAction::PowerOn(on) = on else {
        panic!("expected power-on service stage")
    };
    assert_eq!(on.request().expected_observation_generation, 5);

    assert_eq!(
        execution
            .accept_power_sign(
                &power_sign(
                    &host,
                    &boot,
                    "power-on",
                    CreatePowerState::Off,
                    CreatePowerState::On,
                    5,
                    6,
                ),
                140,
            )
            .unwrap(),
        CreateRestartAction::AwaitFreshModeObservation
    );
    let restore = execution
        .accept_mode_observation(mode_observation(&host, &boot), 151)
        .unwrap();
    let CreateRestartAction::RestoreMode(restore) = restore else {
        panic!("expected exact mode service stage")
    };
    assert_eq!(restore.request().expected_current_mode, OiMode::Passive);
    assert_eq!(restore.request().target_mode, OiMode::Full);

    let sign = execution
        .accept_mode_sign(&mode_sign(&host, &boot), 160)
        .unwrap();
    assert_eq!(sign.power_off_generation, 5);
    assert_eq!(sign.power_on_generation, 6);
    assert_eq!(sign.observed_mode, OiMode::Full);
}

#[test]
fn provider_failure_during_stop_is_not_reinterpreted_as_safe() {
    let host = host();
    let boot = boot();
    let (mut execution, _) = start_create_restart(
        binding(&host, &boot),
        request(),
        Some(authority(&host, &boot)),
        100,
    )
    .unwrap();
    assert_eq!(
        execution.accept_safe_disposition(
            DriveSafetySign::SafeDisposition {
                cause: SafeDispositionCause::ProviderFailure(CreateOiFailure::WriteFailed),
                safety_generation: 8,
            },
            101
        ),
        Err(CreateRestartRefusal::StopFailed)
    );
}

#[test]
fn power_signs_cannot_be_reordered_replayed_or_accepted_without_a_pulse() {
    let host = host();
    let boot = boot();
    let mut execution = through_stop(&host, &boot);
    assert_eq!(
        execution.accept_power_sign(
            &power_sign(
                &host,
                &boot,
                "power-on",
                CreatePowerState::Off,
                CreatePowerState::On,
                4,
                5
            ),
            110
        ),
        Err(CreateRestartRefusal::RequestMismatch)
    );

    let mut execution = through_stop(&host, &boot);
    let mut sign = power_sign(
        &host,
        &boot,
        "power-off",
        CreatePowerState::On,
        CreatePowerState::Off,
        4,
        4,
    );
    sign.pulse_emitted = false;
    assert_eq!(
        execution.accept_power_sign(&sign, 110),
        Err(CreateRestartRefusal::RequiredPulseMissing)
    );
}

#[test]
fn stale_or_wrong_identity_mode_truth_fails_at_the_mode_seam() {
    let host = host();
    let boot = boot();
    let mut execution = through_stop(&host, &boot);
    execution
        .accept_power_sign(
            &power_sign(
                &host,
                &boot,
                "power-off",
                CreatePowerState::On,
                CreatePowerState::Off,
                4,
                5,
            ),
            120,
        )
        .unwrap();
    execution
        .accept_power_sign(
            &power_sign(
                &host,
                &boot,
                "power-on",
                CreatePowerState::Off,
                CreatePowerState::On,
                5,
                6,
            ),
            140,
        )
        .unwrap();
    let mut stale = mode_observation(&host, &boot);
    stale.observed_at_tick = 100;
    assert_eq!(
        execution.accept_mode_observation(stale, 151),
        Err(CreateRestartRefusal::StaleModeObservation)
    );
}

#[test]
fn admission_requires_exact_authority_and_finite_supported_target() {
    let host = host();
    let boot = boot();
    assert!(matches!(
        start_create_restart(binding(&host, &boot), request(), None, 100),
        Err(CreateRestartRefusal::MissingAuthority)
    ));
    let mut passive = request();
    passive.target_mode = OiMode::Passive;
    assert!(matches!(
        start_create_restart(
            binding(&host, &boot),
            passive,
            Some(authority(&host, &boot)),
            100
        ),
        Err(CreateRestartRefusal::UnsupportedTargetMode)
    ));
}
