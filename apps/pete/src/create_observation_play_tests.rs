use super::*;
use crate::{
    live_create_observation_advertisement, CreateObservationEncodeRefusal,
    CreateObservationFailure, OiMode, UartProfile, CREATE_ODOMETRY_RESET_AUTHORITY,
};
use conduit_core::{BootId, ConnectionBase, HostId, OfferGeneration};
use std::collections::VecDeque;

const FORM: &str = "form contact_sample {\n contact: robotics/observe-contact\n}\n";
const BEACON_FORM: &str = "form beacon_sample {\n beacon: robotics/observe-beacon\n}\n";
const ODOMETRY_FORM: &str = "form odometry_sample {\n odometry: robotics/observe-odometry\n}\n";

struct Provider {
    available: bool,
    writes: Vec<Vec<u8>>,
    read: VecDeque<u8>,
}

impl CreateUartProvider for Provider {
    type Error = ();

    fn is_available(&self) -> bool {
        self.available
    }
    fn profile(&self) -> UartProfile {
        UartProfile::CREATE_OI
    }
    fn write_all(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
        self.writes.push(bytes.to_vec());
        Ok(())
    }
    fn read_byte(&mut self, _: u64) -> Result<Option<u8>, Self::Error> {
        Ok(self.read.pop_front())
    }
}

fn evidence() -> CreateObservationEvidence {
    CreateObservationEvidence {
        host_id: HostId::from("host/std-create"),
        boot_id: BootId::from("boot/1"),
        offer_generation: OfferGeneration(7),
        serial_base_id: "base/tty-create".into(),
        robot_identity: "create/serial-1".into(),
        session_resource_id: "session/create-1".into(),
        mode: OiMode::Safe,
        observed_at_tick: 90,
        maximum_age_ticks: 20,
    }
}

fn plan_for(source: &str, form_name: &str) -> Plan {
    let (startup, profile) = crate::catalogs().unwrap();
    let syntax = conduit_form::parse_syntax_document(source);
    let checked = conduit_form::check_syntax_document(&syntax, &startup).unwrap();
    let expanded = conduit_form::expand_canonical_form(&checked, form_name, &profile).unwrap();
    let host = live_create_observation_advertisement(&evidence(), 100).unwrap();
    let placements =
        conduit_planner::default_expanded_placements(&expanded, std::slice::from_ref(&host))
            .unwrap();
    conduit_planner::plan_expanded_canonical(
        &expanded,
        &[host],
        &placements,
        &[ConnectionBase::Local],
    )
    .unwrap()
}

fn plan() -> Plan {
    plan_for(FORM, "contact_sample")
}

fn frame(contact: u8, include_virtual_wall: bool) -> Vec<u8> {
    frame_with_delta(contact, include_virtual_wall, 0, 0)
}

fn frame_with_delta(
    contact: u8,
    include_virtual_wall: bool,
    distance_delta_mm: i16,
    angle_delta_degrees: i16,
) -> Vec<u8> {
    let mut group = [0_u8; 26];
    group[0] = contact;
    group[6] = u8::from(include_virtual_wall);
    group[12..14].copy_from_slice(&distance_delta_mm.to_be_bytes());
    group[14..16].copy_from_slice(&angle_delta_degrees.to_be_bytes());
    group[16] = 3;
    group[17..19].copy_from_slice(&14_000_u16.to_be_bytes());
    group[19..21].copy_from_slice(&100_i16.to_be_bytes());
    group[22..24].copy_from_slice(&1_000_u16.to_be_bytes());
    group[24..26].copy_from_slice(&2_000_u16.to_be_bytes());
    let mut frame = vec![19, 29, 0];
    frame.extend_from_slice(&group);
    frame.extend_from_slice(&[34, 0]);
    let sum = frame.iter().fold(0_u8, |sum, byte| sum.wrapping_add(*byte));
    frame.push(0_u8.wrapping_sub(sum));
    frame
}

fn provider(bytes: &[u8]) -> Provider {
    Provider {
        available: true,
        writes: Vec::new(),
        read: bytes.iter().copied().collect(),
    }
}

#[test]
fn production_kernel_dispatches_correlated_session_and_delivers_canonical_value() {
    let mut execution = prepare_create_observation_execution(
        &plan(),
        CreateObservationChannel::Contact,
        &evidence(),
    )
    .unwrap();
    let mut provider = provider(&frame(0b11, false));
    let report = run_create_observation_execution(&mut execution, &mut provider, 105, 100, 100);
    assert_eq!(report.terminal, CreateObservationTerminal::Completed);
    assert_eq!(report.canonical_value_len, 1);
    assert_eq!(
        report.canonical_value[0],
        conduit_core::BODY_SECTOR_FRONT_LEFT | conduit_core::BODY_SECTOR_FRONT_RIGHT
    );
    assert_eq!(report.observation_generation, Some(1));
    assert_eq!(report.serial_base_id, "base/tty-create");
    assert_eq!(report.robot_identity, "create/serial-1");
    assert_eq!(
        provider.writes,
        [vec![128], vec![131], vec![148, 2, 0, 34], vec![150, 0]]
    );
    assert!(report.kernel_decisions > 0);
    assert!(report.kernel_signs > 0);
}

#[test]
fn no_response_stale_and_absent_optional_value_remain_distinct() {
    let mut silent_execution = prepare_create_observation_execution(
        &plan(),
        CreateObservationChannel::Contact,
        &evidence(),
    )
    .unwrap();
    let mut silent = provider(&[]);
    let silent_report =
        run_create_observation_execution(&mut silent_execution, &mut silent, 105, 100, 100);
    assert_eq!(
        silent_report.terminal,
        CreateObservationTerminal::Failed(CreateObservationExecutionFailure::Session(
            CreateObservationFailure::Protocol(crate::CreateOiFailure::DeviceNoResponse)
        ))
    );

    let mut stale_execution = prepare_create_observation_execution(
        &plan(),
        CreateObservationChannel::Contact,
        &evidence(),
    )
    .unwrap();
    let mut stale = provider(&frame(0, false));
    let stale_report =
        run_create_observation_execution(&mut stale_execution, &mut stale, 105, 100, 121);
    assert_eq!(
        stale_report.terminal,
        CreateObservationTerminal::Failed(CreateObservationExecutionFailure::Encoding(
            CreateObservationEncodeRefusal::StaleObservation
        ))
    );

    let mut absent_execution = prepare_create_observation_execution(
        &plan_for(BEACON_FORM, "beacon_sample"),
        CreateObservationChannel::Infrared,
        &evidence(),
    )
    .unwrap();
    let mut absent = provider(&frame(0, false));
    let absent_report =
        run_create_observation_execution(&mut absent_execution, &mut absent, 105, 100, 100);
    assert_eq!(
        absent_report.terminal,
        CreateObservationTerminal::Failed(CreateObservationExecutionFailure::MissingCurrentValue)
    );
}

#[test]
fn stale_plan_identity_and_pre_dispatch_cancellation_refuse_without_uart_bytes() {
    let mut wrong = evidence();
    wrong.boot_id = BootId::from("boot/stale");
    assert!(prepare_create_observation_execution(
        &plan(),
        CreateObservationChannel::Contact,
        &wrong
    )
    .is_err());

    let mut execution = prepare_create_observation_execution(
        &plan(),
        CreateObservationChannel::Contact,
        &evidence(),
    )
    .unwrap();
    let report = cancel_create_observation_execution(&mut execution);
    assert_eq!(
        report.terminal,
        CreateObservationTerminal::CancelledBeforeDispatch
    );

    let mut dispatched = prepare_create_observation_execution(
        &plan(),
        CreateObservationChannel::Contact,
        &evidence(),
    )
    .unwrap();
    let mut provider = provider(&frame(0, false));
    dispatch_create_observation_execution(&mut dispatched, &mut provider, 105, 100, 100).unwrap();
    let report = cancel_create_observation_execution(&mut dispatched);
    assert_eq!(
        report.terminal,
        CreateObservationTerminal::CancelledAfterDispatch
    );
    assert_eq!(report.observation_generation, Some(1));
    assert_eq!(provider.writes.last().unwrap(), &[150, 0]);
}

#[test]
fn malformed_truncated_and_lost_provider_keep_exact_protocol_failures() {
    let run = |mut provider: Provider| {
        let mut execution = prepare_create_observation_execution(
            &plan(),
            CreateObservationChannel::Contact,
            &evidence(),
        )
        .unwrap();
        run_create_observation_execution(&mut execution, &mut provider, 105, 100, 100).terminal
    };

    let mut malformed = frame(0, false);
    malformed[3] ^= 1;
    assert_eq!(
        run(provider(&malformed)),
        CreateObservationTerminal::Failed(CreateObservationExecutionFailure::Session(
            CreateObservationFailure::Protocol(crate::CreateOiFailure::MalformedFrame)
        ))
    );
    assert_eq!(
        run(provider(&frame(0, false)[..5])),
        CreateObservationTerminal::Failed(CreateObservationExecutionFailure::Session(
            CreateObservationFailure::Protocol(crate::CreateOiFailure::TruncatedFrame)
        ))
    );
    let mut lost = provider(&frame(0, false));
    lost.available = false;
    assert_eq!(
        run(lost),
        CreateObservationTerminal::Failed(CreateObservationExecutionFailure::Session(
            CreateObservationFailure::Protocol(crate::CreateOiFailure::ProviderUnavailable)
        ))
    );
}

#[test]
fn odometry_state_crosses_sessions_and_reset_advances_the_exact_frame() {
    let evidence = evidence();
    let plan = plan_for(ODOMETRY_FORM, "odometry_sample");
    let mut first =
        prepare_create_observation_execution(&plan, CreateObservationChannel::Odometry, &evidence)
            .unwrap();
    let mut first_provider = provider(&frame_with_delta(0, false, 100, 90));
    let first_report =
        run_create_observation_execution(&mut first, &mut first_provider, 105, 100, 100);
    assert_eq!(first_report.terminal, CreateObservationTerminal::Completed);
    assert_eq!(first_report.odometry_frame_generation, Some(1));
    assert_eq!(first_report.odometry_sample_generation, Some(1));
    assert_eq!(
        conduit_core::OdometryObservation::decode(
            &first_report.canonical_value[..usize::from(first_report.canonical_value_len)]
        )
        .unwrap()
        .components(),
        (71, 71, 1_570_797)
    );

    let retained = create_observation_odometry_state(&first).unwrap();
    let mut second = prepare_create_observation_execution_with_odometry(
        &plan,
        CreateObservationChannel::Odometry,
        &evidence,
        retained,
    )
    .unwrap();
    let mut second_provider = provider(&frame_with_delta(0, false, 100, 0));
    let second_report =
        run_create_observation_execution(&mut second, &mut second_provider, 115, 110, 110);
    assert_eq!(second_report.odometry_sample_generation, Some(2));
    assert_eq!(
        conduit_core::OdometryObservation::decode(
            &second_report.canonical_value[..usize::from(second_report.canonical_value_len)]
        )
        .unwrap()
        .components(),
        (71, 171, 1_570_797)
    );

    let retained = create_observation_odometry_state(&second).unwrap();
    let mut reset_execution = prepare_create_observation_execution_with_odometry(
        &plan,
        CreateObservationChannel::Odometry,
        &evidence,
        retained,
    )
    .unwrap();
    let sign = reset_create_observation_odometry(
        &mut reset_execution,
        CreateOdometryResetRequest {
            request_id: "reset/std-odometry-1",
            expected_frame_generation: 1,
        },
        Some(CreateOdometryResetAuthority {
            grant_id: CREATE_ODOMETRY_RESET_AUTHORITY,
            host_id: &evidence.host_id,
            boot_id: &evidence.boot_id,
            offer_generation: evidence.offer_generation,
            implementation_id: CreateObservationChannel::Odometry.implementation_id(),
            valid_until_tick: 200,
        }),
        120,
    )
    .unwrap();
    assert_eq!(sign.current_frame_generation, 2);
    let mut reset_provider = provider(&frame_with_delta(0, false, 10, 0));
    let reset_report =
        run_create_observation_execution(&mut reset_execution, &mut reset_provider, 130, 125, 125);
    assert_eq!(reset_report.odometry_frame_generation, Some(2));
    assert_eq!(reset_report.odometry_sample_generation, Some(1));
    assert_eq!(
        conduit_core::OdometryObservation::decode(
            &reset_report.canonical_value[..usize::from(reset_report.canonical_value_len)]
        )
        .unwrap()
        .components(),
        (10, 0, 0)
    );
}

#[test]
fn odometry_reset_refuses_wrong_channel_and_dispatched_execution() {
    let evidence = evidence();
    let mut contact =
        prepare_create_observation_execution(&plan(), CreateObservationChannel::Contact, &evidence)
            .unwrap();
    let request = CreateOdometryResetRequest {
        request_id: "reset/wrong-channel",
        expected_frame_generation: 1,
    };
    assert_eq!(
        reset_create_observation_odometry(&mut contact, request, None, 100),
        Err(CreateObservationOdometryResetRefusal::WrongChannel)
    );

    let plan = plan_for(ODOMETRY_FORM, "odometry_sample");
    let mut odometry =
        prepare_create_observation_execution(&plan, CreateObservationChannel::Odometry, &evidence)
            .unwrap();
    let mut provider = provider(&frame_with_delta(0, false, 1, 0));
    let report = run_create_observation_execution(&mut odometry, &mut provider, 105, 100, 100);
    assert_eq!(report.terminal, CreateObservationTerminal::Completed);
    assert_eq!(
        reset_create_observation_odometry(&mut odometry, request, None, 101),
        Err(CreateObservationOdometryResetRefusal::ObservationInFlightOrFinished)
    );
}
