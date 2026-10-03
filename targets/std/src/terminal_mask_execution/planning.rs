//! The ordinary semantic Mask graph, specialized to attached hosted terminal I/O.
use super::*;
use conduit_core::*;
use conduit_planner::{
    default_expanded_placements, plan_expanded_authoring_with_options, ConnectionQueueLimits,
    ForeBoundaryKey, PlanningOptions,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    KindSignature, ProfileCatalog, StartupCatalog,
};
use conduit_presentation::*;
use std::collections::BTreeMap;

pub(super) fn plan(host: &HostAdvertisement) -> Result<PlannedMaskPlot, TerminalError> {
    let mut attached = host.clone();
    attached.bases.clear();
    attached.resources = terminal_resources();
    attached.capabilities = terminal_capabilities();
    attached.planner_capabilities.clear();
    plan_on_host(host, &attached)
}

/// The attached path must plan against the actual current Host advertisement.
/// Merely copying its Host/Boot into a made-up terminal offer is insufficient.
pub(super) fn plan_attached(host: &HostAdvertisement) -> Result<PlannedMaskPlot, TerminalError> {
    for required in attached_terminal_capabilities() {
        if !host.capabilities.contains(&required) {
            return Err(error(
                "current Host does not offer the attached terminal Back",
            ));
        }
    }
    for required in attached_terminal_resources() {
        if !host.resources.contains(&required) {
            return Err(error(
                "current Host does not offer the attached terminal resource",
            ));
        }
    }
    plan_on_host(host, host)
}

fn plan_on_host(
    host: &HostAdvertisement,
    planning_host: &HostAdvertisement,
) -> Result<PlannedMaskPlot, TerminalError> {
    if host.protocol_version != PROTOCOL_VERSION
        || host.host_id.as_str().is_empty()
        || host.boot_id.as_str().is_empty()
        || host.offer_generation.0 == 0
    {
        return Err(error("invalid current Host/Boot/generation"));
    }
    let mut startup = StartupCatalog::new();
    let mut catalog = ProfileCatalog::new();
    install_mask_plot_value_aliases(&mut startup).map_err(error)?;
    for kind in [
        presentation_tee_kind_projection(),
        renderer_kind_projection(),
        face_interaction_kind_projection(),
    ] {
        startup
            .insert(KindSignature {
                kind: kind.kind_id.as_str().into(),
                startup_parameters: vec![],
            })
            .map_err(debug_error)?;
        catalog.insert(kind).map_err(debug_error)?;
    }
    let checked = check_syntax_document(
        &parse_syntax_document(include_str!("terminal.conduit")),
        &startup,
    )
    .map_err(debug_error)?;
    let authoring =
        expand_canonical_plot_for_authoring(&checked, "terminal", &catalog).map_err(debug_error)?;
    let mask = MaskPlot::admit(&authoring).map_err(debug_error)?;
    let hosts = [planning_host.clone()];
    let placements =
        default_expanded_placements(&authoring.expanded, &hosts).map_err(debug_error)?;
    let boundaries = [
        (PortDirection::Input, "face"),
        (PortDirection::Output, "show"),
        (PortDirection::Output, "interaction"),
    ]
    .into_iter()
    .map(|(direction, name)| {
        (
            ForeBoundaryKey {
                direction,
                front_port_id: port_id(name),
                track: ConnectionTrack::Payload,
            },
            ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: if name == "interaction" {
                    MAX_FACE_INTERACTION_BYTES as u32
                } else {
                    MAX_TERMINAL_VALUE_BYTES
                },
            },
        )
    })
    .collect();
    let plan = plan_expanded_authoring_with_options(
        &authoring,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: MAX_TERMINAL_VALUE_BYTES,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .map_err(debug_error)?;
    PlannedMaskPlot::admit(&mask, &plan).map_err(debug_error)
}

pub(crate) fn terminal_capabilities() -> Vec<CapabilityOffer> {
    attached_terminal_capabilities()
}

pub(crate) const READ_ONLY_INTERACTION_IMPLEMENTATION: &str =
    "presentation/terminal-input-closed@1";

pub(crate) fn attached_terminal_capabilities() -> Vec<CapabilityOffer> {
    let limits = CapabilityLimits {
        max_active_instances: 1,
        max_queue_items: 1,
        max_queue_bytes: MAX_TERMINAL_VALUE_BYTES,
    };
    vec![
        presentation_tee_offer(
            "terminal/tee".into(),
            ImplementationOffer {
                execution_profile_id: "std/terminal-mask@1".into(),
                implementation_id: "conduit.presentation/tee-kernel@1".into(),
                artifact_id: "std/terminal-mask@1".into(),
            },
            limits.clone(),
        ),
        renderer_offer(RendererRealizationOffer {
            capability_id: "terminal/render".into(),
            execution_profile_id: "std/terminal-mask@1".into(),
            implementation_id: "presentation/renderer-terminal@1".into(),
            artifact_id: "std/terminal-mask@1".into(),
            host_call: HostCallRequirement {
                contract_id: PRESENT_CALL.into(),
                target_kind: Some(kind_id(TERMINAL_TARGET)),
                maximum_in_flight: 1,
                maximum_input_bytes: MAX_TERMINAL_VALUE_BYTES,
                maximum_output_bytes: MAX_TERMINAL_VALUE_BYTES,
            },
            resource_requirement: resource_requirement("conduit.resource/terminal-output@1", 1),
            limits: limits.clone(),
        }),
        face_interaction_offer(FaceInteractionRealizationOffer {
            capability_id: "terminal/input".into(),
            execution_profile_id: "std/terminal-mask@1".into(),
            implementation_id: "presentation/terminal-input@1".into(),
            artifact_id: "std/terminal-mask@1".into(),
            host_call: HostCallRequirement {
                contract_id: INTERACTION_CALL.into(),
                target_kind: Some(kind_id(FACE_INTERACTION_VALUE_KIND)),
                maximum_in_flight: 1,
                maximum_input_bytes: MAX_TERMINAL_VALUE_BYTES,
                maximum_output_bytes: MAX_FACE_INTERACTION_BYTES as u32,
            },
            resource_requirement: resource_requirement("conduit.resource/terminal-input@1", 1),
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: MAX_FACE_INTERACTION_BYTES as u32 * 8,
            },
        }),
    ]
}

pub(crate) fn terminal_resources() -> Vec<ResourceOffer> {
    vec![
        resource_offer("terminal/input", "conduit.resource/terminal-input@1", 1),
        resource_offer("terminal/output", "conduit.resource/terminal-output@1", 1),
    ]
}

pub(crate) fn attached_terminal_resources() -> Vec<ResourceOffer> {
    terminal_resources()
}
