use super::*;

#[derive(Default)]
struct Provider {
    available: bool,
    levels: Vec<bool>,
}

impl CreatePowerToggleProvider for Provider {
    type Error = ();
    fn is_available(&self) -> bool {
        self.available
    }
    fn set_output_low(&mut self) -> Result<(), Self::Error> {
        self.levels.push(false);
        Ok(())
    }
    fn set_output_high(&mut self) -> Result<(), Self::Error> {
        self.levels.push(true);
        Ok(())
    }
}

fn provider() -> Provider {
    Provider {
        available: true,
        levels: Vec::new(),
    }
}
fn host() -> HostId {
    HostId::from("host/pico-pete")
}
fn boot() -> BootId {
    BootId::from("boot/pico-pete-1")
}

fn binding<'a>(
    host: &'a HostId,
    boot: &'a BootId,
    state: CreatePowerState,
) -> CreatePowerServiceBinding<'a> {
    CreatePowerServiceBinding {
        host_id: host,
        boot_id: boot,
        offer_generation: OfferGeneration(6),
        implementation_id: CREATE_POWER_SERVICE_IMPLEMENTATION,
        robot_identity: "create/physical-1",
        attachment_id: CREATE_POWER_SERVICE_ATTACHMENT,
        translation_path_verified: true,
        translator_enabled: true,
        output_idle_low_observed: true,
        direct_untranslated_connection: false,
        motion_active: false,
        safe_disposition_generation: 8,
        power: CreatePowerObservation {
            state,
            generation: 1,
            observed_at_tick: 90,
            maximum_age_ticks: 20,
        },
        pulse_profile: CreatePowerPulseProfile {
            low_settle_ticks: 5,
            high_pulse_ticks: 10,
        },
    }
}

fn authority<'a>(host: &'a HostId, boot: &'a BootId) -> CreatePowerServiceAuthority<'a> {
    CreatePowerServiceAuthority {
        grant_id: CREATE_POWER_SERVICE_AUTHORITY,
        host_id: host,
        boot_id: boot,
        offer_generation: OfferGeneration(6),
        implementation_id: CREATE_POWER_SERVICE_IMPLEMENTATION,
        robot_identity: "create/physical-1",
        attachment_id: CREATE_POWER_SERVICE_ATTACHMENT,
        valid_until_tick: 250,
    }
}

fn request(target: CreatePowerState) -> CreatePowerServiceRequest<'static> {
    CreatePowerServiceRequest {
        request_id: "service/power-1",
        target,
        expected_observation_generation: 1,
        expected_safe_disposition_generation: 8,
        deadline_tick: 200,
    }
}

fn verification<'a>(
    host: &'a HostId,
    boot: &'a BootId,
    state: CreatePowerState,
) -> CreatePowerVerification<'a> {
    CreatePowerVerification {
        host_id: host,
        boot_id: boot,
        offer_generation: OfferGeneration(6),
        implementation_id: CREATE_POWER_SERVICE_IMPLEMENTATION,
        robot_identity: "create/physical-1",
        attachment_id: CREATE_POWER_SERVICE_ATTACHMENT,
        observation: CreatePowerObservation {
            state,
            generation: 2,
            observed_at_tick: 116,
            maximum_age_ticks: 20,
        },
    }
}

fn pulsing(start: CreatePowerServiceStart) -> PreparedCreatePowerService {
    match start {
        CreatePowerServiceStart::Pulsing {
            execution,
            progress: CreatePowerServiceProgress::WaitingLowSettle { raise_at_tick: 105 },
        } => execution,
        _ => panic!("expected exact pulsing start"),
    }
}

#[test]
fn matching_fresh_truth_is_a_noop_without_gpio_writes() {
    let host = host();
    let boot = boot();
    for state in [CreatePowerState::Off, CreatePowerState::On] {
        let mut provider = provider();
        let start = start_create_power_service(
            &mut provider,
            binding(&host, &boot, state),
            request(state),
            Some(authority(&host, &boot)),
            100,
        )
        .unwrap();
        let CreatePowerServiceStart::NoOp(sign) = start else {
            panic!("matching power truth must not pulse")
        };
        assert!(!sign.pulse_emitted);
        assert_eq!(sign.observed_state, state);
        assert!(provider.levels.is_empty());
    }
}

#[test]
fn opposite_truth_emits_one_pulse_and_requires_fresh_matching_verification() {
    let host = host();
    let boot = boot();
    for (prior, target) in [
        (CreatePowerState::Off, CreatePowerState::On),
        (CreatePowerState::On, CreatePowerState::Off),
    ] {
        let mut provider = provider();
        let mut execution = pulsing(
            start_create_power_service(
                &mut provider,
                binding(&host, &boot, prior),
                request(target),
                Some(authority(&host, &boot)),
                100,
            )
            .unwrap(),
        );
        assert_eq!(provider.levels, [false]);
        assert_eq!(
            advance_create_power_service(&mut execution, &mut provider, 105),
            Ok(CreatePowerServiceProgress::WaitingHighPulse { lower_at_tick: 115 })
        );
        assert_eq!(
            advance_create_power_service(&mut execution, &mut provider, 115),
            Ok(CreatePowerServiceProgress::AwaitingFreshVerification)
        );
        assert_eq!(provider.levels, [false, true, false]);
        let sign =
            verify_create_power_service(&mut execution, verification(&host, &boot, target), 116)
                .unwrap();
        assert!(sign.pulse_emitted);
        assert_eq!((sign.prior_state, sign.observed_state), (prior, target));
        assert_eq!(sign.safe_disposition_generation, 8);
        assert_eq!(
            advance_create_power_service(&mut execution, &mut provider, 117),
            Err(CreatePowerServiceRefusal::InvalidServiceState)
        );
        assert_eq!(provider.levels, [false, true, false]);
    }
}

#[test]
fn unsafe_unknown_stale_and_unbound_requests_are_inert() {
    let host = host();
    let boot = boot();
    let assert_inert = |binding: CreatePowerServiceBinding<'_>,
                        request: CreatePowerServiceRequest<'_>,
                        authority: Option<CreatePowerServiceAuthority<'_>>,
                        expected| {
        let mut provider = provider();
        assert_eq!(
            start_create_power_service(&mut provider, binding, request, authority, 100)
                .err()
                .unwrap(),
            expected
        );
        assert!(provider.levels.is_empty());
    };
    assert_inert(
        binding(&host, &boot, CreatePowerState::Unknown),
        request(CreatePowerState::On),
        Some(authority(&host, &boot)),
        CreatePowerServiceRefusal::UnknownPower,
    );
    let mut unsafe_attachment = binding(&host, &boot, CreatePowerState::Off);
    unsafe_attachment.direct_untranslated_connection = true;
    assert_inert(
        unsafe_attachment,
        request(CreatePowerState::On),
        Some(authority(&host, &boot)),
        CreatePowerServiceRefusal::UnsafeElectricalAttachment,
    );
    let mut motion = binding(&host, &boot, CreatePowerState::Off);
    motion.motion_active = true;
    assert_inert(
        motion,
        request(CreatePowerState::On),
        Some(authority(&host, &boot)),
        CreatePowerServiceRefusal::MotionActive,
    );
    let mut stale = binding(&host, &boot, CreatePowerState::Off);
    stale.power.observed_at_tick = 79;
    assert_inert(
        stale,
        request(CreatePowerState::On),
        Some(authority(&host, &boot)),
        CreatePowerServiceRefusal::StalePowerObservation,
    );
    assert_inert(
        binding(&host, &boot, CreatePowerState::Off),
        request(CreatePowerState::On),
        None,
        CreatePowerServiceRefusal::MissingAuthority,
    );
    let mut wrong = authority(&host, &boot);
    wrong.offer_generation = OfferGeneration(7);
    assert_inert(
        binding(&host, &boot, CreatePowerState::Off),
        request(CreatePowerState::On),
        Some(wrong),
        CreatePowerServiceRefusal::OfferGenerationMismatch,
    );

    let mut translator = binding(&host, &boot, CreatePowerState::Off);
    translator.translator_enabled = false;
    assert_inert(
        translator,
        request(CreatePowerState::On),
        Some(authority(&host, &boot)),
        CreatePowerServiceRefusal::TranslatorUnavailable,
    );
    let mut not_idle = binding(&host, &boot, CreatePowerState::Off);
    not_idle.output_idle_low_observed = false;
    assert_inert(
        not_idle,
        request(CreatePowerState::On),
        Some(authority(&host, &boot)),
        CreatePowerServiceRefusal::OutputNotObservedIdleLow,
    );
    let mut wrong_generation = request(CreatePowerState::On);
    wrong_generation.expected_observation_generation = 2;
    assert_inert(
        binding(&host, &boot, CreatePowerState::Off),
        wrong_generation,
        Some(authority(&host, &boot)),
        CreatePowerServiceRefusal::PowerObservationGenerationMismatch,
    );
    let mut wrong_safe = request(CreatePowerState::On);
    wrong_safe.expected_safe_disposition_generation = 9;
    assert_inert(
        binding(&host, &boot, CreatePowerState::Off),
        wrong_safe,
        Some(authority(&host, &boot)),
        CreatePowerServiceRefusal::SafeDispositionGenerationMismatch,
    );
    let mut invalid_profile = binding(&host, &boot, CreatePowerState::Off);
    invalid_profile.pulse_profile.high_pulse_ticks = 0;
    assert_inert(
        invalid_profile,
        request(CreatePowerState::On),
        Some(authority(&host, &boot)),
        CreatePowerServiceRefusal::Pulse(CreatePowerPulseFailure::InvalidProfile),
    );
}

#[test]
fn provider_loss_and_bad_verification_are_terminal_without_another_pulse() {
    let host = host();
    let boot = boot();
    let mut lost_provider = provider();
    let mut execution = pulsing(
        start_create_power_service(
            &mut lost_provider,
            binding(&host, &boot, CreatePowerState::Off),
            request(CreatePowerState::On),
            Some(authority(&host, &boot)),
            100,
        )
        .unwrap(),
    );
    advance_create_power_service(&mut execution, &mut lost_provider, 105).unwrap();
    lost_provider.available = false;
    assert_eq!(
        advance_create_power_service(&mut execution, &mut lost_provider, 115),
        Err(CreatePowerServiceRefusal::Pulse(
            CreatePowerPulseFailure::ProviderUnavailable
        ))
    );
    lost_provider.available = true;
    assert_eq!(
        advance_create_power_service(&mut execution, &mut lost_provider, 116),
        Err(CreatePowerServiceRefusal::InvalidServiceState)
    );
    assert_eq!(lost_provider.levels, [false, true]);

    let mut provider = provider();
    let mut execution = pulsing(
        start_create_power_service(
            &mut provider,
            binding(&host, &boot, CreatePowerState::Off),
            request(CreatePowerState::On),
            Some(authority(&host, &boot)),
            100,
        )
        .unwrap(),
    );
    advance_create_power_service(&mut execution, &mut provider, 105).unwrap();
    advance_create_power_service(&mut execution, &mut provider, 115).unwrap();
    assert_eq!(
        verify_create_power_service(
            &mut execution,
            verification(&host, &boot, CreatePowerState::Off),
            116
        ),
        Err(CreatePowerServiceRefusal::VerificationMismatch)
    );
    assert_eq!(provider.levels, [false, true, false]);
}

#[test]
fn verification_generation_freshness_and_identity_fail_terminally() {
    let host = host();
    let boot = boot();
    for expected in [
        CreatePowerServiceRefusal::VerificationGenerationDidNotAdvance,
        CreatePowerServiceRefusal::VerificationStale,
        CreatePowerServiceRefusal::HostMismatch,
    ] {
        let mut provider = provider();
        let mut execution = pulsing(
            start_create_power_service(
                &mut provider,
                binding(&host, &boot, CreatePowerState::Off),
                request(CreatePowerState::On),
                Some(authority(&host, &boot)),
                100,
            )
            .unwrap(),
        );
        advance_create_power_service(&mut execution, &mut provider, 105).unwrap();
        advance_create_power_service(&mut execution, &mut provider, 115).unwrap();
        let other_host = HostId::from("host/other");
        let mut value = verification(&host, &boot, CreatePowerState::On);
        match expected {
            CreatePowerServiceRefusal::VerificationGenerationDidNotAdvance => {
                value.observation.generation = 1;
            }
            CreatePowerServiceRefusal::VerificationStale => {
                value.observation.observed_at_tick = 90;
                value.observation.maximum_age_ticks = 20;
            }
            CreatePowerServiceRefusal::HostMismatch => value.host_id = &other_host,
            _ => unreachable!(),
        }
        assert_eq!(
            verify_create_power_service(&mut execution, value, 116),
            Err(expected)
        );
        assert_eq!(
            verify_create_power_service(
                &mut execution,
                verification(&host, &boot, CreatePowerState::On),
                117,
            ),
            Err(CreatePowerServiceRefusal::InvalidServiceState)
        );
        assert_eq!(provider.levels, [false, true, false]);
    }
}
