//! Portable non-metric robot input and inertial observation contracts.

use crate::{StandardKindContract, TerminalBehavior};
use alloc::format;
use alloc::string::ToString;
use alloc::{vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, PortDescriptor, PortDirection, PortTemporal,
    ROBOTICS_ACCELERATION_INFO_ID, ROBOTICS_BEACON_INFO_ID, ROBOTICS_BUTTONS_INFO_ID,
    ROBOTICS_PROXIMITY_INFO_ID,
};

pub const ROBOTICS_OBSERVE_PROXIMITY_KIND: &str = "robotics/observe-proximity";
pub const ROBOTICS_OBSERVE_BEACON_KIND: &str = "robotics/observe-beacon";
pub const ROBOTICS_OBSERVE_BUTTONS_KIND: &str = "robotics/observe-buttons";
pub const ROBOTICS_OBSERVE_ACCELERATION_KIND: &str = "robotics/observe-acceleration";

pub const ROBOTICS_OBSERVE_PROXIMITY_REVISION: &str = "conduit.std/robotics-observe-proximity@1";
pub const ROBOTICS_OBSERVE_BEACON_REVISION: &str = "conduit.std/robotics-observe-beacon@1";
pub const ROBOTICS_OBSERVE_BUTTONS_REVISION: &str = "conduit.std/robotics-observe-buttons@1";
pub const ROBOTICS_OBSERVE_ACCELERATION_REVISION: &str =
    "conduit.std/robotics-observe-acceleration@1";

pub fn robotics_observe_proximity_contract() -> StandardKindContract {
    observation_contract(
        ROBOTICS_OBSERVE_PROXIMITY_KIND,
        "Body proximity observation",
        "Observe body-relative proximity sectors without inventing metric range.",
        "proximity",
        ROBOTICS_PROXIMITY_INFO_ID,
        1,
    )
}

pub fn robotics_observe_beacon_contract() -> StandardKindContract {
    observation_contract(
        ROBOTICS_OBSERVE_BEACON_KIND,
        "Beacon observation",
        "Observe virtual-wall presence or an exact infrared code without conflating them.",
        "beacon",
        ROBOTICS_BEACON_INFO_ID,
        2,
    )
}

pub fn robotics_observe_buttons_contract() -> StandardKindContract {
    observation_contract(
        ROBOTICS_OBSERVE_BUTTONS_KIND,
        "Button-set observation",
        "Observe one finite set of pressed semantic button positions.",
        "buttons",
        ROBOTICS_BUTTONS_INFO_ID,
        4,
    )
}

pub fn robotics_observe_acceleration_contract() -> StandardKindContract {
    observation_contract(
        ROBOTICS_OBSERVE_ACCELERATION_KIND,
        "Body acceleration observation",
        "Observe exact forward/left/up body-frame acceleration in millimetres per second squared.",
        "acceleration",
        ROBOTICS_ACCELERATION_INFO_ID,
        12,
    )
}

pub fn robotics_input_contracts() -> Vec<StandardKindContract> {
    vec![
        robotics_observe_proximity_contract(),
        robotics_observe_beacon_contract(),
        robotics_observe_buttons_contract(),
        robotics_observe_acceleration_contract(),
    ]
}

#[cfg(any(feature = "form-catalog", test))]
pub(crate) fn robotics_input_contracts_with_revisions() -> Vec<(StandardKindContract, &'static str)>
{
    vec![
        (
            robotics_observe_proximity_contract(),
            ROBOTICS_OBSERVE_PROXIMITY_REVISION,
        ),
        (
            robotics_observe_beacon_contract(),
            ROBOTICS_OBSERVE_BEACON_REVISION,
        ),
        (
            robotics_observe_buttons_contract(),
            ROBOTICS_OBSERVE_BUTTONS_REVISION,
        ),
        (
            robotics_observe_acceleration_contract(),
            ROBOTICS_OBSERVE_ACCELERATION_REVISION,
        ),
    ]
}

fn observation_contract(
    kind: &str,
    name: &str,
    summary: &str,
    port_name: &str,
    info: &str,
    maximum_value_bytes: u32,
) -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(kind),
        plain_name: name.to_string(),
        summary: summary.to_string(),
        inputs: Vec::new(),
        outputs: vec![PortDescriptor {
            port_id: port_id(port_name),
            value_kind: kind_id(info),
            direction: PortDirection::Output,
            temporal: PortTemporal::Current,
        }],
        configuration: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 16,
            max_queue_items: 1,
            max_queue_bytes: maximum_value_bytes,
        },
        terminal_behavior: TerminalBehavior::HostObservationEndsOrFailsSource,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: format!("observation: {kind}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_metric_and_inertial_contracts_are_bounded_and_mechanism_free() {
        for contract in robotics_input_contracts() {
            assert_eq!(contract.outputs.len(), 1);
            assert_eq!(contract.limits.max_queue_items, 1);
            assert!(contract.limits.max_queue_bytes <= 12);
            let debug = format!("{contract:?}").to_ascii_lowercase();
            for forbidden in ["create", "uart", "gpio", "pete"] {
                assert!(!debug.contains(forbidden));
            }
        }
        assert_ne!(
            robotics_observe_proximity_contract().outputs[0]
                .value_kind
                .as_str(),
            conduit_core::ROBOTICS_RANGE_INFO_ID
        );
    }

    #[cfg(feature = "form-catalog")]
    #[test]
    fn canonical_input_form_checks_and_expands_without_device_facts() {
        let mut startup = conduit_form::StartupCatalog::new();
        let mut profile = conduit_form::ProfileCatalog::new();
        crate::install_robotics_catalogs(&mut startup, &mut profile).unwrap();
        let source = "form robot_inputs {\n near: robotics/observe-proximity\n beacon: robotics/observe-beacon\n buttons: robotics/observe-buttons\n acceleration: robotics/observe-acceleration\n}\n";
        let syntax = conduit_form::parse_syntax_document(source);
        let checked = conduit_form::check_syntax_document(&syntax, &startup).unwrap();
        let expanded =
            conduit_form::expand_canonical_form(&checked, "robot_inputs", &profile).unwrap();
        assert_eq!(expanded.gears.len(), 4);
    }
}
