//! Seal exact external Fore contracts after expanded planning.
use super::*;

/// Seal a root's exact Fore together with its recursively planned activations.
/// The checked document is required because an expanded parent does not carry
/// the selected child Plot source needed to prepare its Plan.
#[allow(clippy::too_many_arguments)]
pub fn plan_expanded_authoring_with_activations(
    document: &CheckedSyntaxDocument,
    plot: &ExpandedAuthoringPlot,
    catalog: &ProfileCatalog,
    backs: &CanonicalBackCatalog,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
    bases: &[BaseImplementationId],
    options: PlanningOptions<'_>,
    boundary_limits: &BTreeMap<ForeBoundaryKey, ConnectionQueueLimits>,
) -> Result<Plan, PlannerError> {
    let checked = expand_canonical_plot_for_authoring_with_backs(
        document,
        &plot.expanded.name,
        catalog,
        backs,
    )
    .map_err(|error| PlannerError::InvalidPlotIdentity(error.to_string()))?;
    if &checked != plot {
        return Err(PlannerError::InvalidPlotIdentity(
            "authoring Plot differs from its exact checked source and Back catalog".into(),
        ));
    }
    let plan = plan_expanded_canonical_with_activations(
        document,
        &plot.expanded,
        catalog,
        backs,
        hosts,
        placements,
        bases,
        options,
    )?;
    seal_fore(plot, plan, boundary_limits)
}

pub fn plan_expanded_authoring_with_options(
    plot: &ExpandedAuthoringPlot,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
    bases: &[BaseImplementationId],
    options: PlanningOptions<'_>,
    boundary_limits: &BTreeMap<ForeBoundaryKey, ConnectionQueueLimits>,
) -> Result<Plan, PlannerError> {
    let plan =
        plan_expanded_canonical_with_options(&plot.expanded, hosts, placements, bases, options)?;
    seal_fore(plot, plan, boundary_limits)
}

/// Admit every internal Cord and external Fore with its own exact finite queue.
pub fn plan_expanded_authoring_with_connection_limits(
    plot: &ExpandedAuthoringPlot,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
    bases: &[BaseImplementationId],
    options: PlanningOptions<'_>,
    connection_limits: &BTreeMap<ConnectionEndpoints, ConnectionQueueLimits>,
    boundary_limits: &BTreeMap<ForeBoundaryKey, ConnectionQueueLimits>,
) -> Result<Plan, PlannerError> {
    let plan = plan_expanded_canonical_with_connection_limits(
        &plot.expanded,
        hosts,
        placements,
        bases,
        options,
        connection_limits,
    )?;
    seal_fore(plot, plan, boundary_limits)
}

fn seal_fore(
    plot: &ExpandedAuthoringPlot,
    mut plan: Plan,
    boundary_limits: &BTreeMap<ForeBoundaryKey, ConnectionQueueLimits>,
) -> Result<Plan, PlannerError> {
    attach_fore(
        &plot.front,
        &plot.input_bindings,
        &plot.output_bindings,
        &mut plan.fragments,
        boundary_limits,
    )?;
    Ok(conduit_core::seal_plan_with_activation_entries(
        PlotIdentity {
            source_document_id: plan.source_document_id,
            checked_plot_id: plan.checked_plot_id,
            expanded_plot_id: plan.expanded_plot_id,
        },
        plan.completion_policy,
        plan.realization_backs,
        plan.activations,
        plan.fragments,
    ))
}

/// Shared exact Fore checks for rich authoring and authenticated indexed Make
/// material. The caller seals the resulting fragments in their original scope.
pub(crate) fn attach_fore(
    front: &conduit_core::CheckedFront,
    input_bindings: &[conduit_plot::AuthoringFrontBinding],
    output_bindings: &[conduit_plot::AuthoringFrontBinding],
    fragments: &mut [conduit_core::PlanFragment],
    boundary_limits: &BTreeMap<ForeBoundaryKey, ConnectionQueueLimits>,
) -> Result<(), PlannerError> {
    let count = input_bindings.len() + output_bindings.len();
    // Preserve all-or-nothing attachment while avoiding a second indexed port
    // array for the common local fragment. Every port is checked by visit_fore.
    if fragments.len() == 1 {
        let mut ports = Vec::with_capacity(count);
        visit_fore(
            front,
            input_bindings,
            output_bindings,
            fragments,
            boundary_limits,
            |index, port| {
                debug_assert_eq!(index, 0);
                ports.push(port);
                Ok(())
            },
        )?;
        if fragments[0].fore_ports.is_empty() {
            fragments[0].fore_ports = ports;
        } else {
            fragments[0].fore_ports.extend(ports);
        }
        fragments[0].fore_ports.sort_by(|left, right| {
            (left.direction as u8, &left.front_port_id, left.track).cmp(&(
                right.direction as u8,
                &right.front_port_id,
                right.track,
            ))
        });
        return Ok(());
    }
    let mut ports = Vec::with_capacity(count);
    visit_fore(
        front,
        input_bindings,
        output_bindings,
        fragments,
        boundary_limits,
        |index, port| {
            ports.push((index, port));
            Ok(())
        },
    )?;
    for (index, port) in ports {
        fragments[index].fore_ports.push(port);
    }
    for fragment in fragments {
        fragment.fore_ports.sort_by(|left, right| {
            (left.direction as u8, &left.front_port_id, left.track).cmp(&(
                right.direction as u8,
                &right.front_port_id,
                right.track,
            ))
        });
    }
    Ok(())
}

/// Construct one exact Fore at a time for sealing or read-only comparison.
/// Verification never clones the selected graph or its configuration payloads.
pub(crate) fn visit_fore(
    front: &conduit_core::CheckedFront,
    input_bindings: &[conduit_plot::AuthoringFrontBinding],
    output_bindings: &[conduit_plot::AuthoringFrontBinding],
    fragments: &[conduit_core::PlanFragment],
    boundary_limits: &BTreeMap<ForeBoundaryKey, ConnectionQueueLimits>,
    mut visit: impl FnMut(usize, PlannedForePort) -> Result<(), PlannerError>,
) -> Result<(), PlannerError> {
    let expected = input_bindings
        .iter()
        .map(|binding| ForeBoundaryKey {
            direction: conduit_core::PortDirection::Input,
            front_port_id: binding.front_port_id.clone(),
            track: binding.track,
        })
        .chain(output_bindings.iter().map(|binding| ForeBoundaryKey {
            direction: conduit_core::PortDirection::Output,
            front_port_id: binding.front_port_id.clone(),
            track: binding.track,
        }))
        .collect::<BTreeSet<_>>()
        .len();
    if expected != boundary_limits.len() {
        return Err(PlannerError::InvalidPlotIdentity(
            "every external Fore binding requires one exact queue limit".into(),
        ));
    }
    let mut seen = BTreeSet::new();
    for (direction, bindings, descriptors) in [
        (
            conduit_core::PortDirection::Input,
            input_bindings,
            front.inputs(),
        ),
        (
            conduit_core::PortDirection::Output,
            output_bindings,
            front.outputs(),
        ),
    ] {
        for binding in bindings {
            let key = ForeBoundaryKey {
                direction,
                front_port_id: binding.front_port_id.clone(),
                track: binding.track,
            };
            if !seen.insert(key.clone())
                && (direction != conduit_core::PortDirection::Input
                    || binding.track != conduit_core::ConnectionTrack::Payload)
            {
                return Err(PlannerError::InvalidPlotIdentity(format!(
                    "external Fore port '{}' track '{}' has unsupported multiple internal bindings",
                    binding.front_port_id.as_str(),
                    binding.track.as_str(),
                )));
            }
            let limits = boundary_limits.get(&key).ok_or_else(|| {
                PlannerError::InvalidPlotIdentity(format!(
                    "external Fore port '{}' track '{}' has no queue limit",
                    binding.front_port_id.as_str(),
                    binding.track.as_str(),
                ))
            })?;
            if limits.item_capacity == 0 || limits.byte_capacity == 0 {
                return Err(PlannerError::InvalidPlotIdentity(format!(
                    "external Fore port '{}' has a zero queue limit",
                    binding.front_port_id.as_str(),
                )));
            }
            let descriptor = descriptors
                .iter()
                .find(|port| port.port_id == binding.front_port_id)
                .ok_or_else(|| {
                    PlannerError::InvalidPlotIdentity("external Fore descriptor is missing".into())
                })?;
            let value_contract = (!matches!(
                binding.track,
                conduit_core::ConnectionTrack::NormalClose
                    | conduit_core::ConnectionTrack::Quiescence
            ))
            .then(|| {
                let location = match direction {
                    conduit_core::PortDirection::Input => {
                        conduit_core::FrontValueLocation::Input(binding.front_port_id.clone())
                    }
                    conduit_core::PortDirection::Output => {
                        conduit_core::FrontValueLocation::Output(binding.front_port_id.clone())
                    }
                };
                front
                    .value_contracts()
                    .iter()
                    .find(|contract| contract.location == location)
                    .map(|contract| contract.contract.clone())
            })
            .flatten();
            let (fragment_index, fragment) = fragments
                .iter()
                .enumerate()
                .find(|(_, fragment)| {
                    fragment
                        .placements
                        .iter()
                        .any(|placement| placement.gear_id == binding.gear_id)
                })
                .ok_or_else(|| {
                    PlannerError::InvalidPlotIdentity("external Fore placement is missing".into())
                })?;
            let placement = fragment
                .placements
                .iter()
                .find(|placement| placement.gear_id == binding.gear_id)
                .expect("fragment selected by this placement");
            let internal = match direction {
                conduit_core::PortDirection::Input => &placement.inputs,
                conduit_core::PortDirection::Output => &placement.outputs,
            }
            .iter()
            .find(|port| port.port_id == binding.gear_port_id)
            .ok_or_else(|| {
                PlannerError::InvalidPlotIdentity("external Fore internal port is missing".into())
            })?;
            let internal_matches = match binding.track {
                conduit_core::ConnectionTrack::Payload => {
                    internal.value_kind == descriptor.value_kind
                        && internal.temporal == descriptor.temporal
                        && internal.abnormal_kind == descriptor.abnormal_kind
                }
                conduit_core::ConnectionTrack::AbnormalTerminal => {
                    internal.abnormal_kind.as_ref() == Some(&descriptor.value_kind)
                        && descriptor.temporal == conduit_core::PortTemporal::Value
                }
                conduit_core::ConnectionTrack::NormalClose => {
                    matches!(
                        internal.temporal,
                        conduit_core::PortTemporal::Flow { closes: true }
                    ) && descriptor.value_kind.as_str() == conduit_core::EMPTY_INFO_ID
                        && descriptor.temporal == conduit_core::PortTemporal::Value
                }
                conduit_core::ConnectionTrack::Quiescence => {
                    matches!(internal.temporal, conduit_core::PortTemporal::Flow { .. })
                        && descriptor.value_kind.as_str() == conduit_core::EMPTY_INFO_ID
                        && descriptor.temporal == conduit_core::PortTemporal::Value
                }
            };
            if !internal_matches {
                return Err(PlannerError::InvalidPlotIdentity(format!(
                    "external Fore port '{}' does not match its selected Back",
                    binding.front_port_id.as_str(),
                )));
            }
            if !matches!(
                binding.track,
                conduit_core::ConnectionTrack::NormalClose
                    | conduit_core::ConnectionTrack::Quiescence
            ) {
                let internal_front = placement.checked_port_front();
                let internal_location = match (direction, binding.track) {
                    (
                        conduit_core::PortDirection::Input,
                        conduit_core::ConnectionTrack::AbnormalTerminal,
                    ) => conduit_core::FrontValueLocation::InputAbnormal(
                        binding.gear_port_id.clone(),
                    ),
                    (
                        conduit_core::PortDirection::Output,
                        conduit_core::ConnectionTrack::AbnormalTerminal,
                    ) => conduit_core::FrontValueLocation::OutputAbnormal(
                        binding.gear_port_id.clone(),
                    ),
                    (conduit_core::PortDirection::Input, _) => {
                        conduit_core::FrontValueLocation::Input(binding.gear_port_id.clone())
                    }
                    (conduit_core::PortDirection::Output, _) => {
                        conduit_core::FrontValueLocation::Output(binding.gear_port_id.clone())
                    }
                };
                let internal_contract = internal_front
                    .value_contracts()
                    .iter()
                    .find(|contract| contract.location == internal_location)
                    .map(|contract| contract.contract.clone());
                if internal_contract != value_contract {
                    return Err(PlannerError::InvalidPlotIdentity(format!(
                        "external Fore port '{}' value contract does not match its selected Back",
                        binding.front_port_id.as_str(),
                    )));
                }
            }
            if limits.item_capacity > placement.limits.max_queue_items
                || limits.byte_capacity > placement.limits.max_queue_bytes
            {
                return Err(PlannerError::QueueRequirementAboveHostLimit(format!(
                    "external Fore port '{}' exceeds selected Back queue limits",
                    binding.front_port_id.as_str(),
                )));
            }
            visit(
                fragment_index,
                PlannedForePort {
                    front_port_id: binding.front_port_id.clone(),
                    direction,
                    placement_id: placement.placement_id.clone(),
                    gear_port_id: binding.gear_port_id.clone(),
                    value_kind: descriptor.value_kind.clone(),
                    value_contract,
                    abnormal_kind: descriptor.abnormal_kind.clone(),
                    track: binding.track,
                    temporal: descriptor.temporal,
                    pressure_policy: conduit_core::DeliveryPressurePolicy::PreserveOrder,
                    item_capacity: limits.item_capacity,
                    byte_capacity: limits.byte_capacity,
                    selected_line: None,
                },
            )?;
        }
    }
    Ok(())
}
