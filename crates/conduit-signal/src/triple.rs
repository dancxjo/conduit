//! One exact planned std/stdout, browser/DOM, and Pico/LED Signal execution.

use alloc::collections::BTreeMap;
use alloc::vec;

use conduit_core::{
    process_owned_line_offer_with_limits, ArtifactId, CapabilityId, CapabilityLimits,
    CapabilityOffer, ConnectionBase, GearId, HostAdvertisement, HostId, HostProfileId,
    ImplementationId, LineOffer, LinkEndpointId, LinkLimits, OfferGeneration, Plan,
    PROTOCOL_VERSION,
};
use conduit_planner::{plan_with_line_offers, PlacementChoice, PlacementChoices};

use crate::{
    pulse_contract_revision, pulse_execution_profile, pulse_host_operation_requirements,
    pulse_outputs, pulse_resource_requirements, show_contract_revision, show_execution_profile,
    show_host_operation_requirements, show_inputs, show_resource_requirements,
    signal_profile_catalog, signal_resource_offers, DISTRIBUTED_MAXIMUM_FRAME_BYTES,
    DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS, SIGNAL_ENCODED_LEN,
};

pub const SOURCE_HOST_ID: &str = "s4/triple-std";
pub const SOURCE_BOOT_ID: &str = "s4/triple-std-boot";
pub const BROWSER_HOST_ID: &str = "s4/triple-browser";
pub const BROWSER_BOOT_ID: &str = "s4/triple-browser-boot";
pub const PICO_HOST_ID: &str = "s4/triple-pico";
pub const PICO_IMAGE_BOOT_ID: &str = "s4/triple-pico-image-boot";

pub const BROWSER_LINK_ID: &str = "s4/triple-std-browser-link";
pub const BROWSER_LINE_ID: &str = "s4/line/triple-std-browser";
pub const BROWSER_BASE_INSTANCE_ID: &str = "s4/triple-websocket-loopback";
pub const BROWSER_SOURCE_ENDPOINT_ID: &str = "s4/triple-browser-egress";
pub const BROWSER_SINK_ENDPOINT_ID: &str = "s4/triple-browser-ingress";
pub const PICO_LINK_ID: &str = "s4/triple-std-pico-link";
pub const PICO_LINE_ID: &str = "s4/line/triple-std-pico";
pub const PICO_BASE_INSTANCE_ID: &str = "s4/triple-pico-usb-cdc-0";
pub const PICO_SOURCE_ENDPOINT_ID: &str = "s4/triple-pico-egress";
pub const PICO_SINK_ENDPOINT_ID: &str = "s4/triple-pico-ingress";

pub const PULSE_CAPABILITY_ID: &str = "triple-pulse-1";
pub const STDOUT_CAPABILITY_ID: &str = "triple-stdout-show-1";
pub const BROWSER_CAPABILITY_ID: &str = "triple-dom-show-1";
pub const PICO_CAPABILITY_ID: &str = "triple-cyw43-show-1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactTripleSignalPlan {
    pub source_advertisement: HostAdvertisement,
    pub browser_advertisement: HostAdvertisement,
    pub pico_advertisement: HostAdvertisement,
    pub browser_line: LineOffer,
    pub pico_line: LineOffer,
    pub plan: Plan,
}

fn capability(capability_id: &str, implementation_id: &str, is_pulse: bool) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: if is_pulse {
            crate::pulse_face_startup_parameters()
        } else {
            vec![]
        },
        shorthand: None,
        capability_id: CapabilityId::from(capability_id),
        kind_id: if is_pulse {
            crate::pulse_kind()
        } else {
            crate::show_kind()
        },
        kind_contract_revision: if is_pulse {
            pulse_contract_revision()
        } else {
            show_contract_revision()
        },
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: if is_pulse {
                pulse_execution_profile()
            } else {
                show_execution_profile()
            },
            implementation_id: ImplementationId::from(implementation_id),
            artifact_id: ArtifactId::from(if is_pulse {
                "conduit-signal/pulse-artifact-v1"
            } else {
                "conduit-signal/show-artifact-v1"
            }),
        },
        inputs: if is_pulse { vec![] } else { show_inputs() },
        outputs: if is_pulse { pulse_outputs() } else { vec![] },
        host_operations: if is_pulse {
            pulse_host_operation_requirements()
        } else {
            show_host_operation_requirements()
        },
        resource_requirements: if is_pulse {
            pulse_resource_requirements()
        } else {
            show_resource_requirements()
        },
        authority_requirements: vec![],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS,
            max_queue_bytes: SIGNAL_ENCODED_LEN,
        },
    }
}

pub fn source_advertisement() -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(SOURCE_HOST_ID),
        boot_id: conduit_core::BootId::from(SOURCE_BOOT_ID),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("rust-std-triple-kernel"),
        resources: signal_resource_offers("s4/triple-std-timer", "s4/triple-stdout", 1),
        planner_capabilities: vec![],
        capabilities: vec![
            capability(PULSE_CAPABILITY_ID, "std/kernel-pulse-v1", true),
            capability(
                STDOUT_CAPABILITY_ID,
                "std/kernel-stdout-show-signal-v1",
                false,
            ),
        ],
    }
}

pub fn browser_advertisement() -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(BROWSER_HOST_ID),
        boot_id: conduit_core::BootId::from(BROWSER_BOOT_ID),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("browser-wasm-triple-kernel"),
        resources: signal_resource_offers("s4/triple-browser-unused-timer", "s4/triple-dom", 1)
            .into_iter()
            .filter(|resource| {
                resource.class_id.as_str() == conduit_core::PRESENTATION_RESOURCE_CLASS
            })
            .collect(),
        planner_capabilities: vec![],
        capabilities: vec![capability(
            BROWSER_CAPABILITY_ID,
            "browser/kernel-dom-show-signal-v1",
            false,
        )],
    }
}

pub fn pico_advertisement() -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(PICO_HOST_ID),
        boot_id: conduit_core::BootId::from(PICO_IMAGE_BOOT_ID),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("rp2040-triple-kernel"),
        resources: signal_resource_offers("s4/triple-pico-unused-timer", "s4/triple-cyw43-led", 1)
            .into_iter()
            .filter(|resource| {
                resource.class_id.as_str() == conduit_core::PRESENTATION_RESOURCE_CLASS
            })
            .collect(),
        planner_capabilities: vec![],
        capabilities: vec![capability(
            PICO_CAPABILITY_ID,
            "pico/kernel-cyw43-show-signal-v1",
            false,
        )],
    }
}

#[allow(clippy::too_many_arguments)]
fn line_offer(
    line_id: &str,
    id: &str,
    base: ConnectionBase,
    base_instance: &str,
    source_endpoint: &str,
    sink_endpoint: &str,
    source: &HostAdvertisement,
    sink: &HostAdvertisement,
) -> LineOffer {
    let mut line = process_owned_line_offer_with_limits(
        line_id,
        id,
        base,
        base_instance,
        source,
        sink,
        LinkLimits {
            maximum_in_flight_items: DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS,
            maximum_payload_bytes: SIGNAL_ENCODED_LEN,
            maximum_buffered_bytes: SIGNAL_ENCODED_LEN,
            maximum_frame_bytes: DISTRIBUTED_MAXIMUM_FRAME_BYTES,
        },
    );
    line.binding.source.endpoint_id = LinkEndpointId::from(source_endpoint);
    line.binding.sink.endpoint_id = LinkEndpointId::from(sink_endpoint);
    line
}

pub fn exact_plan() -> Result<ExactTripleSignalPlan, alloc::string::String> {
    let source_advertisement = source_advertisement();
    let browser_advertisement = browser_advertisement();
    let pico_advertisement = pico_advertisement();
    let browser_line = line_offer(
        BROWSER_LINE_ID,
        BROWSER_LINK_ID,
        ConnectionBase::WebSocket,
        BROWSER_BASE_INSTANCE_ID,
        BROWSER_SOURCE_ENDPOINT_ID,
        BROWSER_SINK_ENDPOINT_ID,
        &source_advertisement,
        &browser_advertisement,
    );
    let pico_line = line_offer(
        PICO_LINE_ID,
        PICO_LINK_ID,
        ConnectionBase::UsbCdc,
        PICO_BASE_INSTANCE_ID,
        PICO_SOURCE_ENDPOINT_ID,
        PICO_SINK_ENDPOINT_ID,
        &source_advertisement,
        &pico_advertisement,
    );
    let form = conduit_form::parse_with_startup(
        include_str!("../../../fixtures/forms/triple-signal.conduit"),
        &crate::signal_startup_catalog(),
        &signal_profile_catalog(),
    )
    .map_err(|error| error.to_string())?;
    let placements = PlacementChoices {
        by_gear: BTreeMap::from([
            (
                GearId::from("triple-signal/pulse"),
                PlacementChoice {
                    host_id: source_advertisement.host_id.clone(),
                    capability_id: CapabilityId::from(PULSE_CAPABILITY_ID),
                },
            ),
            (
                GearId::from("triple-signal/local"),
                PlacementChoice {
                    host_id: source_advertisement.host_id.clone(),
                    capability_id: CapabilityId::from(STDOUT_CAPABILITY_ID),
                },
            ),
            (
                GearId::from("triple-signal/web"),
                PlacementChoice {
                    host_id: browser_advertisement.host_id.clone(),
                    capability_id: CapabilityId::from(BROWSER_CAPABILITY_ID),
                },
            ),
            (
                GearId::from("triple-signal/light"),
                PlacementChoice {
                    host_id: pico_advertisement.host_id.clone(),
                    capability_id: CapabilityId::from(PICO_CAPABILITY_ID),
                },
            ),
        ]),
    };
    let plan = plan_with_line_offers(
        &form,
        &[
            source_advertisement.clone(),
            browser_advertisement.clone(),
            pico_advertisement.clone(),
        ],
        &placements,
        &[
            ConnectionBase::Local,
            ConnectionBase::WebSocket,
            ConnectionBase::UsbCdc,
        ],
        DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS,
        SIGNAL_ENCODED_LEN,
        &[browser_line.clone(), pico_line.clone()],
    )
    .map_err(|error| error.to_string())?;
    Ok(ExactTripleSignalPlan {
        source_advertisement,
        browser_advertisement,
        pico_advertisement,
        browser_line,
        pico_line,
        plan,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unchanged_form_plans_one_local_and_two_exact_bounded_remote_branches() {
        let exact = exact_plan().expect("triple plan resolves");
        assert_eq!(exact.plan.fragments.len(), 3);
        let source = exact
            .plan
            .fragments
            .iter()
            .find(|fragment| fragment.host_id == exact.source_advertisement.host_id)
            .expect("source fragment");
        assert_eq!(source.connections.len(), 3);
        for link in [&exact.browser_line, &exact.pico_line] {
            assert_eq!(link.binding.limits.maximum_in_flight_items, 1);
            assert_eq!(
                link.binding.limits.maximum_payload_bytes,
                SIGNAL_ENCODED_LEN
            );
            assert_eq!(
                link.binding.limits.maximum_buffered_bytes,
                SIGNAL_ENCODED_LEN
            );
            assert_eq!(
                link.binding.limits.maximum_frame_bytes,
                DISTRIBUTED_MAXIMUM_FRAME_BYTES
            );
        }
    }

    #[test]
    fn missing_capability_and_stale_link_fail_closed() {
        let exact = exact_plan().expect("triple plan resolves");
        let form = conduit_form::parse_with_startup(
            include_str!("../../../fixtures/forms/triple-signal.conduit"),
            &crate::signal_startup_catalog(),
            &signal_profile_catalog(),
        )
        .unwrap();
        let mut missing_show = exact.source_advertisement.clone();
        missing_show
            .capabilities
            .retain(|offer| offer.capability_id.as_str() == PULSE_CAPABILITY_ID);
        assert!(
            conduit_planner::default_placements(&form, core::slice::from_ref(&missing_show))
                .is_err()
        );

        let mut stale = exact.browser_line.clone();
        stale.binding.sink.boot_id = conduit_core::BootId::from("stale-browser-boot");
        let placements = PlacementChoices {
            by_gear: BTreeMap::from([
                (
                    GearId::from("triple-signal/pulse"),
                    PlacementChoice {
                        host_id: exact.source_advertisement.host_id.clone(),
                        capability_id: CapabilityId::from(PULSE_CAPABILITY_ID),
                    },
                ),
                (
                    GearId::from("triple-signal/local"),
                    PlacementChoice {
                        host_id: exact.source_advertisement.host_id.clone(),
                        capability_id: CapabilityId::from(STDOUT_CAPABILITY_ID),
                    },
                ),
                (
                    GearId::from("triple-signal/web"),
                    PlacementChoice {
                        host_id: exact.browser_advertisement.host_id.clone(),
                        capability_id: CapabilityId::from(BROWSER_CAPABILITY_ID),
                    },
                ),
                (
                    GearId::from("triple-signal/light"),
                    PlacementChoice {
                        host_id: exact.pico_advertisement.host_id.clone(),
                        capability_id: CapabilityId::from(PICO_CAPABILITY_ID),
                    },
                ),
            ]),
        };
        assert!(plan_with_line_offers(
            &form,
            &[
                exact.source_advertisement,
                exact.browser_advertisement,
                exact.pico_advertisement,
            ],
            &placements,
            &[
                ConnectionBase::Local,
                ConnectionBase::WebSocket,
                ConnectionBase::UsbCdc,
            ],
            1,
            SIGNAL_ENCODED_LEN,
            &[stale, exact.pico_line],
        )
        .is_err());
    }
}
