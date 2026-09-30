use crate::{
    verify_plan_at_depth, KindId, PlacementId, Plan, PortDirection, PortId, PortTemporal,
    SignStorageBudget,
};
use alloc::{boxed::Box, string::String, vec::Vec};
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
    pub maximum_items: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlannedActivationTerminalPolicy {
    DrainThenPropagateExact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlannedActivationCancellationPolicy {
    CancelActiveAndRejectLateCompletion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlannedActivationEffectMultiplicity {
    OncePerAcceptedInput,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlannedFoldTerminalPolicy {
    DrainThenEmitAccumulatorExactlyOnce,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlannedFoldAbnormalPolicy {
    DiscardAccumulatorAndPropagateExact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlannedFoldCancellationPolicy {
    DiscardAccumulatorWithoutEmission,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlannedScanTerminalPolicy {
    DrainThenCloseWithoutExtraEmission,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlannedScanAbnormalPolicy {
    DiscardAccumulatorAndPropagateExact,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlannedScanCancellationPolicy {
    DiscardAccumulatorWithoutEmission,
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
    pub effect_multiplicity: PlannedActivationEffectMultiplicity,
    pub per_activation_sign_budget: SignStorageBudget,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlannedActivationEntry {
    Unary(PlannedActivation),
    Fold(PlannedFoldActivation),
    Scan(PlannedScanActivation),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedScanActivation {
    pub activation_id: String,
    pub owner_placement_id: PlacementId,
    pub selected_plan_id: crate::PlanId,
    pub selected_plan: Box<Plan>,
    pub accumulator_input: PlannedActivationFront,
    pub item_input: PlannedActivationFront,
    pub output: PlannedActivationFront,
    pub initial_accumulator: Vec<u8>,
    pub retained_accumulator_bytes: u32,
    pub retained_item_bytes: u32,
    pub limits: PlannedActivationLimits,
    pub terminal_policy: PlannedScanTerminalPolicy,
    pub abnormal_policy: PlannedScanAbnormalPolicy,
    pub cancellation_policy: PlannedScanCancellationPolicy,
    pub effect_multiplicity: PlannedActivationEffectMultiplicity,
    pub per_activation_sign_budget: SignStorageBudget,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedFoldActivation {
    pub activation_id: String,
    pub owner_placement_id: PlacementId,
    pub selected_plan_id: crate::PlanId,
    pub selected_plan: Box<Plan>,
    pub accumulator_input: PlannedActivationFront,
    pub item_input: PlannedActivationFront,
    pub output: PlannedActivationFront,
    pub initial_accumulator: Vec<u8>,
    pub retained_accumulator_bytes: u32,
    pub retained_item_bytes: u32,
    pub limits: PlannedActivationLimits,
    pub terminal_policy: PlannedFoldTerminalPolicy,
    pub abnormal_policy: PlannedFoldAbnormalPolicy,
    pub cancellation_policy: PlannedFoldCancellationPolicy,
    pub effect_multiplicity: PlannedActivationEffectMultiplicity,
    pub per_activation_sign_budget: SignStorageBudget,
}

pub(crate) fn verify_planned_activations(plan: &Plan, depth: u8) -> bool {
    if depth >= MAXIMUM_PLANNED_ACTIVATION_DEPTH {
        return plan.activations.is_empty();
    }
    let mut identities = alloc::collections::BTreeSet::new();
    plan.activations.iter().all(|entry| match entry {
        PlannedActivationEntry::Unary(activation) => {
            !activation.activation_id.is_empty()
                && identities.insert(activation.activation_id.clone())
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
                && activation.effect_multiplicity
                    == PlannedActivationEffectMultiplicity::OncePerAcceptedInput
                && exact_sign_budget(
                    &activation.selected_plan,
                    activation.per_activation_sign_budget,
                )
        }
        PlannedActivationEntry::Fold(activation) => {
            verify_fold(plan, activation, depth, &mut identities)
        }
        PlannedActivationEntry::Scan(activation) => {
            verify_scan(plan, activation, depth, &mut identities)
        }
    })
}

fn verify_scan(
    plan: &Plan,
    activation: &PlannedScanActivation,
    depth: u8,
    identities: &mut alloc::collections::BTreeSet<String>,
) -> bool {
    let common = PlannedFoldActivation {
        activation_id: activation.activation_id.clone(),
        owner_placement_id: activation.owner_placement_id.clone(),
        selected_plan_id: activation.selected_plan_id.clone(),
        selected_plan: activation.selected_plan.clone(),
        accumulator_input: activation.accumulator_input.clone(),
        item_input: activation.item_input.clone(),
        output: activation.output.clone(),
        initial_accumulator: activation.initial_accumulator.clone(),
        retained_accumulator_bytes: activation.retained_accumulator_bytes,
        retained_item_bytes: activation.retained_item_bytes,
        limits: activation.limits,
        terminal_policy: PlannedFoldTerminalPolicy::DrainThenEmitAccumulatorExactlyOnce,
        abnormal_policy: PlannedFoldAbnormalPolicy::DiscardAccumulatorAndPropagateExact,
        cancellation_policy: PlannedFoldCancellationPolicy::DiscardAccumulatorWithoutEmission,
        effect_multiplicity: activation.effect_multiplicity,
        per_activation_sign_budget: activation.per_activation_sign_budget,
    };
    verify_fold(plan, &common, depth, identities)
        && activation.terminal_policy
            == PlannedScanTerminalPolicy::DrainThenCloseWithoutExtraEmission
        && activation.abnormal_policy
            == PlannedScanAbnormalPolicy::DiscardAccumulatorAndPropagateExact
        && activation.cancellation_policy
            == PlannedScanCancellationPolicy::DiscardAccumulatorWithoutEmission
}

fn verify_fold(
    plan: &Plan,
    activation: &PlannedFoldActivation,
    depth: u8,
    identities: &mut alloc::collections::BTreeSet<String>,
) -> bool {
    !activation.activation_id.is_empty()
        && identities.insert(activation.activation_id.clone())
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
        && resolve_front(
            &activation.selected_plan,
            &activation.accumulator_input,
            PortDirection::Input,
        )
        .is_some()
        && resolve_front(
            &activation.selected_plan,
            &activation.item_input,
            PortDirection::Input,
        )
        .is_some()
        && resolve_front(
            &activation.selected_plan,
            &activation.output,
            PortDirection::Output,
        )
        .is_some()
        && activation.accumulator_input.front_port_id.as_str() == "accumulator"
        && activation.item_input.front_port_id.as_str() == "item"
        && activation.output.front_port_id.as_str() == "combined"
        && activation.accumulator_input.value_kind == activation.output.value_kind
        && activation.accumulator_input.abnormal_kind == activation.item_input.abnormal_kind
        && activation.accumulator_input.abnormal_kind == activation.output.abnormal_kind
        && (!activation.initial_accumulator.is_empty()
            || activation.accumulator_input.value_kind.as_str() == crate::UNIT_INFO_ID)
        && activation.initial_accumulator.len() <= activation.retained_accumulator_bytes as usize
        && activation.retained_accumulator_bytes > 0
        && activation.retained_item_bytes > 0
        && activation.limits.maximum_active == 1
        && activation.limits.maximum_queue_items == 1
        && activation.limits.maximum_items > 0
        && activation
            .retained_accumulator_bytes
            .checked_mul(2)
            .and_then(|bytes| {
                activation
                    .retained_item_bytes
                    .checked_mul(2)
                    .and_then(|items| bytes.checked_add(items))
            })
            .is_some_and(|required| activation.limits.maximum_queue_bytes >= required)
        && activation.terminal_policy
            == PlannedFoldTerminalPolicy::DrainThenEmitAccumulatorExactlyOnce
        && activation.abnormal_policy
            == PlannedFoldAbnormalPolicy::DiscardAccumulatorAndPropagateExact
        && activation.cancellation_policy
            == PlannedFoldCancellationPolicy::DiscardAccumulatorWithoutEmission
        && activation.effect_multiplicity
            == PlannedActivationEffectMultiplicity::OncePerAcceptedInput
        && exact_sign_budget(
            &activation.selected_plan,
            activation.per_activation_sign_budget,
        )
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
    activation.limits.maximum_items > 0
        && matches!((input, output), (Some(input), Some(output))
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

fn exact_sign_budget(plan: &Plan, budget: SignStorageBudget) -> bool {
    let Some((items, bytes)) =
        plan.fragments
            .iter()
            .try_fold((0u16, 0u32), |(items, bytes), fragment| {
                Some((
                    items.checked_add(fragment.sign_storage_budget.item_capacity)?,
                    bytes.checked_add(fragment.sign_storage_budget.byte_capacity)?,
                ))
            })
    else {
        return false;
    };
    budget
        == (SignStorageBudget {
            item_capacity: items,
            byte_capacity: bytes,
        })
}
