//! Reviewed Back and ordinary Plot Plan for the browser's bounded DOM Mask.

use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    KindSignature, ProfileCatalog, StartupCatalog,
};
use std::{collections::BTreeMap, string::String};

use conduit_core::{
    kind_id, port_id, resource_requirement, ArtifactId, Back, BackOfferBuilder, CapabilityId,
    CapabilityLimits, ExecutionProfileId, HostAdvertisement, HostCallContractId,
    HostCallRequirement, ImplementationId, Kind, KindIdentity, PortDescriptor, PortDirection,
    PortTemporal, PRESENTATION_RESOURCE_CLASS,
};
use conduit_presentation::{
    install_mask_plot_value_aliases, MaskPlot, PlannedMaskPlot, FACE_INTERACTION_VALUE_KIND,
    PRESENTATION_VALUE_KIND, SHOW_VALUE_KIND,
};

mod remote;
pub use remote::{planned_owner_face_show_interaction_mask, planned_owner_face_show_mask};

pub const MASK_BYTES: u32 = 512 * 1024;
const MASK_OPERATION: &str = "browser.host/dom-mask@1";

fn port(
    name: &str,
    value_kind: &str,
    direction: PortDirection,
    temporal: PortTemporal,
) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(value_kind),
        direction,
        temporal,
        abnormal_kind: None,
    }
}

pub fn kind() -> Kind {
    Kind {
        startup_parameters: vec![],
        shorthand: None,
        kind_id: kind_id("presentation/browser-dom-mask"),
        kind_contract_revision: KindIdentity::from("conduit.browser/presentation-dom-mask@1"),
        inputs: vec![port(
            "presentation",
            PRESENTATION_VALUE_KIND,
            PortDirection::Input,
            PortTemporal::Value,
        )],
        outputs: vec![
            port(
                "interaction",
                FACE_INTERACTION_VALUE_KIND,
                PortDirection::Output,
                PortTemporal::Flow { closes: true },
            ),
            port(
                "show",
                SHOW_VALUE_KIND,
                PortDirection::Output,
                PortTemporal::Value,
            ),
        ],
        configuration: vec![],
        semantic_laws: Default::default(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 4,
            max_queue_bytes: MASK_BYTES,
        },
    }
}

pub fn offer() -> conduit_core::CapabilityOffer {
    BackOfferBuilder::new(
        kind(),
        Back {
            capability_id: CapabilityId::from("capability/browser-dom-mask"),
            execution_profile_id: ExecutionProfileId::from("browser/mask@1"),
            implementation_id: ImplementationId::from("implementation/browser-dom-mask"),
            artifact_id: ArtifactId::from("artifact/browser-runtime"),
            host_calls: vec![HostCallRequirement {
                contract_id: HostCallContractId::from(MASK_OPERATION),
                target_kind: Some(kind_id("presentation/browser-dom-mask")),
                maximum_in_flight: 1,
                maximum_input_bytes: MASK_BYTES,
                maximum_output_bytes: MASK_BYTES,
            }],
            resource_requirements: vec![resource_requirement(PRESENTATION_RESOURCE_CLASS, 1)],
            authority_requirements: vec![],
        },
    )
    .build()
}

pub const MASK_SOURCE: &str = "plot browser-graphical (\n >> face: Presentation\n interaction: FaceInteraction...| >>\n show: Show >>\n) {\n mask: presentation/browser-dom-mask\n face >> mask.presentation\n mask.interaction >> interaction\n mask.show >> show\n}\n";
pub const ALTERNATE_MASK_SOURCE: &str = "plot browser-graphical-alternate (\n >> face: Presentation\n interaction: FaceInteraction...| >>\n show: Show >>\n) {\n mask: presentation/browser-dom-mask\n face >> mask.presentation\n mask.interaction >> interaction\n mask.show >> show\n}\n";

#[derive(Clone, Copy)]
struct OwnerCarrierForeLimits {
    face_bytes: u32,
    show_bytes: u32,
}

pub fn planned_mask(
    host: &HostAdvertisement,
    source: &str,
    name: &str,
) -> Result<PlannedMaskPlot, String> {
    planned_mask_with_fore_limits(host, source, name, None)
}

fn planned_mask_with_fore_limits(
    host: &HostAdvertisement,
    source: &str,
    name: &str,
    owner_carrier: Option<OwnerCarrierForeLimits>,
) -> Result<PlannedMaskPlot, String> {
    let definition = kind();
    let mut startup = StartupCatalog::new();
    install_mask_plot_value_aliases(&mut startup).map_err(|error| format!("{error:?}"))?;
    startup
        .insert(KindSignature {
            kind: definition.kind_id.as_str().into(),
            startup_parameters: vec![],
        })
        .map_err(|error| format!("{error:?}"))?;
    let mut profiles = ProfileCatalog::new();
    profiles
        .insert_kind(definition.clone())
        .map_err(|error| format!("{error:?}"))?;
    let (mask, authoring) = mask_plot(source, name, &startup, &profiles)?;
    let syntax = parse_syntax_document(source);
    if !syntax.diagnostics.is_empty() {
        return Err(format!("{:?}", syntax.diagnostics));
    }
    let placements = conduit_planner::default_expanded_placements(
        &authoring.expanded,
        core::slice::from_ref(host),
    )
    .map_err(|error| format!("{error:?}"))?;
    let mut boundary_limits = BTreeMap::new();
    for (direction, bindings) in [
        (PortDirection::Input, authoring.input_bindings.as_slice()),
        (PortDirection::Output, authoring.output_bindings.as_slice()),
    ] {
        for binding in bindings {
            let (item_capacity, byte_capacity) = match owner_carrier {
                Some(limits)
                    if direction == PortDirection::Input
                        && binding.front_port_id == mask.face_input.front_port_id =>
                {
                    (1, limits.face_bytes)
                }
                Some(limits)
                    if direction == PortDirection::Output
                        && binding.front_port_id == mask.show_output.front_port_id =>
                {
                    (1, limits.show_bytes)
                }
                Some(_)
                    if direction == PortDirection::Output
                        && binding.front_port_id == mask.interaction_output.front_port_id =>
                {
                    (1, conduit_presentation::MAX_FACE_INTERACTION_BYTES as u32)
                }
                Some(_) => return Err("unexpected browser Mask Fore boundary".into()),
                None => (4, MASK_BYTES),
            };
            boundary_limits.insert(
                conduit_planner::ForeBoundaryKey {
                    direction,
                    front_port_id: binding.front_port_id.clone(),
                    track: binding.track,
                },
                conduit_planner::ConnectionQueueLimits {
                    item_capacity,
                    byte_capacity,
                },
            );
        }
    }
    let plan = conduit_planner::plan_expanded_authoring_with_options(
        &authoring,
        core::slice::from_ref(host),
        &placements,
        &[],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 4,
            connection_byte_capacity: MASK_BYTES,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundary_limits,
    )
    .map_err(|error| format!("{error:?}"))?;
    let planned = PlannedMaskPlot::admit(&mask, &plan).map_err(|error| format!("{error:?}"))?;
    Ok(planned)
}

fn mask_plot(
    source: &str,
    name: &str,
    startup: &StartupCatalog,
    profiles: &ProfileCatalog,
) -> Result<(MaskPlot, conduit_plot::ExpandedAuthoringPlot), String> {
    let syntax = parse_syntax_document(source);
    if !syntax.diagnostics.is_empty() {
        return Err(format!("{:?}", syntax.diagnostics));
    }
    let checked = check_syntax_document(&syntax, startup).map_err(|error| format!("{error:?}"))?;
    let authoring = expand_canonical_plot_for_authoring(&checked, name, profiles)
        .map_err(|error| format!("{error:?}"))?;
    let mask = MaskPlot::admit(&authoring).map_err(|error| format!("{error:?}"))?;
    Ok((mask, authoring))
}
