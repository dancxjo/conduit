use super::*;
use crate::{UartProfile, CREATE_OI_MAX_WHEEL_SPEED_MM_S};
use std::collections::VecDeque;

struct Provider {
    available: bool,
    profile: UartProfile,
    writes: Vec<Vec<u8>>,
    read: VecDeque<u8>,
    fail_write: Option<usize>,
    fail_read: bool,
}

impl CreateUartProvider for Provider {
    type Error = ();

    fn is_available(&self) -> bool {
        self.available
    }

    fn profile(&self) -> UartProfile {
        self.profile
    }

    fn write_all(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
        if self.fail_write == Some(self.writes.len()) {
            return Err(());
        }
        self.writes.push(bytes.to_vec());
        Ok(())
    }

    fn read_byte(&mut self, _: u64) -> Result<Option<u8>, Self::Error> {
        if self.fail_read {
            return Err(());
        }
        Ok(self.read.pop_front())
    }
}

fn provider(mode: OiMode) -> Provider {
    Provider {
        available: true,
        profile: UartProfile::CREATE_OI,
        writes: Vec::new(),
        read: VecDeque::from([mode_byte(mode)]),
        fail_write: None,
        fail_read: false,
    }
}

fn mode_byte(mode: OiMode) -> u8 {
    match mode {
        OiMode::Off => 0,
        OiMode::Passive => 1,
        OiMode::Safe => 2,
        OiMode::Full => 3,
    }
}

fn host() -> HostId {
    HostId::from("host/pete")
}

fn boot() -> BootId {
    BootId::from("boot/pete-1")
}

fn binding<'a>(host: &'a HostId, boot: &'a BootId) -> CreateModeServiceBinding<'a> {
    CreateModeServiceBinding {
        host_id: host,
        boot_id: boot,
        offer_generation: OfferGeneration(9),
        implementation_id: CREATE_MODE_SERVICE_IMPLEMENTATION,
        serial_base_id: "base/create-uart-1",
        robot_identity: "create/physical-1",
        robot_identity_verified: true,
        service_attachment_id: CREATE_MODE_SERVICE_ATTACHMENT,
        current_mode: OiMode::Safe,
        mode_observation_generation: 4,
        observed_at_tick: 90,
        maximum_age_ticks: 20,
    }
}

fn authority<'a>(host: &'a HostId, boot: &'a BootId) -> CreateModeServiceAuthority<'a> {
    CreateModeServiceAuthority {
        grant_id: CREATE_MODE_SERVICE_AUTHORITY,
        host_id: host,
        boot_id: boot,
        offer_generation: OfferGeneration(9),
        implementation_id: CREATE_MODE_SERVICE_IMPLEMENTATION,
        robot_identity: "create/physical-1",
        service_attachment_id: CREATE_MODE_SERVICE_ATTACHMENT,
        valid_until_tick: 200,
    }
}

fn request(target_mode: OiMode) -> CreateModeServiceRequest<'static> {
    CreateModeServiceRequest {
        request_id: "service/mode-1",
        expected_current_mode: OiMode::Safe,
        expected_mode_observation_generation: 4,
        target_mode,
        deadline_tick: 150,
    }
}

#[test]
fn passive_safe_and_full_are_stop_first_exact_and_verified() {
    let host = host();
    let boot = boot();
    for (mode, expected_writes) in [
        (
            OiMode::Passive,
            vec![vec![145, 0, 0, 0, 0], vec![128], vec![142, 35]],
        ),
        (
            OiMode::Safe,
            vec![
                vec![145, 0, 0, 0, 0],
                vec![128],
                vec![131],
                vec![145, 0, 0, 0, 0],
                vec![142, 35],
            ],
        ),
        (
            OiMode::Full,
            vec![
                vec![145, 0, 0, 0, 0],
                vec![128],
                vec![132],
                vec![145, 0, 0, 0, 0],
                vec![142, 35],
            ],
        ),
    ] {
        let mut provider = provider(mode);
        let sign = transition_create_mode(
            &mut provider,
            binding(&host, &boot),
            request(mode),
            Some(authority(&host, &boot)),
            100,
        )
        .unwrap();
        assert_eq!(provider.writes, expected_writes);
        assert_eq!(sign.prior_mode, OiMode::Safe);
        assert_eq!(sign.observed_mode, mode);
        assert_eq!(sign.offer_generation, OfferGeneration(9));
        assert_eq!(sign.robot_identity, "create/physical-1");
    }
    assert_eq!(CREATE_OI_MAX_WHEEL_SPEED_MM_S, 500);
}

#[test]
fn authority_identity_and_deadline_mutations_emit_no_bytes() {
    let host = host();
    let boot = boot();
    let other_host = HostId::from("host/other");
    let mut cases = Vec::new();
    cases.push((None, CreateModeServiceRefusal::MissingAuthority));
    let mut wrong_host = authority(&host, &boot);
    wrong_host.host_id = &other_host;
    cases.push((Some(wrong_host), CreateModeServiceRefusal::HostMismatch));
    let mut stale_offer = authority(&host, &boot);
    stale_offer.offer_generation = OfferGeneration(8);
    cases.push((
        Some(stale_offer),
        CreateModeServiceRefusal::OfferGenerationMismatch,
    ));
    let mut wrong_robot = authority(&host, &boot);
    wrong_robot.robot_identity = "create/other";
    cases.push((
        Some(wrong_robot),
        CreateModeServiceRefusal::RobotIdentityMismatch,
    ));
    for (authority, expected) in cases {
        let mut provider = provider(OiMode::Safe);
        assert_eq!(
            transition_create_mode(
                &mut provider,
                binding(&host, &boot),
                request(OiMode::Safe),
                authority,
                100,
            ),
            Err(expected)
        );
        assert!(provider.writes.is_empty());
    }

    let mut expired_provider = provider(OiMode::Safe);
    let mut expired = authority(&host, &boot);
    expired.valid_until_tick = 100;
    assert_eq!(
        transition_create_mode(
            &mut expired_provider,
            binding(&host, &boot),
            request(OiMode::Safe),
            Some(expired),
            100,
        ),
        Err(CreateModeServiceRefusal::AuthorityExpired)
    );
    assert!(expired_provider.writes.is_empty());

    let assert_inert = |binding: CreateModeServiceBinding<'_>,
                        request: CreateModeServiceRequest<'_>,
                        authority: Option<CreateModeServiceAuthority<'_>>,
                        now_tick,
                        expected| {
        let mut provider = provider(OiMode::Safe);
        assert_eq!(
            transition_create_mode(&mut provider, binding, request, authority, now_tick),
            Err(expected)
        );
        assert!(provider.writes.is_empty());
    };

    let mut wrong_boot = authority(&host, &boot);
    let other_boot = BootId::from("boot/other");
    wrong_boot.boot_id = &other_boot;
    assert_inert(
        binding(&host, &boot),
        request(OiMode::Safe),
        Some(wrong_boot),
        100,
        CreateModeServiceRefusal::BootMismatch,
    );
    let mut wrong_implementation = authority(&host, &boot);
    wrong_implementation.implementation_id = "pete/other-mode-service@1";
    assert_inert(
        binding(&host, &boot),
        request(OiMode::Safe),
        Some(wrong_implementation),
        100,
        CreateModeServiceRefusal::ImplementationMismatch,
    );
    let mut wrong_attachment = authority(&host, &boot);
    wrong_attachment.service_attachment_id = "service/other";
    assert_inert(
        binding(&host, &boot),
        request(OiMode::Safe),
        Some(wrong_attachment),
        100,
        CreateModeServiceRefusal::ServiceAttachmentMismatch,
    );
    let mut too_short = authority(&host, &boot);
    too_short.valid_until_tick = 149;
    assert_inert(
        binding(&host, &boot),
        request(OiMode::Safe),
        Some(too_short),
        100,
        CreateModeServiceRefusal::OperationOutlivesAuthority,
    );
    let mut stale = binding(&host, &boot);
    stale.observed_at_tick = 79;
    assert_inert(
        stale,
        request(OiMode::Safe),
        Some(authority(&host, &boot)),
        100,
        CreateModeServiceRefusal::StaleModeObservation,
    );
    let mut wrong_generation = request(OiMode::Safe);
    wrong_generation.expected_mode_observation_generation = 3;
    assert_inert(
        binding(&host, &boot),
        wrong_generation,
        Some(authority(&host, &boot)),
        100,
        CreateModeServiceRefusal::ModeObservationGenerationMismatch,
    );
    let mut wrong_mode = request(OiMode::Safe);
    wrong_mode.expected_current_mode = OiMode::Full;
    assert_inert(
        binding(&host, &boot),
        wrong_mode,
        Some(authority(&host, &boot)),
        100,
        CreateModeServiceRefusal::CurrentModeMismatch,
    );
}

#[test]
fn stop_transition_query_and_observation_fail_at_distinct_stages() {
    let host = host();
    let boot = boot();
    for (failed_write, stage, prior_writes) in [
        (0, CreateOiModeTransitionStage::MandatoryStop, 0),
        (1, CreateOiModeTransitionStage::ModeTransition, 1),
        (2, CreateOiModeTransitionStage::ModeTransition, 2),
        (3, CreateOiModeTransitionStage::MandatoryStop, 3),
        (4, CreateOiModeTransitionStage::VerificationQuery, 4),
    ] {
        let mut provider = provider(OiMode::Safe);
        provider.fail_write = Some(failed_write);
        assert_eq!(
            transition_create_mode(
                &mut provider,
                binding(&host, &boot),
                request(OiMode::Safe),
                Some(authority(&host, &boot)),
                100,
            ),
            Err(CreateModeServiceRefusal::Protocol {
                stage,
                failure: CreateOiFailure::WriteFailed,
            })
        );
        assert_eq!(provider.writes.len(), prior_writes);
    }

    let mut silent = provider(OiMode::Safe);
    silent.read.clear();
    assert_eq!(
        transition_create_mode(
            &mut silent,
            binding(&host, &boot),
            request(OiMode::Safe),
            Some(authority(&host, &boot)),
            100,
        ),
        Err(CreateModeServiceRefusal::Protocol {
            stage: CreateOiModeTransitionStage::VerificationRead,
            failure: CreateOiFailure::DeviceNoResponse,
        })
    );

    let mut mismatch = provider(OiMode::Passive);
    assert_eq!(
        transition_create_mode(
            &mut mismatch,
            binding(&host, &boot),
            request(OiMode::Full),
            Some(authority(&host, &boot)),
            100,
        ),
        Err(CreateModeServiceRefusal::ModeMismatch {
            requested: OiMode::Full,
            observed: OiMode::Passive,
        })
    );

    let mut absent = provider(OiMode::Safe);
    absent.available = false;
    assert_eq!(
        transition_create_mode(
            &mut absent,
            binding(&host, &boot),
            request(OiMode::Safe),
            Some(authority(&host, &boot)),
            100,
        ),
        Err(CreateModeServiceRefusal::Protocol {
            stage: CreateOiModeTransitionStage::MandatoryStop,
            failure: CreateOiFailure::ProviderUnavailable,
        })
    );
    let mut wrong_profile = provider(OiMode::Safe);
    wrong_profile.profile.baud = 115_200;
    assert!(matches!(
        transition_create_mode(
            &mut wrong_profile,
            binding(&host, &boot),
            request(OiMode::Safe),
            Some(authority(&host, &boot)),
            100,
        ),
        Err(CreateModeServiceRefusal::Protocol {
            stage: CreateOiModeTransitionStage::MandatoryStop,
            failure: CreateOiFailure::WrongUartProfile { .. },
        })
    ));
    let mut read_failed = provider(OiMode::Safe);
    read_failed.fail_read = true;
    assert_eq!(
        transition_create_mode(
            &mut read_failed,
            binding(&host, &boot),
            request(OiMode::Safe),
            Some(authority(&host, &boot)),
            100,
        ),
        Err(CreateModeServiceRefusal::Protocol {
            stage: CreateOiModeTransitionStage::VerificationRead,
            failure: CreateOiFailure::ReadFailed,
        })
    );
    let mut malformed = provider(OiMode::Safe);
    malformed.read = VecDeque::from([4]);
    assert_eq!(
        transition_create_mode(
            &mut malformed,
            binding(&host, &boot),
            request(OiMode::Safe),
            Some(authority(&host, &boot)),
            100,
        ),
        Err(CreateModeServiceRefusal::Protocol {
            stage: CreateOiModeTransitionStage::VerificationRead,
            failure: CreateOiFailure::MalformedFrame,
        })
    );
}
