//! Exact Create 1 indicator realization of canonical Signal presentation.

use std::collections::BTreeMap;

use conduit_core::{
    authority_grant, kind_id, present_authority_requirement, resource_offer, resource_requirement,
    ArtifactId, BootId, CapabilityId, CapabilityLimits, CapabilityOffer, ConnectionBase, GearId,
    HostAdvertisement, HostId, HostProfileId, ImplementationId, ImplementationOffer,
    OfferGeneration, PRESENTATION_RESOURCE_CLASS, PROTOCOL_VERSION,
};
use conduit_planner::{
    plan_with_options, PlacementChoice, PlacementChoices, PlannerError, PlanningOptions,
};

use crate::{OiMode, SERIAL_OPERATION_RESOURCE};

pub const CREATE_1_INDICATOR_SPECIFICATION: &str = crate::CREATE_1_OI_SPECIFICATION;
pub const CREATE_1_INDICATOR_SPECIFICATION_URL: &str = crate::CREATE_1_OI_SPECIFICATION_URL;
pub const CREATE_1_LEDS_OPCODE: u8 = 139;
pub const CREATE_1_POWER_LED_GREEN: u8 = 0;
pub const CREATE_1_POWER_LED_FULL_INTENSITY: u8 = 255;
pub const CREATE_1_INDICATOR_COMMAND_BYTES: usize = 4;

pub const INDICATOR_CAPABILITY: &str = "pete/create1-show-signal@1";
pub const INDICATOR_PROFILE: &str = "pete/create1-oi-power-indicator@1";
pub const INDICATOR_IMPLEMENTATION: &str = "pete/create1-oi-power-led-show@1";
pub const INDICATOR_ARTIFACT: &str = "conduit-pete/create1-oi-indicator@1";
pub const INDICATOR_RESOURCE: &str = "pete.resource/create1-indicator@1";
pub const INDICATOR_GRANT: &str = "grant/pete-create1-indicator-present";

pub const CREATE_INDICATOR_FORM: &str = include_str!("../../../examples/signal-demo.conduit");

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateIndicatorObservation {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub serial_base_id: String,
    pub robot_identity: String,
    pub robot_identity_verified: bool,
    pub indicator_resource_id: String,
    pub timer_resource_id: String,
    pub mode: OiMode,
    pub currently_usable: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IndicatorRefusal {
    MissingIdentity,
    UnverifiedIdentity,
    NotCurrentlyUsable,
    UnsupportedMode,
    OpcodeOutsideIndicatorAuthority,
}

pub fn encode_indicator(level: bool) -> [u8; CREATE_1_INDICATOR_COMMAND_BYTES] {
    if level {
        [
            CREATE_1_LEDS_OPCODE,
            0,
            CREATE_1_POWER_LED_GREEN,
            CREATE_1_POWER_LED_FULL_INTENSITY,
        ]
    } else {
        [CREATE_1_LEDS_OPCODE, 0, 0, 0]
    }
}

pub fn indicator_authority_admits(command: &[u8]) -> bool {
    command == encode_indicator(false) || command == encode_indicator(true)
}

pub fn live_indicator_advertisement(
    observation: &CreateIndicatorObservation,
) -> Result<HostAdvertisement, IndicatorRefusal> {
    if observation.serial_base_id.is_empty()
        || observation.robot_identity.is_empty()
        || observation.indicator_resource_id.is_empty()
        || observation.timer_resource_id.is_empty()
    {
        return Err(IndicatorRefusal::MissingIdentity);
    }
    if !observation.robot_identity_verified {
        return Err(IndicatorRefusal::UnverifiedIdentity);
    }
    if !observation.currently_usable {
        return Err(IndicatorRefusal::NotCurrentlyUsable);
    }
    if !matches!(observation.mode, OiMode::Safe | OiMode::Full) {
        return Err(IndicatorRefusal::UnsupportedMode);
    }

    let source = conduit_signal_conformance::distributed_source_advertisement_for(
        observation.host_id.clone(),
        observation.boot_id.clone(),
    );
    let mut pulse = source.capabilities[0].clone();
    pulse.capability_id = CapabilityId::from("pete/create1-indicator-pulse@1");
    pulse.limits.max_queue_items = 4;
    pulse.limits.max_queue_bytes = 4 * conduit_signal::SIGNAL_ENCODED_LEN;
    let present_authority =
        present_authority_requirement(kind_id(conduit_signal::SIGNAL_PRESENTATION_KIND));
    let serial_pool = format!("{}/indicator-operation", observation.serial_base_id);
    let mut resources = vec![
        resource_offer(
            &observation.timer_resource_id,
            conduit_core::TIMER_RESOURCE_CLASS,
            1,
        ),
        resource_offer(
            &observation.indicator_resource_id,
            PRESENTATION_RESOURCE_CLASS,
            1,
        ),
        resource_offer(&serial_pool, SERIAL_OPERATION_RESOURCE, 1),
    ];
    resources.sort();
    let mut requirements = vec![
        resource_requirement(PRESENTATION_RESOURCE_CLASS, 1),
        resource_requirement(SERIAL_OPERATION_RESOURCE, 1),
    ];
    requirements.sort();
    let show = CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from(INDICATOR_CAPABILITY),
        kind_id: conduit_signal::show_kind(),
        kind_contract_revision: conduit_signal::show_contract_revision(),
        implementation: ImplementationOffer {
            execution_profile_id: conduit_signal::show_execution_profile(),
            implementation_id: ImplementationId::from(INDICATOR_IMPLEMENTATION),
            artifact_id: ArtifactId::from(INDICATOR_ARTIFACT),
        },
        inputs: conduit_signal::show_inputs(),
        outputs: Vec::new(),
        host_operations: conduit_signal::show_host_operation_requirements(),
        resource_requirements: requirements,
        authority_requirements: vec![present_authority],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 4,
            max_queue_bytes: 4 * conduit_signal::SIGNAL_ENCODED_LEN,
        },
    };
    let mut capabilities = vec![pulse, show];
    capabilities.sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
    Ok(HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: observation.host_id.clone(),
        boot_id: observation.boot_id.clone(),
        offer_generation: observation.offer_generation,
        profile: HostProfileId::from(INDICATOR_PROFILE),
        resources,
        planner_capabilities: Vec::new(),
        capabilities,
    })
}

pub fn create_indicator_plan(
    observation: &CreateIndicatorObservation,
    authority_granted: bool,
) -> Result<conduit_core::Plan, PlannerError> {
    let form = conduit_form::parse(
        CREATE_INDICATOR_FORM,
        &conduit_signal::signal_profile_catalog(),
    )
    .expect("canonical Signal Form checks independently of Create facts");
    let host = live_indicator_advertisement(observation)
        .expect("caller supplies one fresh usable indicator observation");
    let pulse_capability = host
        .capabilities
        .iter()
        .find(|capability| capability.kind_id.as_str() == conduit_signal::PULSE_KIND)
        .expect("indicator host has one pulse source");
    let placements = PlacementChoices {
        by_gear: BTreeMap::from([
            (
                GearId::from("signal-demo/pulse"),
                PlacementChoice {
                    host_id: host.host_id.clone(),
                    capability_id: pulse_capability.capability_id.clone(),
                },
            ),
            (
                GearId::from("signal-demo/show"),
                PlacementChoice {
                    host_id: host.host_id.clone(),
                    capability_id: CapabilityId::from(INDICATOR_CAPABILITY),
                },
            ),
        ]),
    };
    let requirement =
        present_authority_requirement(kind_id(conduit_signal::SIGNAL_PRESENTATION_KIND));
    let grant = authority_granted.then(|| {
        authority_grant(
            INDICATOR_GRANT,
            &requirement,
            host.host_id.clone(),
            host.boot_id.clone(),
            CapabilityId::from(INDICATOR_CAPABILITY),
        )
    });
    plan_with_options(
        &form,
        &[host],
        &placements,
        &[ConnectionBase::Local],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 4,
            connection_byte_capacity: 4 * conduit_signal::SIGNAL_ENCODED_LEN,
            authority_grants: grant.as_slice(),
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation() -> CreateIndicatorObservation {
        CreateIndicatorObservation {
            host_id: HostId::from("std/create-indicator"),
            boot_id: BootId::from("std/create-indicator-boot"),
            offer_generation: OfferGeneration(1),
            serial_base_id: "std/create-uart/0".into(),
            robot_identity: "robot/create1/0".into(),
            robot_identity_verified: true,
            indicator_resource_id: "robot/create1/0/power-led".into(),
            timer_resource_id: "std/timer/create-indicator".into(),
            mode: OiMode::Safe,
            currently_usable: true,
        }
    }

    #[test]
    fn indicator_authority_is_exactly_off_or_full_green() {
        assert!(indicator_authority_admits(&encode_indicator(false)));
        assert!(indicator_authority_admits(&encode_indicator(true)));
        for refused in [
            &[128][..],
            &[131],
            &[137, 0, 0, 0, 0],
            &[139, 1, 0, 255],
            &[140, 0, 1, 60, 32],
            &[143],
        ] {
            assert!(!indicator_authority_admits(refused), "admitted {refused:?}");
        }
    }

    #[test]
    fn current_verified_identity_is_required_for_offer() {
        let mut value = observation();
        value.robot_identity_verified = false;
        assert_eq!(
            live_indicator_advertisement(&value),
            Err(IndicatorRefusal::UnverifiedIdentity)
        );
        value.robot_identity_verified = true;
        value.currently_usable = false;
        assert_eq!(
            live_indicator_advertisement(&value),
            Err(IndicatorRefusal::NotCurrentlyUsable)
        );
    }

    #[test]
    fn unchanged_signal_form_plans_only_with_present_authority() {
        for forbidden in ["create", "uart", "serial", "robot", "led", "opcode"] {
            assert!(!CREATE_INDICATOR_FORM
                .to_ascii_lowercase()
                .contains(forbidden));
        }
        assert!(matches!(
            create_indicator_plan(&observation(), false),
            Err(PlannerError::AuthorityGrantMissing(_))
        ));
        let plan = create_indicator_plan(&observation(), true).unwrap();
        assert_eq!(plan.fragments.len(), 1);
        assert_eq!(plan.fragments[0].placements.len(), 2);
        let encoded = serde_json::to_string(&plan).unwrap();
        assert!(encoded.contains(INDICATOR_IMPLEMENTATION));
        assert!(!encoded.contains(crate::CREATE_DRIVE_IMPLEMENTATION));
    }
}
