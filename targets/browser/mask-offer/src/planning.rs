//! Ordinary Source/Root expansion, planning and independent configured re-admission.
use crate::{MASK_BYTES, configured_sources, kind};
use conduit_core::{HostAdvertisement, PortDirection, kind_id};
use conduit_face::{MaskPlot, PlannedMaskPlot, install_mask_plot_value_aliases};
use conduit_plot::{
    KindSignature, ProfileCatalog, StartupCatalog, check_syntax_document,
    expand_canonical_plot_for_authoring, parse_syntax_document,
};
use std::{collections::BTreeMap, string::String};

#[derive(Clone, Copy)]
pub struct OwnerCarrierForeLimits {
    pub face_bytes: u32,
    pub show_bytes: u32,
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

pub(super) fn planned_mask_configuration(
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
    planned_mask_configuration_with_look(host, source, name, owner_carrier, None)
}

pub fn planned_mask_configuration_with_look(
    host: &HostAdvertisement,
    source: &str,
    name: &str,
    owner_carrier: Option<OwnerCarrierForeLimits>,
    selected_look: Option<&conduit_face::LookInfo>,
) -> Result<
    (
        PlannedMaskPlot,
        Option<conduit_face::ConfiguredMaskConfiguration>,
    ),
    String,
> {
    let configured = configured_sources::contains(source);
    if selected_look.is_some()
        && (source != conduit_face::HISTORY_COMPONENT_MASK_SOURCE
            || name != conduit_face::HISTORY_COMPONENT_MASK_ROOT)
    {
        return Err("explicit Look startup is outside the reviewed history Root".into());
    }
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
            ("LookInfo", conduit_face::look_type()),
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
    let checked = check_syntax_document(&parse_syntax_document(source), &startup)
        .map_err(|error| format!("{error:?}"))?;
    let selected_startup = selected_look
        .map(|look| {
            use conduit_plot::rust_binding::NativeRustBinding;
            let value = look
                .clone()
                .into_structured()
                .map_err(|error| format!("{error:?}"))?;
            conduit_plot::PreparedRootStartupInputs::prepare(
                &checked,
                name,
                &profiles,
                &[conduit_plot::CheckedStartupBinding {
                    name: "look".into(),
                    value_type: "LookInfo".into(),
                    value: conduit_plot::CanonicalStartupValue::Structured(
                        conduit_plot::CanonicalStructuredStartupValue::from_owned_checked_value(
                            value,
                        ),
                    ),
                }],
            )
            .map_err(|error| format!("history Look startup: {error:?}"))
        })
        .transpose()?;
    let authoring = match selected_startup.as_ref() {
        Some(inputs) => conduit_plot::expand_canonical_plot_for_authoring_with_startup(
            &checked,
            name,
            &profiles,
            &conduit_plot::CanonicalBackCatalog::new(),
            inputs,
        ),
        None => expand_canonical_plot_for_authoring(&checked, name, &profiles),
    }
    .map_err(|error| format!("{error:?}"))?;
    let mask = MaskPlot::admit(&authoring).map_err(|error| format!("{error:?}"))?;
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
        let selection = conduit_face::ConfiguredMaskPlanningInputs {
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
        };
        let backs = conduit_plot::CanonicalBackCatalog::new();
        let renderer = kind_id(conduit_face::COMPONENT_MASK_KIND);
        let prepared = if let Some(inputs) = selected_startup.as_ref() {
            conduit_face::prepare_configured_mask_with_startup(
                &checked, &authoring, &profiles, &backs, inputs, &planned, selection, &renderer,
            )
        } else {
            conduit_face::prepare_configured_mask(
                &checked,
                &authoring,
                &profiles,
                &conduit_plot::CanonicalBackCatalog::new(),
                &planned,
                selection,
                &renderer,
            )
        }
        .map_err(|error| format!("configured browser Mask admission: {error:?}"))?;
        Some(prepared.configuration())
    } else {
        None
    };
    Ok((planned, configuration))
}

/// Re-admit the component from current actual Host offers and exact Fore budgets.
/// The sealed owner route remains responsible for external Line bindings; this
/// join compares the entire rendering placement against the ordinary local Plan.
pub fn configured_mask_configuration(
    host: &HostAdvertisement,
    planned: &PlannedMaskPlot,
) -> Result<Option<conduit_face::ConfiguredMaskConfiguration>, String> {
    configured_mask_configuration_with_look(host, planned, None)
}

/// Reproduce from independently retained selection input, never inferred from Plan/configuration.
pub fn configured_mask_configuration_with_look(
    host: &HostAdvertisement,
    planned: &PlannedMaskPlot,
    selected_look: Option<&conduit_face::LookInfo>,
) -> Result<Option<conduit_face::ConfiguredMaskConfiguration>, String> {
    let configured = planned
        .plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .any(|placement| placement.kind_id.as_str() == conduit_face::COMPONENT_MASK_KIND);
    if !configured {
        return if selected_look.is_some() {
            Err("explicit Look requires the reviewed configured history Mask".into())
        } else {
            Ok(None)
        };
    }
    let root = planned.mask.plot_name.as_str();
    let source =
        configured_sources::for_root(root).ok_or("unknown configured component source root")?;
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
    let (expected, configuration) = planned_mask_configuration_with_look(
        host,
        source,
        root,
        Some(OwnerCarrierForeLimits {
            face_bytes: budget(PortDirection::Input, &planned.mask.face_input.front_port_id)?,
            show_bytes: budget(
                PortDirection::Output,
                &planned.mask.show_output.front_port_id,
            )?,
        }),
        selected_look,
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
