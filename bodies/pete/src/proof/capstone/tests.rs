use super::*;
use crate::{
    IndependentWatchdogObservation, OiMode, SafetyInputObservation, SafetyObservation,
    CREATE_DRIVE_IMPLEMENTATION, CREATE_DRIVE_REDUCED_SAFETY_PROFILE,
};
use conduit_core::{BootId, HostId, OfferGeneration};

fn evidence(class: CapstoneHostClass) -> CapstoneHostEvidence {
    let (host, boot, serial, watchdog, translator, safety_inputs, independent_watchdog) =
        match class {
            CapstoneHostClass::Std => (
                "host/std-create",
                "boot/std-create",
                "base/std-prolific-by-id",
                None,
                None,
                SafetyInputObservation::Unavailable,
                IndependentWatchdogObservation::Absent,
            ),
            CapstoneHostClass::PicoW => (
                "host/pico-w-create",
                "boot/pico-w-create",
                "base/pico-uart0-gpio0-gpio1",
                Some("base/pico-watchdog".to_string()),
                Some("attachment/pico-create-level-translator".to_string()),
                SafetyInputObservation::Clear,
                IndependentWatchdogObservation::Healthy,
            ),
        };
    let host_id = HostId::from(host);
    let boot_id = BootId::from(boot);
    let generation = OfferGeneration(9);
    let robot = "device/pete-create1";
    CapstoneHostEvidence {
        class,
        observation: CreateObservationEvidence {
            host_id: host_id.clone(),
            boot_id: boot_id.clone(),
            offer_generation: generation,
            serial_base_id: serial.into(),
            robot_identity: robot.into(),
            session_resource_id: format!("{host}/observation-session"),
            mode: OiMode::Safe,
            observed_at_tick: 100,
            maximum_age_ticks: 10,
        },
        drive: CreateDriveObservation {
            host_id,
            boot_id,
            offer_generation: generation,
            serial_base_id: serial.into(),
            robot_identity: robot.into(),
            drive_resource_id: format!("{host}/drive"),
            mode: OiMode::Safe,
            safety: SafetyObservation {
                generation: 4,
                latch_generation: 1,
                latched_hazards: crate::SafetyHazardSet::EMPTY,
                observed_at_tick: 100,
                maximum_age_ticks: 10,
                emergency_stop: safety_inputs,
                wheel_drop: false,
                cliff: false,
                contact: false,
                tilt: safety_inputs,
                impact: safety_inputs,
                charging: false,
                control_alive: true,
                body_link_alive: true,
                independent_watchdog,
            },
        },
        serialized_client_pool_id: format!("{host}/serialized-create-provider"),
        watchdog_pool_id: watchdog,
        translator_pool_id: translator,
    }
}

#[test]
fn capstone_source_is_canonical_and_contains_no_realization_facts() {
    for forbidden in [
        "create",
        "uart",
        "serial",
        "pico",
        "linux",
        "std",
        "gpio",
        "watchdog",
        "provider",
        "host",
        "boot",
        "websocket",
    ] {
        assert!(!PETE_CAPSTONE_FORM.to_ascii_lowercase().contains(forbidden));
    }
    let (startup, profile) = crate::catalogs().unwrap();
    let syntax = conduit_form::parse_syntax_document(PETE_CAPSTONE_FORM);
    let checked = conduit_form::check_syntax_document(&syntax, &startup).unwrap();
    let expanded =
        conduit_form::expand_canonical_form(&checked, PETE_CAPSTONE_FORM_NAME, &profile).unwrap();
    assert_eq!(expanded.gears.len(), 5);
    assert_eq!(expanded.connections.len(), 5);
}

#[test]
fn same_form_seals_distinct_std_and_pico_plans_without_two_uarts() {
    let std_evidence = evidence(CapstoneHostClass::Std);
    let pico_evidence = evidence(CapstoneHostClass::PicoW);
    let std_host = capstone_advertisement(&std_evidence, 105).unwrap();
    let pico_host = capstone_advertisement(&pico_evidence, 105).unwrap();
    for host in [&std_host, &pico_host] {
        let uart = host
            .resources
            .iter()
            .find(|resource| resource.class_id.as_str() == CREATE_UART_BASE_RESOURCE)
            .unwrap();
        let device = host
            .resources
            .iter()
            .find(|resource| resource.class_id.as_str() == CREATE_DEVICE_RESOURCE)
            .unwrap();
        let clients = host
            .resources
            .iter()
            .find(|resource| resource.class_id.as_str() == CAPSTONE_SERIALIZED_CLIENT_RESOURCE)
            .unwrap();
        assert_eq!(uart.capacity_units, 1);
        assert_eq!(device.capacity_units, 1);
        assert_eq!(clients.capacity_units, 2);
    }
    assert!(std_host.resources.iter().all(|resource| !matches!(
        resource.class_id.as_str(),
        CAPSTONE_WATCHDOG_RESOURCE | CAPSTONE_TRANSLATOR_RESOURCE
    )));
    assert!(pico_host
        .resources
        .iter()
        .any(|resource| { resource.class_id.as_str() == CAPSTONE_WATCHDOG_RESOURCE }));
    assert!(pico_host
        .resources
        .iter()
        .any(|resource| { resource.class_id.as_str() == CAPSTONE_TRANSLATOR_RESOURCE }));

    let std_plan = capstone_plan(&std_evidence, 105).unwrap();
    let pico_plan = capstone_plan(&pico_evidence, 105).unwrap();
    assert_eq!(std_plan.source_document_id, pico_plan.source_document_id);
    assert_eq!(std_plan.checked_form_id, pico_plan.checked_form_id);
    assert_eq!(std_plan.expanded_form_id, pico_plan.expanded_form_id);
    assert_ne!(std_plan.plan_id, pico_plan.plan_id);
    assert_eq!(std_plan.fragments[0].placements.len(), 5);
    assert_eq!(pico_plan.fragments[0].placements.len(), 5);

    let std_drive = std_plan.fragments[0]
        .placements
        .iter()
        .find(|placement| {
            placement.kind_id.as_str() == conduit_semantic_catalog::ROBOTICS_DRIVE_DIFFERENTIAL_KIND
        })
        .unwrap();
    let pico_drive = pico_plan.fragments[0]
        .placements
        .iter()
        .find(|placement| {
            placement.kind_id.as_str() == conduit_semantic_catalog::ROBOTICS_DRIVE_DIFFERENTIAL_KIND
        })
        .unwrap();
    assert_eq!(
        std_drive.execution_profile_id.as_str(),
        CREATE_DRIVE_REDUCED_SAFETY_PROFILE
    );
    assert_eq!(
        std_drive.implementation_id.as_str(),
        CREATE_DRIVE_IMPLEMENTATION
    );
    assert_eq!(
        pico_drive.implementation_id.as_str(),
        CREATE_DRIVE_IMPLEMENTATION
    );
    assert_ne!(
        std_drive.execution_profile_id,
        pico_drive.execution_profile_id
    );
    assert!(std_drive.resources.iter().all(|resource| !matches!(
        resource.class_id.as_str(),
        CAPSTONE_WATCHDOG_RESOURCE | CAPSTONE_TRANSLATOR_RESOURCE
    )));
    assert!(pico_drive
        .resources
        .iter()
        .any(|resource| { resource.class_id.as_str() == CAPSTONE_WATCHDOG_RESOURCE }));
}

#[test]
fn wrong_host_class_and_cross_boot_truth_refuse_before_planning() {
    let mut invented = evidence(CapstoneHostClass::Std);
    invented.watchdog_pool_id = Some("invented/watchdog".into());
    assert_eq!(
        capstone_advertisement(&invented, 105),
        Err(CapstoneAdvertisementRefusal::StdInventedEmbeddedResource)
    );
    let mut missing = evidence(CapstoneHostClass::PicoW);
    missing.translator_pool_id = None;
    assert_eq!(
        capstone_advertisement(&missing, 105),
        Err(CapstoneAdvertisementRefusal::PicoMissingEmbeddedResource)
    );
    let mut crossed = evidence(CapstoneHostClass::PicoW);
    crossed.drive.boot_id = BootId::from("boot/not-the-observed-pico");
    assert_eq!(
        capstone_advertisement(&crossed, 105),
        Err(CapstoneAdvertisementRefusal::IdentityMismatch)
    );
}
