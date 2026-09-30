use crate::{
    verify_plan_at_depth, KindId, PlacementId, Plan, PortDirection, PortId, PortTemporal,
    SignStorageBudget,
};
use alloc::{boxed::Box, string::String};
use serde::{Deserialize, Serialize};

pub const MAXIMUM_PLANNED_ACTIVATION_DEPTH: u8 = 8;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedActivationFront {
    pub front_port_id: PortId,
    pub value_kind: KindId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub abnormal_kind: Option<KindId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedActivationLimits {
    pub maximum_active: u16,
    pub maximum_queue_items: u16,
    pub maximum_queue_bytes: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlannedActivationTerminalPolicy {
    DrainThenPropagateExact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlannedActivationCancellationPolicy {
    CancelActiveAndRejectLateCompletion,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedActivation {
    pub activation_id: String,
    pub owner_placement_id: PlacementId,
    pub selected_plan_id: crate::PlanId,
    pub selected_plan: Box<Plan>,
    pub input: PlannedActivationFront,
    pub output: PlannedActivationFront,
    pub limits: PlannedActivationLimits,
    pub terminal_policy: PlannedActivationTerminalPolicy,
    pub cancellation_policy: PlannedActivationCancellationPolicy,
    pub per_activation_sign_budget: SignStorageBudget,
}

pub(crate) fn verify_planned_activations(plan: &Plan, depth: u8) -> bool {
    if depth >= MAXIMUM_PLANNED_ACTIVATION_DEPTH {
        return plan.activations.is_empty();
    }
    let mut identities = alloc::collections::BTreeSet::new();
    plan.activations.iter().all(|activation| {
        !activation.activation_id.is_empty()
            && identities.insert(activation.activation_id.as_str())
            && plan
                .fragments
                .iter()
                .flat_map(|fragment| &fragment.placements)
                .filter(|placement| placement.placement_id == activation.owner_placement_id)
                .count()
                == 1
            && activation.selected_plan_id == activation.selected_plan.plan_id
            && activation.selected_plan_id != plan.plan_id
            && verify_plan_at_depth(&activation.selected_plan, depth + 1)
            && fronts_are_exact(activation)
            && activation.limits.maximum_active == 1
            && activation.limits.maximum_queue_items == 1
            && activation.limits.maximum_queue_bytes > 0
            && activation.input.abnormal_kind == activation.output.abnormal_kind
            && exact_sign_budget(activation)
    })
}

fn fronts_are_exact(activation: &PlannedActivation) -> bool {
    let input = resolve_front(
        &activation.selected_plan,
        &activation.input,
        PortDirection::Input,
    );
    let output = resolve_front(
        &activation.selected_plan,
        &activation.output,
        PortDirection::Output,
    );
    matches!((input, output), (Some(input), Some(output))
        if activation.limits.maximum_queue_bytes >= input.byte_capacity
            && activation.limits.maximum_queue_bytes >= output.byte_capacity)
}

fn resolve_front<'a>(
    plan: &'a Plan,
    expected: &PlannedActivationFront,
    direction: PortDirection,
) -> Option<&'a crate::PlannedForePort> {
    let mut matches = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.fore_ports)
        .filter(|port| {
            port.front_port_id == expected.front_port_id
                && port.direction == direction
                && port.temporal == PortTemporal::Value
                && port.value_kind == expected.value_kind
                && port.abnormal_kind == expected.abnormal_kind
        });
    let result = matches.next()?;
    matches.next().is_none().then_some(result)
}

fn exact_sign_budget(activation: &PlannedActivation) -> bool {
    let Some((items, bytes)) = activation.selected_plan.fragments.iter().try_fold(
        (0u16, 0u32),
        |(items, bytes), fragment| {
            Some((
                items.checked_add(fragment.sign_storage_budget.item_capacity)?,
                bytes.checked_add(fragment.sign_storage_budget.byte_capacity)?,
            ))
        },
    ) else {
        return false;
    };
    activation.per_activation_sign_budget
        == (SignStorageBudget {
            item_capacity: items,
            byte_capacity: bytes,
        })
}
