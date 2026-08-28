//! Portable robotics contracts required by physical hazard and docking Hosts.
//!
//! These are semantic catalog definitions, not std simulation offers. A Host
//! may advertise an implementation only after observing the exact mechanism,
//! resources, freshness, safety boundary, and authority required by that
//! implementation.

use crate::{StandardKindContract, TerminalBehavior};
use alloc::format;
use alloc::string::ToString;
use alloc::{vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, PortDescriptor, PortDirection, PortTemporal, BOOL_INFO_ID,
    ROBOTICS_CHARGING_INFO_ID, ROBOTICS_CLIFF_INFO_ID, ROBOTICS_CONTACT_INFO_ID,
    ROBOTICS_WHEEL_DROP_INFO_ID,
};

pub const ROBOTICS_OBSERVE_CONTACT_KIND: &str = "robotics/observe-contact";
pub const ROBOTICS_OBSERVE_CLIFF_KIND: &str = "robotics/observe-cliff";
pub const ROBOTICS_OBSERVE_WHEEL_DROP_KIND: &str = "robotics/observe-wheel-drop";
pub const ROBOTICS_OBSERVE_CHARGING_KIND: &str = "robotics/observe-charging";
pub const ROBOTICS_DOCK_KIND: &str = "robotics/dock";
pub const ROBOTICS_DOCK_DEFAULT_TIMEOUT_MS: u64 = 30_000;
pub const ROBOTICS_DOCK_MINIMUM_TIMEOUT_MS: u64 = 1_000;
pub const ROBOTICS_DOCK_MAXIMUM_TIMEOUT_MS: u64 = 120_000;

pub const ROBOTICS_OBSERVE_CONTACT_REVISION: &str = "conduit.std/robotics-observe-contact@1";
pub const ROBOTICS_OBSERVE_CLIFF_REVISION: &str = "conduit.std/robotics-observe-cliff@1";
pub const ROBOTICS_OBSERVE_WHEEL_DROP_REVISION: &str = "conduit.std/robotics-observe-wheel-drop@1";
pub const ROBOTICS_OBSERVE_CHARGING_REVISION: &str = "conduit.std/robotics-observe-charging@1";
pub const ROBOTICS_DOCK_REVISION: &str = "conduit.std/robotics-dock@2";

pub fn robotics_observe_contact_contract() -> StandardKindContract {
    observation_contract(
        ROBOTICS_OBSERVE_CONTACT_KIND,
        "Body contact observation",
        "Observe exact active body sectors; mechanism freshness and provenance remain explicit realization evidence.",
        "contact",
        ROBOTICS_CONTACT_INFO_ID,
    )
}

pub fn robotics_observe_cliff_contract() -> StandardKindContract {
    observation_contract(
        ROBOTICS_OBSERVE_CLIFF_KIND,
        "Cliff hazard observation",
        "Observe exact body-relative cliff sectors and only the detector signals actually available.",
        "cliff",
        ROBOTICS_CLIFF_INFO_ID,
    )
}

pub fn robotics_observe_wheel_drop_contract() -> StandardKindContract {
    observation_contract(
        ROBOTICS_OBSERVE_WHEEL_DROP_KIND,
        "Wheel-drop observation",
        "Observe the exact body-relative wheels currently reported dropped.",
        "wheel-drop",
        ROBOTICS_WHEEL_DROP_INFO_ID,
    )
}

pub fn robotics_observe_charging_contract() -> StandardKindContract {
    observation_contract(
        ROBOTICS_OBSERVE_CHARGING_KIND,
        "Charging observation",
        "Observe charging state, sources, voltage, current, temperature, charge, and capacity without fabricating unavailable data.",
        "charging",
        ROBOTICS_CHARGING_INFO_ID,
    )
}

pub fn robotics_dock_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(ROBOTICS_DOCK_KIND),
        plain_name: "Request docking".to_string(),
        summary: "Consume an explicit docking request; authority, mechanism, and completion evidence belong to the selected realization."
            .to_string(),
        inputs: vec![current_port(
            "request",
            BOOL_INFO_ID,
            PortDirection::Input,
        )],
        outputs: Vec::new(),
        configuration: vec![crate::StandardConfigurationField {
            key: "timeout-ms".to_string(),
            default_value: conduit_core::ConfigurationValue::U64(
                ROBOTICS_DOCK_DEFAULT_TIMEOUT_MS,
            ),
            rule: crate::StandardConfigurationRule::U64Range {
                minimum: ROBOTICS_DOCK_MINIMUM_TIMEOUT_MS,
                maximum: ROBOTICS_DOCK_MAXIMUM_TIMEOUT_MS,
            },
        }],
        limits: limits(1),
        terminal_behavior: TerminalBehavior::CompletesAfterDockedRefusedOrDeadline,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "dock: robotics/dock(timeout-ms = 30000)".to_string(),
    }
}

pub fn robotics_hazard_contracts() -> Vec<StandardKindContract> {
    vec![
        robotics_observe_contact_contract(),
        robotics_observe_cliff_contract(),
        robotics_observe_wheel_drop_contract(),
        robotics_observe_charging_contract(),
        robotics_dock_contract(),
    ]
}

#[cfg(any(feature = "form-catalog", test))]
pub(crate) fn robotics_hazard_contracts_with_revisions() -> Vec<(StandardKindContract, &'static str)>
{
    vec![
        (
            robotics_observe_contact_contract(),
            ROBOTICS_OBSERVE_CONTACT_REVISION,
        ),
        (
            robotics_observe_cliff_contract(),
            ROBOTICS_OBSERVE_CLIFF_REVISION,
        ),
        (
            robotics_observe_wheel_drop_contract(),
            ROBOTICS_OBSERVE_WHEEL_DROP_REVISION,
        ),
        (
            robotics_observe_charging_contract(),
            ROBOTICS_OBSERVE_CHARGING_REVISION,
        ),
        (robotics_dock_contract(), ROBOTICS_DOCK_REVISION),
    ]
}

fn observation_contract(
    kind: &str,
    name: &str,
    summary: &str,
    port_name: &str,
    info: &str,
) -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(kind),
        plain_name: name.to_string(),
        summary: summary.to_string(),
        inputs: Vec::new(),
        outputs: vec![current_port(port_name, info, PortDirection::Output)],
        configuration: Vec::new(),
        limits: limits(12),
        terminal_behavior: TerminalBehavior::HostObservationEndsOrFailsSource,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: format!("observation: {kind}"),
    }
}

fn current_port(name: &str, info: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(info),
        direction,
        temporal: PortTemporal::Current,
    }
}

fn limits(maximum_value_bytes: u32) -> CapabilityLimits {
    CapabilityLimits {
        max_active_instances: 16,
        max_queue_items: 1,
        max_queue_bytes: maximum_value_bytes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::collections::BTreeSet;
    use conduit_core::KindContractRevision;

    #[test]
    fn portable_contracts_are_exact_distinct_bounded_and_mechanism_free() {
        let contracts = robotics_hazard_contracts();
        assert_eq!(contracts.len(), 5);
        assert_eq!(
            contracts
                .iter()
                .map(|contract| contract.kind_id.as_str())
                .collect::<BTreeSet<_>>()
                .len(),
            contracts.len()
        );
        for contract in contracts {
            assert!(contract.hosted_implementation_required);
            assert!(contract.limits.max_queue_items <= 1);
            assert!(contract.limits.max_queue_bytes <= 12);
            let serialized = format!("{contract:?}").to_ascii_lowercase();
            for forbidden in ["create", "uart", "serial", "gpio", "pete"] {
                assert!(
                    !serialized.contains(forbidden),
                    "{forbidden} leaked into {serialized}"
                );
            }
        }
    }

    #[cfg(feature = "form-catalog")]
    #[test]
    fn canonical_forms_check_without_any_device_vocabulary() {
        let mut startup = conduit_form::StartupCatalog::new();
        let mut profile = conduit_form::ProfileCatalog::new();
        crate::install_robotics_catalogs(&mut startup, &mut profile).unwrap();
        let source = "form robot_hazards {\n contact: robotics/observe-contact\n cliff: robotics/observe-cliff\n wheel: robotics/observe-wheel-drop\n charging: robotics/observe-charging\n dock: robotics/dock\n}\n";
        let syntax = conduit_form::parse_syntax_document(source);
        let checked = conduit_form::check_syntax_document(&syntax, &startup).unwrap();
        let expanded =
            conduit_form::expand_canonical_form(&checked, "robot_hazards", &profile).unwrap();
        assert_eq!(expanded.gears.len(), 5);
        for forbidden in ["create", "uart", "serial", "gpio", "pete"] {
            assert!(!source.contains(forbidden));
        }
    }

    #[test]
    fn bool_wall_or_cliff_is_not_mislabeled_as_metric_range() {
        for contract in [
            robotics_observe_contact_contract(),
            robotics_observe_cliff_contract(),
            robotics_observe_wheel_drop_contract(),
        ] {
            assert!(contract
                .outputs
                .iter()
                .all(|port| port.value_kind.as_str() != conduit_core::ROBOTICS_RANGE_INFO_ID));
        }
    }

    #[test]
    fn revisions_are_exact_and_unique() {
        let revisions = robotics_hazard_contracts_with_revisions()
            .into_iter()
            .map(|(_, revision)| KindContractRevision::from(revision))
            .collect::<BTreeSet<_>>();
        assert_eq!(revisions.len(), 5);
    }
}
