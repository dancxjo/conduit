use super::*;
use alloc::boxed::Box;
use conduit_core::{
    PlannedActivation, PlannedActivationCancellationPolicy, PlannedActivationEffectMultiplicity,
    PlannedActivationFront, PlannedActivationLimits, PlannedActivationTerminalPolicy,
    SignStorageBudget,
};

#[allow(clippy::too_many_arguments)]
pub fn plan_expanded_canonical_with_activations(
    document: &CheckedSyntaxDocument,
    form: &ExpandedCanonicalForm,
    catalog: &ProfileCatalog,
    backs: &CanonicalBackCatalog,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
    bases: &[BaseImplementationId],
    options: PlanningOptions<'_>,
) -> Result<Plan, PlannerError> {
    let plan = plan_expanded_canonical_with_options(form, hosts, placements, bases, options)?;
    attach_activations(
        document, form, catalog, backs, hosts, bases, options, plan, 0,
    )
}

#[allow(clippy::too_many_arguments)]
fn attach_activations(
    document: &CheckedSyntaxDocument,
    form: &ExpandedCanonicalForm,
    catalog: &ProfileCatalog,
    backs: &CanonicalBackCatalog,
    hosts: &[HostAdvertisement],
    bases: &[BaseImplementationId],
    options: PlanningOptions<'_>,
    plan: Plan,
    depth: u8,
) -> Result<Plan, PlannerError> {
    if form.activations.is_empty() {
        return Ok(plan);
    }
    if depth >= conduit_core::MAXIMUM_PLANNED_ACTIVATION_DEPTH {
        return Err(PlannerError::InvalidFormIdentity(
            "planned activation nesting exceeds the finite architecture bound".into(),
        ));
    }

    let mut planned = Vec::with_capacity(form.activations.len());
    for activation in &form.activations {
        let authoring = expand_canonical_form_for_authoring_with_backs(
            document,
            &activation.selected_form,
            catalog,
            backs,
        )
        .map_err(|error| PlannerError::InvalidFormIdentity(error.to_string()))?;
        if authoring.expanded.checked_form_id != activation.selected_checked_form_id {
            return Err(PlannerError::InvalidFormIdentity(
                "activation selected Form identity changed after checked expansion".into(),
            ));
        }
        let child_placements = default_expanded_placements(&authoring.expanded, hosts)?;
        let boundary_limits = exact_boundary_limits(&authoring)?;
        let child = plan_expanded_authoring_with_options(
            &authoring,
            hosts,
            &child_placements,
            bases,
            options,
            &boundary_limits,
        )?;
        let child = attach_activations(
            document,
            &authoring.expanded,
            catalog,
            backs,
            hosts,
            bases,
            options,
            child,
            depth + 1,
        )?;

        let owner = plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.placements)
            .find(|placement| placement.gear_id == activation.owner_gear_id)
            .ok_or_else(|| {
                PlannerError::InvalidFormIdentity(
                    "activation coordinator has no exact planned placement".into(),
                )
            })?;
        if owner.limits.max_queue_bytes == 0 {
            return Err(PlannerError::InvalidConnectionBudget(
                "activation coordinator offers no finite queue bytes".into(),
            ));
        }
        let sign_budget = child
            .fragments
            .iter()
            .try_fold(
                SignStorageBudget {
                    item_capacity: 0,
                    byte_capacity: 0,
                },
                |mut total, fragment| {
                    total.item_capacity = total
                        .item_capacity
                        .checked_add(fragment.sign_storage_budget.item_capacity)?;
                    total.byte_capacity = total
                        .byte_capacity
                        .checked_add(fragment.sign_storage_budget.byte_capacity)?;
                    Some(total)
                },
            )
            .ok_or_else(|| {
                PlannerError::InvalidConnectionBudget(
                    "activation Sign storage sum exceeds architecture bounds".into(),
                )
            })?;
        planned.push(PlannedActivation {
            activation_id: activation.activation_id.clone(),
            owner_placement_id: owner.placement_id.clone(),
            selected_plan_id: child.plan_id.clone(),
            selected_plan: Box::new(child),
            input: PlannedActivationFront {
                front_port_id: activation.input.port_id.clone(),
                value_kind: activation.input.value_kind.clone(),
                abnormal_kind: activation.input.abnormal_kind.clone(),
            },
            output: PlannedActivationFront {
                front_port_id: activation.output.port_id.clone(),
                value_kind: activation.output.value_kind.clone(),
                abnormal_kind: activation.output.abnormal_kind.clone(),
            },
            limits: PlannedActivationLimits {
                maximum_active: 1,
                maximum_queue_items: 1,
                maximum_queue_bytes: owner.limits.max_queue_bytes,
            },
            terminal_policy: PlannedActivationTerminalPolicy::DrainThenPropagateExact,
            cancellation_policy:
                PlannedActivationCancellationPolicy::CancelActiveAndRejectLateCompletion,
            effect_multiplicity: PlannedActivationEffectMultiplicity::OncePerAcceptedInput,
            per_activation_sign_budget: sign_budget,
        });
    }

    Ok(conduit_core::seal_plan_with_activations(
        FormIdentity {
            source_document_id: plan.source_document_id,
            checked_form_id: plan.checked_form_id,
            expanded_form_id: plan.expanded_form_id,
        },
        plan.completion_policy,
        plan.realization_backs,
        planned,
        plan.fragments,
    ))
}

fn exact_boundary_limits(
    form: &ExpandedAuthoringForm,
) -> Result<BTreeMap<ForeBoundaryKey, ConnectionQueueLimits>, PlannerError> {
    let mut limits = BTreeMap::new();
    for (direction, bindings, descriptors) in [
        (
            conduit_core::PortDirection::Input,
            form.input_bindings.as_slice(),
            form.front.inputs(),
        ),
        (
            conduit_core::PortDirection::Output,
            form.output_bindings.as_slice(),
            form.front.outputs(),
        ),
    ] {
        for binding in bindings {
            let descriptor = descriptors
                .iter()
                .find(|port| port.port_id == binding.front_port_id)
                .ok_or_else(|| {
                    PlannerError::InvalidFormIdentity(
                        "activation selected Form front descriptor is missing".into(),
                    )
                })?;
            let location = match (direction, binding.track) {
                (
                    conduit_core::PortDirection::Input,
                    conduit_core::ConnectionTrack::AbnormalTerminal,
                ) => conduit_core::FrontValueLocation::InputAbnormal(binding.front_port_id.clone()),
                (
                    conduit_core::PortDirection::Output,
                    conduit_core::ConnectionTrack::AbnormalTerminal,
                ) => {
                    conduit_core::FrontValueLocation::OutputAbnormal(binding.front_port_id.clone())
                }
                (conduit_core::PortDirection::Input, _) => {
                    conduit_core::FrontValueLocation::Input(binding.front_port_id.clone())
                }
                (conduit_core::PortDirection::Output, _) => {
                    conduit_core::FrontValueLocation::Output(binding.front_port_id.clone())
                }
            };
            let bytes = form
                .front
                .value_contract(&location)
                .map(|contract| contract.maximum_bytes)
                .unwrap_or_else(|| descriptor.value_kind.as_str().len() as u32)
                .max(1);
            limits.insert(
                ForeBoundaryKey {
                    direction,
                    front_port_id: binding.front_port_id.clone(),
                    track: binding.track,
                },
                ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: bytes,
                },
            );
        }
    }
    Ok(limits)
}
