use super::*;
use crate::{
    create_dock_plan, IndependentWatchdogObservation, OiMode, SafetyInputObservation, UartProfile,
    CREATE_DOCK_GRANT, CREATE_OI_BAUD,
};
use conduit_core::{BootId, HostId, OfferGeneration};

fn reduced_safety(generation: u32, observed_at_tick: u64) -> SafetyObservation {
    SafetyObservation {
        generation,
        observed_at_tick,
        maximum_age_ticks: 60_000,
        emergency_stop: SafetyInputObservation::Unavailable,
        wheel_drop: false,
        cliff: false,
        contact: false,
        tilt: SafetyInputObservation::Unavailable,
        impact: SafetyInputObservation::Unavailable,
        charging: false,
        control_alive: true,
        body_link_alive: true,
        independent_watchdog: IndependentWatchdogObservation::Absent,
    }
}

fn observation() -> CreateDockObservation {
    CreateDockObservation {
        host_id: HostId::from("std/create-dock"),
        boot_id: BootId::from("std/create-dock-boot"),
        offer_generation: OfferGeneration(3),
        serial_base_id: "std/create-uart/0".into(),
        robot_identity: "robot/create1/0".into(),
        robot_identity_verified: true,
        dock_resource_id: "robot/create1/0/dock".into(),
        timer_resource_id: "std/timer/create-dock".into(),
        mode: OiMode::Safe,
        safety: reduced_safety(7, 100),
    }
}

#[derive(Default)]
struct FakeProvider {
    available: bool,
    writes: Vec<Vec<u8>>,
}

impl FakeProvider {
    fn ready() -> Self {
        Self {
            available: true,
            writes: Vec::new(),
        }
    }
}

impl CreateUartProvider for FakeProvider {
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

    fn read_byte(&mut self, _deadline_tick: u64) -> Result<Option<u8>, Self::Error> {
        Ok(None)
    }
}

fn authority<'a>() -> MotionAuthority<'a> {
    MotionAuthority {
        grant_id: CREATE_DOCK_GRANT,
        valid_until_tick: 40_000,
        safety_class: MotionSafetyAuthority::ReducedWheelsOffFloor,
    }
}

fn prepared() -> PreparedCreateDockExecution {
    let evidence = observation();
    let plan = create_dock_plan(&evidence, 100, true).unwrap();
    prepare_create_dock_execution(&plan, &evidence).unwrap()
}

#[test]
fn production_kernel_dispatches_seek_then_charging_completion_stops() {
    assert_eq!(CREATE_OI_BAUD, 57_600);
    let mut execution = prepared();
    let mut provider = FakeProvider::ready();
    let dispatch = dispatch_create_dock_execution(
        &mut execution,
        &mut provider,
        100,
        Some(authority()),
        reduced_safety(7, 100),
    );
    assert!(matches!(
        dispatch.terminal,
        CreateDockExecutionTerminal::Docking {
            deadline_tick: 30_100,
            ..
        }
    ));
    assert_eq!(provider.writes, vec![vec![143]]);

    let mut docked = reduced_safety(8, 110);
    docked.charging = true;
    let complete = supervise_create_dock_execution(&mut execution, &mut provider, 110, docked);
    assert!(matches!(
        complete.terminal,
        CreateDockExecutionTerminal::Docked { .. }
    ));
    assert_eq!(provider.writes, vec![vec![143], vec![145, 0, 0, 0, 0]]);
    assert!(complete.kernel_decisions > 0);
    assert!(complete.kernel_signs > 0);
}

#[test]
fn timeout_and_cancellation_both_emit_the_mandatory_stop() {
    let mut timed = prepared();
    let mut timed_provider = FakeProvider::ready();
    dispatch_create_dock_execution(
        &mut timed,
        &mut timed_provider,
        100,
        Some(authority()),
        reduced_safety(7, 100),
    );
    let report = supervise_create_dock_execution(
        &mut timed,
        &mut timed_provider,
        30_100,
        reduced_safety(8, 30_100),
    );
    assert!(matches!(
        report.terminal,
        CreateDockExecutionTerminal::TimedOut {
            safe_disposition: DockSafeDisposition::Verified,
            ..
        }
    ));

    let mut cancelled = prepared();
    let mut cancelled_provider = FakeProvider::ready();
    dispatch_create_dock_execution(
        &mut cancelled,
        &mut cancelled_provider,
        100,
        Some(authority()),
        reduced_safety(7, 100),
    );
    let report = cancel_create_dock_execution(&mut cancelled, &mut cancelled_provider, 7);
    assert!(matches!(
        report.terminal,
        CreateDockExecutionTerminal::Cancelled {
            safe_disposition: DockSafeDisposition::Verified,
            ..
        }
    ));
    assert_eq!(timed_provider.writes, cancelled_provider.writes);
}

#[test]
fn provider_loss_during_terminal_stop_is_never_safe_success() {
    let mut execution = prepared();
    let mut provider = FakeProvider::ready();
    dispatch_create_dock_execution(
        &mut execution,
        &mut provider,
        100,
        Some(authority()),
        reduced_safety(7, 100),
    );
    provider.available = false;
    let mut docked = reduced_safety(8, 110);
    docked.charging = true;
    let report = supervise_create_dock_execution(&mut execution, &mut provider, 110, docked);
    assert!(matches!(
        report.terminal,
        CreateDockExecutionTerminal::ChargingObservedButStopFailed {
            failure: CreateOiFailure::ProviderUnavailable,
            ..
        }
    ));
}

#[test]
fn provider_absence_and_fresh_hazard_fail_at_distinct_seams() {
    let mut unavailable_execution = prepared();
    let mut unavailable_provider = FakeProvider::default();
    let unavailable = dispatch_create_dock_execution(
        &mut unavailable_execution,
        &mut unavailable_provider,
        100,
        Some(authority()),
        reduced_safety(7, 100),
    );
    assert!(matches!(
        unavailable.terminal,
        CreateDockExecutionTerminal::Refused(CreateDockExecutionRefusal::Device(
            CreateOiFailure::ProviderUnavailable
        ))
    ));
    assert!(unavailable_provider.writes.is_empty());

    let mut hazard_execution = prepared();
    let mut hazard_provider = FakeProvider::ready();
    dispatch_create_dock_execution(
        &mut hazard_execution,
        &mut hazard_provider,
        100,
        Some(authority()),
        reduced_safety(7, 100),
    );
    let mut hazardous = reduced_safety(8, 110);
    hazardous.wheel_drop = true;
    let hazard = supervise_create_dock_execution(
        &mut hazard_execution,
        &mut hazard_provider,
        110,
        hazardous,
    );
    assert!(matches!(
        hazard.terminal,
        CreateDockExecutionTerminal::SafetyInhibited {
            hazard: LocalHazard::WheelDrop,
            safe_disposition: DockSafeDisposition::Verified,
            ..
        }
    ));
    assert_eq!(
        hazard_provider.writes,
        vec![vec![143], vec![145, 0, 0, 0, 0]]
    );
}

#[test]
fn missing_authority_and_pre_dispatch_cancel_emit_no_device_bytes() {
    let mut refused_execution = prepared();
    let mut refused_provider = FakeProvider::ready();
    let refused = dispatch_create_dock_execution(
        &mut refused_execution,
        &mut refused_provider,
        100,
        None,
        reduced_safety(7, 100),
    );
    assert!(matches!(
        refused.terminal,
        CreateDockExecutionTerminal::Refused(CreateDockExecutionRefusal::MissingAuthority)
    ));
    assert!(refused_provider.writes.is_empty());

    let mut cancelled_execution = prepared();
    let mut cancelled_provider = FakeProvider::ready();
    let cancelled =
        cancel_create_dock_execution(&mut cancelled_execution, &mut cancelled_provider, 7);
    assert_eq!(
        cancelled.terminal,
        CreateDockExecutionTerminal::CancelledBeforeDispatch
    );
    assert!(cancelled_provider.writes.is_empty());
}

#[test]
fn dock_authority_is_only_seek_dock_and_zero_output() {
    assert!(dock_authority_admits(&[143]));
    assert!(dock_authority_admits(&[145, 0, 0, 0, 0]));
    for refused in [
        &[128][..],
        &[131],
        &[139, 0, 0, 255],
        &[140, 0, 1, 60, 32],
        &[145, 0, 50, 0, 50],
    ] {
        assert!(!dock_authority_admits(refused));
    }
}

#[test]
fn preparation_rejects_a_mutated_portable_request_graph() {
    let evidence = observation();
    let mut plan = create_dock_plan(&evidence, 100, true).unwrap();
    let source = plan
        .fragments
        .iter_mut()
        .flat_map(|fragment| &mut fragment.placements)
        .find(|placement| placement.gear_id.as_str() == "seek_dock/request")
        .unwrap();
    source
        .configuration
        .iter_mut()
        .find(|entry| entry.key == "initial")
        .unwrap()
        .value = conduit_core::ConfigurationValue::Bool(false);
    assert!(prepare_create_dock_execution(&plan, &evidence).is_err());
}
