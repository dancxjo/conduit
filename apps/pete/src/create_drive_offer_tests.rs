use super::*;
use crate::SafetyInputObservation;
use conduit_core::{
    AuthorityGrant, AuthorityGrantId, BaseImplementationId, ResourceHealth, ResourceObservation,
    SignId,
};
use conduit_planner::{
    plan_selected_realizations_with_characteristics_and_authority, PlannerError,
    SelectedRealizationPlanning,
};
use std::collections::BTreeMap;

const DRIVE_FORM: &str = r#"form move_body {
    drive: robotics/drive-differential(ttl-ms = 250)
}
"#;

fn safety() -> SafetyObservation {
    SafetyObservation {
        generation: 8,
        latch_generation: 1,
        latched_hazards: crate::SafetyHazardSet::EMPTY,
        observed_at_tick: 100,
        maximum_age_ticks: 10,
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

fn observation() -> CreateDriveObservation {
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

fn resource_observations(host: &HostAdvertisement) -> Vec<ResourceObservation> {
    host.resources
        .iter()
        .enumerate()
        .map(|(index, pool)| ResourceObservation {
            host_id: host.host_id.clone(),
            boot_id: host.boot_id.clone(),
            offer_generation: host.offer_generation,
            pool_id: pool.pool_id.clone(),
            class_id: pool.class_id.clone(),
            health: ResourceHealth::Ready,
            unreserved_units: pool.capacity_units,
            utilized_units: 0,
            sign_id: SignId::from(format!("drive-resource-{index}")),
        })
        .collect()
}

fn grant(host: &HostAdvertisement, contract: &str) -> AuthorityGrant {
    AuthorityGrant {
        grant_id: AuthorityGrantId::from("grant/create-motion"),
        contract_id: AuthorityContractId::from(contract),
        host_operation_contract_id: HostOperationContractId::from(CREATE_DRIVE_OPERATION),
        subject_kind: kind_id(SCALAR_INFO_ID),
        host_id: host.host_id.clone(),
        boot_id: host.boot_id.clone(),
        capability_id: CapabilityId::from(CREATE_DRIVE_CAPABILITY),
    }
}

fn plan(
    host: &HostAdvertisement,
    observations: &[ResourceObservation],
    grants: &[AuthorityGrant],
) -> Result<conduit_core::Plan, PlannerError> {
    let (_, profile) = crate::catalogs().unwrap();
    let checked = conduit_form::parse(DRIVE_FORM, &profile).unwrap();
    plan_selected_realizations_with_characteristics_and_authority(
        &checked,
        SelectedRealizationPlanning {
            hosts: std::slice::from_ref(host),
            bases: &[BaseImplementationId::from("conduit.base/local@1")],
            requirements: &BTreeMap::new(),
            advertisements: &[],
            observations,
            policies: &BTreeMap::new(),
            connection_item_capacity: 2,
            connection_byte_capacity: (2 * SCALAR_ENCODED_LEN) as u32,
            authority_grants: grants,
        },
    )
}

#[test]
fn live_offer_has_exact_capacity_one_resources_and_motion_authority() {
    let host = live_create_drive_advertisement(&observation(), 105).unwrap();
    assert_eq!(host.resources.len(), 3);
    assert!(host.resources.iter().all(|pool| pool.capacity_units == 1));
    let offer = &host.capabilities[0];
    assert_eq!(offer.inputs.len(), 2);
    assert_eq!(offer.resource_requirements.len(), 3);
    assert_eq!(offer.authority_requirements.len(), 1);
    assert_eq!(
        offer.kind_contract_revision.as_str(),
        conduit_semantic_catalog::ROBOTICS_DRIVE_DIFFERENTIAL_REVISION
    );
}

#[test]
fn absent_watchdog_selects_the_reduced_profile_and_authority_contract() {
    let mut value = observation();
    value.safety.independent_watchdog = IndependentWatchdogObservation::Absent;
    let host = live_create_drive_advertisement(&value, 105).unwrap();
    assert_eq!(host.profile.as_str(), CREATE_DRIVE_REDUCED_SAFETY_PROFILE);
    assert_eq!(
        host.capabilities[0].authority_requirements[0]
            .contract_id
            .as_str(),
        CREATE_DRIVE_REDUCED_SAFETY_AUTHORITY
    );
    assert!(host
        .resources
        .iter()
        .all(|resource| !resource.class_id.as_str().contains("watchdog")));

    let resources = resource_observations(&host);
    let wrong = [grant(&host, CREATE_DRIVE_AUTHORITY)];
    assert!(matches!(
        plan(&host, &resources, &wrong),
        Err(PlannerError::AuthorityGrantMissing(_))
    ));
    let exact = [grant(&host, CREATE_DRIVE_REDUCED_SAFETY_AUTHORITY)];
    assert!(plan(&host, &resources, &exact).is_ok());
}

#[test]
fn absent_stale_and_inhibited_truth_refuse_before_advertisement() {
    let mut value = observation();
    value.robot_identity.clear();
    assert_eq!(
        live_create_drive_advertisement(&value, 105),
        Err(CreateDriveOfferRefusal::MissingIdentity)
    );
    value = observation();
    value.mode = OiMode::Passive;
    assert_eq!(
        live_create_drive_advertisement(&value, 105),
        Err(CreateDriveOfferRefusal::UnsupportedMode)
    );
    value = observation();
    value.safety.maximum_age_ticks = 0;
    assert_eq!(
        live_create_drive_advertisement(&value, 105),
        Err(CreateDriveOfferRefusal::InvalidFreshness)
    );
    value = observation();
    assert_eq!(
        live_create_drive_advertisement(&value, 111),
        Err(CreateDriveOfferRefusal::SafetyStaleOrInhibited(
            LocalHazard::BodyLinkLost
        ))
    );
    value.safety.observed_at_tick = 111;
    value.safety.cliff = true;
    assert_eq!(
        live_create_drive_advertisement(&value, 111),
        Err(CreateDriveOfferRefusal::SafetyStaleOrInhibited(
            LocalHazard::Cliff
        ))
    );
}

#[test]
fn unlatched_raw_safety_truth_cannot_offer_physical_drive() {
    let mut value = observation();
    value.safety.latch_generation = 0;
    assert_eq!(
        live_create_drive_advertisement(&value, 100),
        Err(CreateDriveOfferRefusal::MissingSafetyEnvelope)
    );
}

#[test]
fn unchanged_mechanism_free_form_requires_exact_authority_and_resources() {
    for forbidden in ["create", "uart", "serial", "gpio", "safety", "host"] {
        assert!(!DRIVE_FORM.contains(forbidden));
    }
    let host = live_create_drive_advertisement(&observation(), 105).unwrap();
    let resources = resource_observations(&host);
    let missing = plan(&host, &resources, &[]).unwrap_err();
    assert!(
        matches!(missing, PlannerError::AuthorityGrantMissing(_)),
        "unexpected refusal: {missing:?}"
    );
    let wrong = [grant(&host, "authority/not-motion")];
    assert!(matches!(
        plan(&host, &resources, &wrong),
        Err(PlannerError::AuthorityGrantMissing(_))
    ));
    let exact = [grant(&host, CREATE_DRIVE_AUTHORITY)];
    let sealed = plan(&host, &resources, &exact).unwrap();
    let placement = &sealed.fragments[0].placements[0];
    assert_eq!(
        placement.implementation_id.as_str(),
        CREATE_DRIVE_IMPLEMENTATION
    );
    assert_eq!(placement.resources.len(), 3);
    assert_eq!(placement.authority.len(), 1);

    let mut contended = resources;
    contended[0].unreserved_units = 0;
    let pressure = plan(&host, &contended, &exact).unwrap_err();
    assert!(
        matches!(
            pressure,
            PlannerError::CurrentResourceObservationUnavailable(_)
        ),
        "unexpected refusal: {pressure:?}"
    );
}
