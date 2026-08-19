use super::*;
use crate::{
    live_create_drive_advertisement, CreateOiFailure, IndependentWatchdogObservation, LocalHazard,
    MotionSafetyAuthority, OiMode, SafetyInputObservation, UartParity, UartProfile,
    CREATE_DRIVE_AUTHORITY, CREATE_DRIVE_CAPABILITY, CREATE_DRIVE_OPERATION,
};
use conduit_core::{
    kind_id, AuthorityContractId, AuthorityGrant, AuthorityGrantId, CapabilityId,
    ConfigurationValue, ConnectionBase, HostOperationContractId, ResourceHealth,
    ResourceObservation, ResourcePoolId, SignId, SCALAR_INFO_ID,
};
use conduit_planner::{
    plan_selected_realizations_with_characteristics_and_authority, SelectedRealizationPlanning,
};
use std::collections::BTreeMap;

const FORM: &str = r#"form move_body {
    drive: robotics/drive-differential(ttl-ms = 250)
}
"#;

struct Provider {
    available: bool,
    profile: UartProfile,
    writes: Vec<Vec<u8>>,
    fail_at_write: Option<usize>,
}

impl Provider {
    fn ready() -> Self {
        Self {
            available: true,
            profile: UartProfile::CREATE_OI,
            writes: Vec::new(),
            fail_at_write: None,
        }
    }
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
        if self.fail_at_write == Some(self.writes.len()) {
            return Err(());
        }
        self.writes.push(bytes.to_vec());
        Ok(())
    }

    fn read_byte(&mut self, _deadline_tick: u64) -> Result<Option<u8>, Self::Error> {
        Ok(None)
    }
}

fn safety() -> SafetyObservation {
    SafetyObservation {
        generation: 8,
        observed_at_tick: 100,
        maximum_age_ticks: 20,
        emergency_stop: SafetyInputObservation::Clear,
        wheel_drop: false,
        cliff: false,
        contact: false,
        tilt: SafetyInputObservation::Clear,
        impact: SafetyInputObservation::Clear,
        charging: false,
        control_alive: true,
        body_link_alive: true,
        independent_watchdog: IndependentWatchdogObservation::Healthy,
    }
}

fn evidence() -> CreateDriveObservation {
    CreateDriveObservation {
        host_id: HostId::from("host/create-live"),
        boot_id: BootId::from("boot/create-live"),
        offer_generation: OfferGeneration(12),
        serial_base_id: "base/uart0".into(),
        robot_identity: "device/create1".into(),
        drive_resource_id: "device/create1/drive".into(),
        mode: OiMode::Safe,
        safety: safety(),
    }
}

fn planned() -> conduit_core::Plan {
    let evidence = evidence();
    let host = live_create_drive_advertisement(&evidence, 100).unwrap();
    let resources = host
        .resources
        .iter()
        .enumerate()
        .map(|(index, pool)| ResourceObservation {
            host_id: host.host_id.clone(),
            boot_id: host.boot_id.clone(),
            offer_generation: host.offer_generation,
            pool_id: pool.pool_id.clone(),
            class_id: pool.class_id.clone(),
            health: ResourceHealth::Ready,
            unreserved_units: 1,
            utilized_units: 0,
            sign_id: SignId::from(format!("drive-resource-{index}")),
        })
        .collect::<Vec<_>>();
    let grants = [AuthorityGrant {
        grant_id: AuthorityGrantId::from("grant/create-motion"),
        contract_id: AuthorityContractId::from(CREATE_DRIVE_AUTHORITY),
        host_operation_contract_id: HostOperationContractId::from(CREATE_DRIVE_OPERATION),
        subject_kind: kind_id(SCALAR_INFO_ID),
        host_id: host.host_id.clone(),
        boot_id: host.boot_id.clone(),
        capability_id: CapabilityId::from(CREATE_DRIVE_CAPABILITY),
    }];
    let (_, profile) = crate::catalogs().unwrap();
    let checked = conduit_form::parse(FORM, &profile).unwrap();
    plan_selected_realizations_with_characteristics_and_authority(
        &checked,
        SelectedRealizationPlanning {
            hosts: &[host],
            bases: &[ConnectionBase::Local],
            requirements: &BTreeMap::new(),
            advertisements: &[],
            observations: &resources,
            policies: &BTreeMap::new(),
            connection_item_capacity: 2,
            connection_byte_capacity: (2 * SCALAR_ENCODED_LEN) as u32,
            authority_grants: &grants,
        },
    )
    .unwrap()
}

fn authority(grant_id: &'static str) -> MotionAuthority<'static> {
    MotionAuthority {
        grant_id,
        valid_until_tick: 1_000,
        safety_class: MotionSafetyAuthority::IndependentWatchdog,
    }
}

fn prepared(linear: i64, angular: i64) -> PreparedCreateDriveExecution {
    prepare_create_drive_execution(
        &planned(),
        &evidence(),
        Scalar::from_raw_microunits(linear),
        Scalar::from_raw_microunits(angular),
    )
    .unwrap()
}

#[test]
fn planned_scalars_dispatch_exact_motion_then_ttl_stop_through_kernel() {
    let mut execution = prepared(500_000, 250_000);
    let mut provider = Provider::ready();
    let admitted = dispatch_create_drive_execution(
        &mut execution,
        &mut provider,
        100,
        Some(authority("grant/create-motion")),
        safety(),
    );
    assert_eq!(provider.writes, [vec![145, 1, 119, 0, 125]]);
    assert!(matches!(
        admitted.terminal,
        CreateDriveExecutionTerminal::MotionAdmitted {
            deadline_tick: 350,
            left_mm_s: 125,
            right_mm_s: 375,
            safety_generation: 8,
            ..
        }
    ));
    assert!(admitted.kernel_decisions > 0 && admitted.kernel_signs > 0);

    let mut current = safety();
    current.observed_at_tick = 349;
    assert_eq!(
        supervise_create_drive_execution(&mut execution, &mut provider, 349, current).terminal,
        CreateDriveExecutionTerminal::Active
    );
    current.observed_at_tick = 350;
    assert!(matches!(
        supervise_create_drive_execution(&mut execution, &mut provider, 350, current).terminal,
        CreateDriveExecutionTerminal::SafeDisposition {
            cause: SafeDispositionCause::DeadlineExpired,
            safety_generation: 8,
        }
    ));
    assert_eq!(provider.writes[1], [145, 0, 0, 0, 0]);
}

#[test]
fn authority_safety_and_combined_demand_refuse_before_motion_bytes() {
    for (authority, mut current, expected) in [
        (
            None,
            safety(),
            CreateDriveExecutionRefusal::Drive(DriveRefusal::MissingAuthority),
        ),
        (
            Some(authority("grant/wrong")),
            safety(),
            CreateDriveExecutionRefusal::AuthorityGrantMismatch,
        ),
        (
            Some(authority("grant/create-motion")),
            SafetyObservation {
                generation: 7,
                ..safety()
            },
            CreateDriveExecutionRefusal::SafetyGenerationRegressed,
        ),
    ] {
        current.observed_at_tick = 100;
        let mut execution = prepared(100_000, 0);
        let mut provider = Provider::ready();
        assert_eq!(
            dispatch_create_drive_execution(
                &mut execution,
                &mut provider,
                100,
                authority,
                current,
            )
            .terminal,
            CreateDriveExecutionTerminal::Refused(expected)
        );
        assert!(provider.writes.is_empty());
    }

    let mut execution = prepared(750_000, 500_000);
    let mut provider = Provider::ready();
    assert_eq!(
        dispatch_create_drive_execution(
            &mut execution,
            &mut provider,
            100,
            Some(authority("grant/create-motion")),
            safety(),
        )
        .terminal,
        CreateDriveExecutionTerminal::Refused(CreateDriveExecutionRefusal::Lowering(
            CreateDriveLoweringRefusal::WheelDemandOutsideRealization
        ))
    );
    assert!(provider.writes.is_empty());
}

#[test]
fn cancellation_and_hazard_stop_and_stop_failure_stays_visible() {
    let mut before = prepared(100_000, 0);
    let mut provider = Provider::ready();
    assert_eq!(
        cancel_create_drive_execution(&mut before, &mut provider, 8).terminal,
        CreateDriveExecutionTerminal::CancelledBeforeDispatch
    );
    assert!(provider.writes.is_empty());

    let mut execution = prepared(100_000, 0);
    dispatch_create_drive_execution(
        &mut execution,
        &mut provider,
        100,
        Some(authority("grant/create-motion")),
        safety(),
    );
    let mut hazard = safety();
    hazard.generation = 9;
    hazard.observed_at_tick = 101;
    hazard.cliff = true;
    assert!(matches!(
        supervise_create_drive_execution(&mut execution, &mut provider, 101, hazard).terminal,
        CreateDriveExecutionTerminal::SafeDisposition {
            cause: SafeDispositionCause::Hazard(LocalHazard::Cliff),
            safety_generation: 9,
        }
    ));
    assert_eq!(provider.writes.last().unwrap(), &[145, 0, 0, 0, 0]);

    let mut failed = prepared(100_000, 0);
    let mut provider = Provider::ready();
    dispatch_create_drive_execution(
        &mut failed,
        &mut provider,
        100,
        Some(authority("grant/create-motion")),
        safety(),
    );
    provider.fail_at_write = Some(1);
    assert_eq!(
        cancel_create_drive_execution(&mut failed, &mut provider, 10).terminal,
        CreateDriveExecutionTerminal::SafeDisposition {
            cause: SafeDispositionCause::ProviderFailure(CreateOiFailure::WriteFailed),
            safety_generation: 10,
        }
    );
}

#[test]
fn expired_authority_uart_profile_and_provider_failure_remain_distinct() {
    let cases = [
        (
            false,
            UartProfile::CREATE_OI,
            None,
            MotionAuthority {
                grant_id: "grant/create-motion",
                valid_until_tick: 1_000,
                safety_class: MotionSafetyAuthority::IndependentWatchdog,
            },
            DriveRefusal::Device(CreateOiFailure::ProviderUnavailable),
        ),
        (
            true,
            UartProfile {
                baud: 115_200,
                data_bits: 8,
                stop_bits: 1,
                parity: UartParity::None,
            },
            None,
            authority("grant/create-motion"),
            DriveRefusal::Device(CreateOiFailure::WrongUartProfile {
                observed: UartProfile {
                    baud: 115_200,
                    data_bits: 8,
                    stop_bits: 1,
                    parity: UartParity::None,
                },
            }),
        ),
        (
            true,
            UartProfile::CREATE_OI,
            Some(0),
            authority("grant/create-motion"),
            DriveRefusal::Device(CreateOiFailure::WriteFailed),
        ),
        (
            true,
            UartProfile::CREATE_OI,
            None,
            MotionAuthority {
                grant_id: "grant/create-motion",
                valid_until_tick: 100,
                safety_class: MotionSafetyAuthority::IndependentWatchdog,
            },
            DriveRefusal::AuthorityExpired,
        ),
    ];
    for (available, profile, fail_at_write, authority, expected) in cases {
        let mut execution = prepared(100_000, 0);
        let mut provider = Provider {
            available,
            profile,
            writes: Vec::new(),
            fail_at_write,
        };
        assert_eq!(
            dispatch_create_drive_execution(
                &mut execution,
                &mut provider,
                100,
                Some(authority),
                safety(),
            )
            .terminal,
            CreateDriveExecutionTerminal::Refused(CreateDriveExecutionRefusal::Drive(expected))
        );
        assert!(provider.writes.is_empty());
    }
}

#[test]
fn preparation_rejects_mutated_plan_identity_and_resources() {
    let exact = planned();
    let rejects = |plan: &conduit_core::Plan| {
        assert!(
            prepare_create_drive_execution(plan, &evidence(), Scalar::ZERO, Scalar::ZERO).is_err()
        );
    };

    let mut plan = exact.clone();
    plan.fragments[0].placements[0].host_id = HostId::from("host/wrong");
    rejects(&plan);
    let mut plan = exact.clone();
    plan.fragments[0].placements[0].offer_generation = OfferGeneration(13);
    rejects(&plan);
    let mut plan = exact.clone();
    plan.fragments[0].placements[0].host_operations[0].contract_id =
        HostOperationContractId::from("operation/wrong");
    rejects(&plan);
    let mut plan = exact.clone();
    plan.fragments[0].placements[0].authority[0].contract_id =
        AuthorityContractId::from("authority/wrong");
    rejects(&plan);
    let mut plan = exact.clone();
    plan.fragments[0].placements[0].resources[0].pool_id = ResourcePoolId::from("pool/wrong");
    rejects(&plan);
    let mut plan = exact;
    plan.fragments[0].placements[0]
        .configuration
        .iter_mut()
        .find(|entry| entry.key == "ttl-ms")
        .unwrap()
        .value = ConfigurationValue::U64(9);
    rejects(&plan);
}
