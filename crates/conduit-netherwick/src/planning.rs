use std::collections::BTreeMap;

use crate::{
    catalogs, live_create_drive_advertisement, live_speaker_advertisement,
    live_speaker_realization, CreateDriveObservation, CreateSpeakerObservation,
    CREATE_DRIVE_AUTHORITY, CREATE_DRIVE_CAPABILITY, CREATE_DRIVE_OPERATION,
    CREATE_DRIVE_REDUCED_SAFETY_AUTHORITY, SPEAKER_AUTHORITY, SPEAKER_CAPABILITY,
    SPEAKER_OPERATION,
};
use conduit_core::{
    kind_id, AuthorityContractId, AuthorityGrant, AuthorityGrantId, CapabilityId, ConnectionBase,
    HostOperationContractId, ResourceHealth, ResourceObservation, SignId, SCALAR_ENCODED_LEN,
    SCALAR_INFO_ID,
};
use conduit_planner::{
    plan_selected_realizations_with_characteristics_and_authority, PlannerError,
    SelectedRealizationPlanning,
};

/// Portable canonical Form. It names musical meaning only; Create, OI,
/// serial, song slots, and speaker resources enter solely through the Plan.
pub const SIMPLE_MELODY_FORM: &str = r#"form simple_melody {
    performance: music/play
}
"#;

/// Portable canonical Form for one bounded body-velocity realization. Exact
/// Host, safety class, authority, Create OI, and UART facts enter through Plan.
pub const BOUNDED_DRIVE_FORM: &str = r#"form bounded_drive {
    drive: robotics/drive-differential(ttl-ms = 250)
}
"#;
pub const BOUNDED_DRIVE_GRANT: &str = "grant/netherwick-bounded-drive";

pub fn simple_melody_plan(
    observation: &CreateSpeakerObservation,
    authority_granted: bool,
) -> Result<conduit_core::Plan, PlannerError> {
    let (_, profile) = catalogs().expect("fixed Pete catalogs are valid");
    let checked = conduit_form::parse(SIMPLE_MELODY_FORM, &profile)
        .expect("portable melody checks without mechanism facts");
    let host = live_speaker_advertisement(observation)
        .expect("caller supplies one fresh usable Create speaker observation");
    let realization = live_speaker_realization(observation)
        .expect("the same observation produces realization facts");
    let observations = host
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
            unreserved_units: pool.capacity_units,
            utilized_units: 0,
            sign_id: SignId::from(format!("create-speaker-resource-{index}")),
        })
        .collect::<Vec<_>>();
    let grants = authority_granted.then(|| AuthorityGrant {
        grant_id: AuthorityGrantId::from("grant/netherwick-create1-speaker-only"),
        contract_id: AuthorityContractId::from(SPEAKER_AUTHORITY),
        host_operation_contract_id: HostOperationContractId::from(SPEAKER_OPERATION),
        subject_kind: kind_id(conduit_core::MUSIC_NOTE_INFO_ID),
        host_id: host.host_id.clone(),
        boot_id: host.boot_id.clone(),
        capability_id: conduit_core::CapabilityId::from(SPEAKER_CAPABILITY),
    });
    let hosts = [host];
    plan_selected_realizations_with_characteristics_and_authority(
        &checked,
        SelectedRealizationPlanning {
            hosts: &hosts,
            bases: &[ConnectionBase::Local],
            requirements: &BTreeMap::new(),
            advertisements: &[realization],
            observations: &observations,
            policies: &BTreeMap::new(),
            connection_item_capacity: 16,
            connection_byte_capacity: 35,
            authority_grants: grants.as_slice(),
        },
    )
}

pub fn bounded_drive_plan(
    observation: &CreateDriveObservation,
    authority_granted: bool,
) -> Result<conduit_core::Plan, PlannerError> {
    let (_, profile) = catalogs().expect("fixed Pete catalogs are valid");
    let checked = conduit_form::parse(BOUNDED_DRIVE_FORM, &profile)
        .expect("portable drive checks without mechanism facts");
    let host = live_create_drive_advertisement(observation, observation.safety.observed_at_tick)
        .expect("caller supplies fresh non-hazardous Create drive truth");
    let observations = host
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
            unreserved_units: pool.capacity_units,
            utilized_units: 0,
            sign_id: SignId::from(format!("create-drive-resource-{index}")),
        })
        .collect::<Vec<_>>();
    let authority_contract = if observation.safety.has_complete_independent_envelope() {
        CREATE_DRIVE_AUTHORITY
    } else {
        CREATE_DRIVE_REDUCED_SAFETY_AUTHORITY
    };
    let grants = authority_granted.then(|| AuthorityGrant {
        grant_id: AuthorityGrantId::from(BOUNDED_DRIVE_GRANT),
        contract_id: AuthorityContractId::from(authority_contract),
        host_operation_contract_id: HostOperationContractId::from(CREATE_DRIVE_OPERATION),
        subject_kind: kind_id(SCALAR_INFO_ID),
        host_id: host.host_id.clone(),
        boot_id: host.boot_id.clone(),
        capability_id: CapabilityId::from(CREATE_DRIVE_CAPABILITY),
    });
    plan_selected_realizations_with_characteristics_and_authority(
        &checked,
        SelectedRealizationPlanning {
            hosts: &[host],
            bases: &[ConnectionBase::Local],
            requirements: &BTreeMap::new(),
            advertisements: &[],
            observations: &observations,
            policies: &BTreeMap::new(),
            connection_item_capacity: 2,
            connection_byte_capacity: (2 * SCALAR_ENCODED_LEN) as u32,
            authority_grants: grants.as_slice(),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CreateDriveObservation, CreateSpeakerObservation, IndependentWatchdogObservation, OiMode,
        SafetyInputObservation, SafetyObservation, CREATE_DRIVE_IMPLEMENTATION,
        CREATE_DRIVE_REDUCED_SAFETY_AUTHORITY, CREATE_DRIVE_REDUCED_SAFETY_PROFILE,
    };
    use conduit_core::{BootId, HostId, OfferGeneration};

    fn live_speaker() -> CreateSpeakerObservation {
        CreateSpeakerObservation {
            host_id: HostId::from("netherwick-std-live"),
            boot_id: BootId::from("netherwick-std-live-boot"),
            offer_generation: OfferGeneration(7),
            serial_base_id: "netherwick/create1/serial/0".into(),
            robot_identity: "netherwick/create1/observed-robot".into(),
            robot_identity_verified: true,
            speaker_resource_id: "netherwick/create1/speaker".into(),
            mode: OiMode::Safe,
            currently_usable: true,
        }
    }

    fn reduced_drive() -> CreateDriveObservation {
        CreateDriveObservation {
            host_id: HostId::from("std/netherwick"),
            boot_id: BootId::from("std/netherwick-boot"),
            offer_generation: OfferGeneration(1),
            serial_base_id: "std/create-uart/0".into(),
            robot_identity: "robot/create1/0".into(),
            drive_resource_id: "robot/create1/0/drive".into(),
            mode: OiMode::Safe,
            safety: SafetyObservation {
                generation: 1,
                latch_generation: 1,
                latched_hazards: crate::SafetyHazardSet::EMPTY,
                observed_at_tick: 100,
                maximum_age_ticks: 1_000,
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
            },
        }
    }

    #[test]
    fn unchanged_drive_form_plans_the_exact_reduced_safety_realization() {
        for forbidden in ["create", "uart", "serial", "watchdog", "std-host", "gpio"] {
            assert!(!BOUNDED_DRIVE_FORM.contains(forbidden));
        }
        let plan = bounded_drive_plan(&reduced_drive(), true).unwrap();
        let placement = &plan.fragments[0].placements[0];
        assert_eq!(
            placement.implementation_id.as_str(),
            CREATE_DRIVE_IMPLEMENTATION
        );
        assert_eq!(
            placement.execution_profile_id.as_str(),
            CREATE_DRIVE_REDUCED_SAFETY_PROFILE
        );
        assert_eq!(
            placement.authority[0].contract_id.as_str(),
            CREATE_DRIVE_REDUCED_SAFETY_AUTHORITY
        );
    }

    #[test]
    fn unchanged_mechanism_free_form_plans_only_with_explicit_speaker_authority() {
        for forbidden in ["create", "oi", "serial", "speaker", "song", "pete"] {
            assert!(!SIMPLE_MELODY_FORM.to_ascii_lowercase().contains(forbidden));
        }
        let without = simple_melody_plan(&live_speaker(), false).unwrap_err();
        assert!(
            matches!(without, PlannerError::AuthorityGrantMissing(_)),
            "unexpected refusal: {without:?}"
        );
        let plan = simple_melody_plan(&live_speaker(), true).unwrap();
        let placement = &plan.fragments[0].placements[0];
        assert_eq!(
            placement.kind_id.as_str(),
            conduit_std_catalog::MUSIC_PLAY_KIND
        );
        assert_eq!(placement.authority.len(), 1);
        assert_eq!(placement.host_operations.len(), 1);
        assert!(serde_json::to_string(&plan)
            .unwrap()
            .contains(SPEAKER_CAPABILITY));
        assert!(!serde_json::to_string(&plan)
            .unwrap()
            .contains(conduit_std_catalog::ROBOTICS_DRIVE_DIFFERENTIAL_KIND));
    }
}
