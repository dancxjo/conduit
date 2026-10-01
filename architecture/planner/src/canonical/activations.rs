use super::*;
use alloc::boxed::Box;
use conduit_core::{
    PlannedActivation, PlannedActivationCancellationPolicy, PlannedActivationEffectMultiplicity,
    PlannedActivationEntry, PlannedActivationFront, PlannedActivationLimits,
    PlannedActivationTerminalPolicy, PlannedFoldAbnormalPolicy, PlannedFoldActivation,
    PlannedFoldCancellationPolicy, PlannedFoldTerminalPolicy, PlannedScanAbnormalPolicy,
    PlannedScanActivation, PlannedScanCancellationPolicy, PlannedScanTerminalPolicy,
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
        if child.fragments.iter().any(|fragment| {
            fragment.host_id != owner.host_id
                || fragment.boot_id != owner.boot_id
                || fragment.offer_generation != owner.offer_generation
        }) {
            return Err(PlannerError::InvalidFormIdentity(
                "activation selected Plan must be prepared on the coordinator owner host, boot, and offer generation".into(),
            ));
        }
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
        let item = PlannedActivationFront {
            front_port_id: activation.input.port_id.clone(),
            value_kind: activation.input.value_kind.clone(),
            abnormal_kind: activation.input.abnormal_kind.clone(),
        };
        let output = PlannedActivationFront {
            front_port_id: activation.output.port_id.clone(),
            value_kind: activation.output.value_kind.clone(),
            abnormal_kind: activation.output.abnormal_kind.clone(),
        };
        let limits = PlannedActivationLimits {
            maximum_active: 1,
            maximum_queue_items: 1,
            maximum_queue_bytes: owner.limits.max_queue_bytes,
            maximum_items: activation.mode.maximum_items(),
        };
        if matches!(
            activation.mode,
            conduit_form::ActivationSyntax::Fold { .. }
                | conduit_form::ActivationSyntax::Scan { .. }
        ) {
            let accumulator = activation.accumulator_input.as_ref().ok_or_else(|| {
                PlannerError::InvalidFormIdentity(
                    "fold activation lost its accumulator front".into(),
                )
            })?;
            let initial = activation
                .initial_accumulator_bytes
                .clone()
                .ok_or_else(|| {
                    PlannerError::InvalidFormIdentity(
                        "fold activation lost canonical initial bytes".into(),
                    )
                })?;
            let law = owner
                .semantic_contract
                .laws
                .iter()
                .find_map(|law| match (&activation.mode, law) {
                    (
                        conduit_form::ActivationSyntax::Fold { .. },
                        conduit_core::KindSemanticLaw::FlowFold(law),
                    ) => Some((
                        &law.initial_accumulator,
                        &law.item,
                        &law.accumulator,
                        law.maximum_items,
                        law.accumulator.maximum_bytes,
                        law.item.maximum_bytes,
                    )),
                    (
                        conduit_form::ActivationSyntax::Scan { .. },
                        conduit_core::KindSemanticLaw::FlowScan(law),
                    ) => Some((
                        &law.initial_accumulator,
                        &law.item,
                        &law.accumulator,
                        law.maximum_items,
                        law.accumulator.maximum_bytes,
                        law.item.maximum_bytes,
                    )),
                    _ => None,
                })
                .ok_or_else(|| {
                    PlannerError::InvalidFormIdentity(
                        "fold or scan coordinator has no exact progression law".into(),
                    )
                })?;
            if law.0 != &initial
                || law.1.value_kind != item.value_kind
                || law.2.value_kind != accumulator.value_kind
                || law.3 != limits.maximum_items
            {
                return Err(PlannerError::InvalidFormIdentity(
                    "progression activation differs from its selected coordinator law".into(),
                ));
            }
            let accumulator_input = PlannedActivationFront {
                front_port_id: accumulator.port_id.clone(),
                value_kind: accumulator.value_kind.clone(),
                abnormal_kind: accumulator.abnormal_kind.clone(),
            };
            if matches!(activation.mode, conduit_form::ActivationSyntax::Scan { .. }) {
                planned.push(PlannedActivationEntry::Scan(PlannedScanActivation {
                    activation_id: activation.activation_id.clone(),
                    owner_placement_id: owner.placement_id.clone(),
                    selected_plan_id: child.plan_id.clone(),
                    selected_plan: Box::new(child),
                    accumulator_input,
                    item_input: item,
                    output,
                    initial_accumulator: initial,
                    retained_accumulator_bytes: law.4,
                    retained_item_bytes: law.5,
                    limits,
                    terminal_policy: PlannedScanTerminalPolicy::DrainThenCloseWithoutExtraEmission,
                    abnormal_policy: PlannedScanAbnormalPolicy::DiscardAccumulatorAndPropagateExact,
                    cancellation_policy:
                        PlannedScanCancellationPolicy::DiscardAccumulatorWithoutEmission,
                    effect_multiplicity: PlannedActivationEffectMultiplicity::OncePerAcceptedInput,
                    per_activation_sign_budget: sign_budget,
                }));
                continue;
            }
            planned.push(PlannedActivationEntry::Fold(PlannedFoldActivation {
                activation_id: activation.activation_id.clone(),
                owner_placement_id: owner.placement_id.clone(),
                selected_plan_id: child.plan_id.clone(),
                selected_plan: Box::new(child),
                accumulator_input,
                item_input: item,
                output,
                initial_accumulator: initial,
                retained_accumulator_bytes: law.4,
                retained_item_bytes: law.5,
                limits,
                terminal_policy: PlannedFoldTerminalPolicy::DrainThenEmitAccumulatorExactlyOnce,
                abnormal_policy: PlannedFoldAbnormalPolicy::DiscardAccumulatorAndPropagateExact,
                cancellation_policy:
                    PlannedFoldCancellationPolicy::DiscardAccumulatorWithoutEmission,
                effect_multiplicity: PlannedActivationEffectMultiplicity::OncePerAcceptedInput,
                per_activation_sign_budget: sign_budget,
            }));
            continue;
        }
        let offered_maximum =
            owner
                .semantic_contract
                .laws
                .iter()
                .find_map(|law| match (&activation.mode, law) {
                    (
                        conduit_form::ActivationSyntax::Each { .. },
                        conduit_core::KindSemanticLaw::FlowEach(law),
                    ) => Some(law.maximum_items),
                    (
                        conduit_form::ActivationSyntax::Select { .. },
                        conduit_core::KindSemanticLaw::FlowSelect(law),
                    ) => Some(law.maximum_items),
                    _ => None,
                });
        if offered_maximum != Some(limits.maximum_items) {
            return Err(PlannerError::InvalidFormIdentity(format!(
                "activation maximum-items {} differs from its coordinator law {:?}",
                limits.maximum_items, offered_maximum
            )));
        }
        planned.push(PlannedActivationEntry::Unary(PlannedActivation {
            activation_id: activation.activation_id.clone(),
            owner_placement_id: owner.placement_id.clone(),
            selected_plan_id: child.plan_id.clone(),
            selected_plan: Box::new(child),
            input: item,
            output,
            limits,
            terminal_policy: PlannedActivationTerminalPolicy::DrainThenPropagateExact,
            cancellation_policy:
                PlannedActivationCancellationPolicy::CancelActiveAndRejectLateCompletion,
            effect_multiplicity: PlannedActivationEffectMultiplicity::OncePerAcceptedInput,
            per_activation_sign_budget: sign_budget,
        }));
    }

    Ok(conduit_core::seal_plan_with_activation_entries(
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
