use std::collections::BTreeMap;

use crate::{
    brainstem_advertisement, catalogs, live_speaker_advertisement, live_speaker_realization,
    CreateSpeakerObservation, SPEAKER_AUTHORITY, SPEAKER_CAPABILITY, SPEAKER_OPERATION,
};
use conduit_core::{
    kind_id, AuthorityContractId, AuthorityGrant, AuthorityGrantId, ConnectionBase,
    HostOperationContractId, ResourceHealth, ResourceObservation, SignId,
};
use conduit_form::{check_syntax_document, expand_canonical_form, parse_syntax_document};
use conduit_planner::{
    default_expanded_placements, plan_expanded_canonical,
    plan_selected_realizations_with_characteristics_and_authority, PlannerError,
    SelectedRealizationPlanning,
};

pub const OBSERVATION_FORM: &str = r#"form pete_describe_only {
    bump: robotics/observe-bump
    imu: robotics/observe-imu
}
"#;

pub const ACTUATOR_ATTEMPT_FORM: &str = r#"form forbidden_drive {
    drive: robotics/drive-differential
}
"#;

/// Portable canonical Form. It names musical meaning only; Create, OI,
/// serial, song slots, and speaker resources enter solely through the Plan.
pub const SIMPLE_MELODY_FORM: &str = r#"form simple_melody {
    performance: music/play
}
"#;

pub fn observation_plan() -> Result<conduit_core::Plan, String> {
    let (startup, profile) = catalogs()?;
    let syntax = parse_syntax_document(OBSERVATION_FORM);
    let checked = check_syntax_document(&syntax, &startup).map_err(|error| format!("{error:?}"))?;
    let expanded = expand_canonical_form(&checked, "pete_describe_only", &profile)
        .map_err(|error| error.to_string())?;
    let host = brainstem_advertisement();
    let placements = default_expanded_placements(&expanded, std::slice::from_ref(&host))
        .map_err(|error| error.to_string())?;
    plan_expanded_canonical(&expanded, &[host], &placements, &[ConnectionBase::Local])
        .map_err(|error| error.to_string())
}

pub fn attempt_actuator_plan() -> Result<(), PlannerError> {
    let (startup, profile) = catalogs().expect("fixed describe catalogs are valid");
    let syntax = parse_syntax_document(ACTUATOR_ATTEMPT_FORM);
    let checked = check_syntax_document(&syntax, &startup).expect("actuator meaning is valid");
    let expanded = expand_canonical_form(&checked, "forbidden_drive", &profile)
        .expect("actuator meaning expands independently of realization");
    default_expanded_placements(&expanded, &[brainstem_advertisement()]).map(|_| ())
}

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
        grant_id: AuthorityGrantId::from("grant/pete-create1-speaker-only"),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CreateSpeakerObservation, OiMode, BUMP_KIND, DRIVE_KIND, IMU_KIND};
    use conduit_core::{BootId, HostId, OfferGeneration};

    fn live_speaker() -> CreateSpeakerObservation {
        CreateSpeakerObservation {
            host_id: HostId::from("pete-brainstem-live"),
            boot_id: BootId::from("pete-brainstem-live-boot"),
            offer_generation: OfferGeneration(7),
            serial_base_id: "pete/create1/serial/0".into(),
            robot_identity: "pete/create1/observed-robot".into(),
            robot_identity_verified: true,
            speaker_resource_id: "pete/create1/speaker".into(),
            mode: OiMode::Safe,
            currently_usable: true,
        }
    }

    #[test]
    fn observation_plan_seals_only_effect_free_sensor_offers() {
        let plan = observation_plan().unwrap();
        let placements = &plan.fragments[0].placements;
        assert_eq!(placements.len(), 2);
        assert!(placements.iter().all(|placement| {
            [BUMP_KIND, IMU_KIND].contains(&placement.kind_id.as_str())
                && placement.host_operations.is_empty()
                && placement.authority.is_empty()
        }));
        assert!(!serde_json::to_string(&plan).unwrap().contains(DRIVE_KIND));
    }

    #[test]
    fn valid_actuator_meaning_has_no_describe_only_realization() {
        assert!(matches!(
            attempt_actuator_plan(),
            Err(PlannerError::UnknownCapability(_))
        ));
        let profile = crate::pinned_profile();
        assert!(profile.effect_audit.is_effect_free());
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
        assert!(!serde_json::to_string(&plan).unwrap().contains(DRIVE_KIND));
    }
}
