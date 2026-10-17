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
    PortTemporal, FACE_RESOURCE_CLASS,
};
use conduit_face::{
    install_mask_plot_value_aliases, MaskPlot, PlannedMaskPlot, FACE_INTERACTION_VALUE_KIND,
    FACE_VALUE_KIND, SHOW_VALUE_KIND,
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
        kind_id: kind_id("face/browser-dom-mask"),
        kind_contract_revision: KindIdentity::from("conduit.browser/face-dom-mask@1"),
        inputs: vec![port(
            "face",
            FACE_VALUE_KIND,
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
                target_kind: Some(kind_id("face/browser-dom-mask")),
                maximum_in_flight: 1,
                maximum_input_bytes: MASK_BYTES,
                maximum_output_bytes: MASK_BYTES,
            }],
            resource_requirements: vec![resource_requirement(FACE_RESOURCE_CLASS, 1)],
            authority_requirements: vec![],
        },
    )
    .build()
}

pub const MASK_SOURCE: &str = "plot browser-graphical (\n >> face: Face\n interaction: FaceInteraction...| >>\n show: Show >>\n) {\n mask: face/browser-dom-mask\n face >> mask.face\n mask.interaction >> interaction\n mask.show >> show\n}\n";
pub const ALTERNATE_MASK_SOURCE: &str = "plot browser-graphical-alternate (\n >> face: Face\n interaction: FaceInteraction...| >>\n show: Show >>\n) {\n mask: face/browser-dom-mask\n face >> mask.face\n mask.interaction >> interaction\n mask.show >> show\n}\n";

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
    planned_mask_configuration(host, source, name, owner_carrier).map(|(planned, _)| planned)
}

fn planned_mask_configuration(
    host: &HostAdvertisement,
    source: &str,
    name: &str,
    owner_carrier: Option<OwnerCarrierForeLimits>,
) -> Result<
    (
        PlannedMaskPlot,
        Option<conduit_face::ConfiguredMaskConfiguration>,
    ),
    String,
> {
    let configured = source == conduit_face::TODO_COMPONENT_MASK_SOURCE;
    let definition = if configured {
        conduit_face::component_mask_kind(MASK_BYTES)
    } else {
        kind()
    };
    let mut startup = StartupCatalog::new();
    install_mask_plot_value_aliases(&mut startup).map_err(|error| format!("{error:?}"))?;
    startup
        .insert(KindSignature {
            kind: definition.kind_id.as_str().into(),
            startup_parameters: definition
                .startup_parameters
                .iter()
                .map(|parameter| conduit_plot::StartupParameterSignature {
                    name: parameter.name.clone(),
                    value_type: parameter.value_type.as_str().into(),
                    default: None,
                })
                .collect(),
        })
        .map_err(|error| format!("{error:?}"))?;
    let mut profiles = ProfileCatalog::new();
    profiles
        .insert_kind(definition.clone())
        .map_err(|error| format!("{error:?}"))?;
    if configured {
        for (alias, schema) in [
            ("ComponentRecipeInfo", conduit_face::component_recipe_type()),
            (
                "VisualExpressionInfo",
                conduit_face::visual_expression_type(),
            ),
        ] {
            let profile = schema.profile().map_err(|error| format!("{error:?}"))?;
            startup
                .insert_structured_type(alias, schema.clone())
                .map_err(|error| format!("{error:?}"))?;
            startup
                .ensure_structured_type(profile.value_kind().as_str(), schema)
                .map_err(|error| format!("{error:?}"))?;
        }
    }
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
                    (1, conduit_face::MAX_FACE_INTERACTION_BYTES as u32)
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
    let configuration = if configured {
        let checked =
            check_syntax_document(&syntax, &startup).map_err(|error| format!("{error:?}"))?;
        let empty = BTreeMap::new();
        let lines = BTreeMap::new();
        let prepared = conduit_face::prepare_configured_mask(
            &checked,
            &authoring,
            &profiles,
            &conduit_plot::CanonicalBackCatalog::new(),
            &planned,
            conduit_face::ConfiguredMaskPlanningInputs {
                hosts: core::slice::from_ref(host),
                placements: &placements,
                bases: &[],
                options: conduit_planner::PlanningOptions {
                    connection_bases: &empty,
                    line_candidates: &lines,
                    connection_item_capacity: 4,
                    connection_byte_capacity: MASK_BYTES,
                    authority_grants: &[],
                    protected_resource_grants: &[],
                    line_offers: &[],
                },
                boundary_limits: &boundary_limits,
            },
            &kind_id(conduit_face::COMPONENT_MASK_KIND),
        )
        .map_err(|error| format!("configured browser Mask admission: {error:?}"))?;
        Some(prepared.configuration())
    } else {
        None
    };
    Ok((planned, configuration))
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

/// Actual browser Back of the portable, source-configured component Fore.
pub fn configured_offer() -> conduit_core::CapabilityOffer {
    let original = offer();
    let mut back = Back {
        capability_id: original.capability_id,
        execution_profile_id: original.implementation.execution_profile_id,
        implementation_id: original.implementation.implementation_id,
        artifact_id: original.implementation.artifact_id,
        host_calls: original.host_calls,
        resource_requirements: original.resource_requirements,
        authority_requirements: original.authority_requirements,
    };
    back.capability_id = CapabilityId::from("capability/browser-component-mask");
    back.implementation_id = ImplementationId::from("implementation/browser-component-mask");
    for requirement in &mut back.host_calls {
        requirement.target_kind = Some(kind_id(conduit_face::COMPONENT_MASK_KIND));
    }
    BackOfferBuilder::new(conduit_face::component_mask_kind(MASK_BYTES), back).build()
}

/// Re-admit the component from current actual Host offers and exact Fore budgets.
/// The sealed owner route remains responsible for external Line bindings; this
/// join compares the entire rendering placement against the ordinary local Plan.
pub fn configured_mask_configuration(
    host: &HostAdvertisement,
    planned: &PlannedMaskPlot,
) -> Result<Option<conduit_face::ConfiguredMaskConfiguration>, String> {
    let configured = planned
        .plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .any(|placement| placement.kind_id.as_str() == conduit_face::COMPONENT_MASK_KIND);
    if !configured {
        return Ok(None);
    }
    let root = planned.mask.plot_name.as_str();
    if ![
        "todo-mask-flat-compact",
        "todo-mask-flat-spacious",
        "todo-mask-grouped-compact",
        "todo-mask-grouped-spacious",
    ]
    .contains(&root)
    {
        return Err("unknown configured component source root".into());
    }
    let budget = |direction, id: &conduit_core::PortId| -> Result<u32, String> {
        let mut ports = planned
            .plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.fore_ports)
            .filter(|port| port.direction == direction && &port.front_port_id == id);
        let port = ports.next().ok_or("configured component Fore missing")?;
        if ports.next().is_some() || port.item_capacity != 1 {
            return Err("configured component Fore is ambiguous or not owner-bounded".into());
        }
        Ok(port.byte_capacity)
    };
    let (expected, configuration) = planned_mask_configuration(
        host,
        conduit_face::TODO_COMPONENT_MASK_SOURCE,
        root,
        Some(OwnerCarrierForeLimits {
            face_bytes: budget(PortDirection::Input, &planned.mask.face_input.front_port_id)?,
            show_bytes: budget(
                PortDirection::Output,
                &planned.mask.show_output.front_port_id,
            )?,
        }),
    )?;
    if expected.mask != planned.mask
        || expected
            .plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.placements)
            .collect::<Vec<_>>()
            != planned
                .plan
                .fragments
                .iter()
                .flat_map(|fragment| &fragment.placements)
                .collect::<Vec<_>>()
    {
        return Err("configured renderer differs from the current source and selected Back".into());
    }
    Ok(configuration)
}

#[cfg(test)]
mod configured_tests {
    use super::*;
    use conduit_core::*;
    use conduit_face::*;

    #[test]
    fn actual_checked_todo_roots_use_exact_configured_offer_and_source_recipe() {
        let host = HostAdvertisement {
            protocol_version: PROTOCOL_VERSION,
            host_id: HostId::from("browser/component"),
            boot_id: BootId::from("browser/component/boot"),
            offer_generation: OfferGeneration(1),
            profile: HostProfileId::from("browser"),
            bases: vec![],
            resources: vec![resource_offer("browser/face", FACE_RESOURCE_CLASS, 1)],
            planner_capabilities: vec![],
            capabilities: vec![configured_offer()],
        };
        for recipe in [TodoComponentRecipe::Flat, TodoComponentRecipe::Grouped] {
            let compact = planned_mask(
                &host,
                TODO_COMPONENT_MASK_SOURCE,
                todo_component_mask_root(ComponentMaskLook::Compact, recipe),
            )
            .unwrap();
            let spacious = planned_mask(
                &host,
                TODO_COMPONENT_MASK_SOURCE,
                todo_component_mask_root(ComponentMaskLook::Spacious, recipe),
            )
            .unwrap();
            assert_ne!(compact.plan.plan_id, spacious.plan.plan_id);
            let compact_renderer = &compact.plan.fragments[0].placements[0];
            let spacious_renderer = &spacious.plan.fragments[0].placements[0];
            assert_eq!(
                compact_renderer.capability_id,
                configured_offer().capability_id
            );
            assert_eq!(
                compact_renderer
                    .configuration
                    .iter()
                    .find(|field| field.key == "recipe"),
                spacious_renderer
                    .configuration
                    .iter()
                    .find(|field| field.key == "recipe")
            );
            assert_ne!(
                compact_renderer
                    .configuration
                    .iter()
                    .find(|field| field.key == "expression"),
                spacious_renderer
                    .configuration
                    .iter()
                    .find(|field| field.key == "expression")
            );
        }
    }
}
