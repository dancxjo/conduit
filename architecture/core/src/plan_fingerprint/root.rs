//! Complete borrowed root fingerprint input; descriptions convey no authority.
use super::*;
#[derive(Clone, Copy)]
pub struct FragmentCommitmentRef<'a> {
    pub host_id: &'a str,
    pub fragment_id: &'a str,
}
impl<'a> From<&'a FragmentCommitment> for FragmentCommitmentRef<'a> {
    fn from(v: &'a FragmentCommitment) -> Self {
        Self {
            host_id: v.host_id.as_str(),
            fragment_id: v.fragment_id.as_str(),
        }
    }
}
#[derive(Clone, Copy)]
pub enum FragmentCommitmentRefs<'a> {
    Single(FragmentCommitmentRef<'a>),
    Owned(&'a [FragmentCommitment]),
    Borrowed(&'a [FragmentCommitmentRef<'a>]),
}
impl<'a> FragmentCommitmentRefs<'a> {
    pub fn len(self) -> usize {
        match self {
            Self::Single(_) => 1,
            Self::Owned(v) => v.len(),
            Self::Borrowed(v) => v.len(),
        }
    }
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }
    pub fn iter(self) -> impl ExactSizeIterator<Item = FragmentCommitmentRef<'a>> {
        (0..self.len()).map(move |i| match self {
            Self::Single(v) => v,
            Self::Owned(v) => (&v[i]).into(),
            Self::Borrowed(v) => v[i],
        })
    }
}
#[derive(Clone, Copy)]
pub struct PlanFingerprintInputRef<'a> {
    pub source_document_id: &'a str,
    pub checked_plot_id: &'a str,
    pub expanded_plot_id: &'a str,
    pub realization_backs: &'a [PlotBack],
    pub activations: &'a [PlannedActivationEntry],
    pub commitments: FragmentCommitmentRefs<'a>,
    pub activation_preparations: &'a [PlannedActivationPreparationBinding],
}
/// Same canonical bytes as the rich root encoder. No verification or admission.
pub fn compute_plan_fingerprint_ref(input: PlanFingerprintInputRef<'_>) -> PlanId {
    PlanId::from(crate::hex_digest(&digest(input)))
}
/// Same canonical identity with explicit output-storage refusal.
pub fn try_compute_plan_fingerprint_ref(
    input: PlanFingerprintInputRef<'_>,
) -> Result<PlanId, alloc::collections::TryReserveError> {
    crate::try_hex_digest(&digest(input)).map(PlanId::from)
}
pub(crate) fn digest(input: PlanFingerprintInputRef<'_>) -> [u8; 32] {
    let mut canonical = CanonicalHash::new();
    push_string(&mut canonical, input.source_document_id);
    push_string(&mut canonical, input.checked_plot_id);
    push_string(&mut canonical, input.expanded_plot_id);
    if !input.realization_backs.is_empty() {
        plan_realization::push_canonical(&mut canonical, input.realization_backs);
    }
    if !input.activation_preparations.is_empty() {
        push_u32(&mut canonical, input.activation_preparations.len() as u32);
        for binding in input.activation_preparations {
            push_string(&mut canonical, &binding.activation_id);
            push_string(&mut canonical, binding.owner_placement_id.as_str());
            push_string(&mut canonical, binding.owner_fragment_id.as_str());
            push_string(&mut canonical, binding.owner_host_id.as_str());
            push_string(&mut canonical, binding.owner_boot_id.as_str());
            canonical.extend_from_slice(&binding.owner_offer_generation.0.to_le_bytes());
            push_string(&mut canonical, binding.selected_plan_id.as_str());
            push_u32(&mut canonical, binding.child_fragments.len() as u32);
            for child in &binding.child_fragments {
                push_string(&mut canonical, child.fragment_id.as_str());
                push_string(&mut canonical, child.host_id.as_str());
                push_string(&mut canonical, child.boot_id.as_str());
                canonical.extend_from_slice(&child.offer_generation.0.to_le_bytes());
                canonical.extend_from_slice(&child.obligation_digest);
            }
        }
    }
    if !input.activations.is_empty() {
        push_u32(&mut canonical, input.activations.len() as u32);
        for entry in input.activations {
            match entry {
                PlannedActivationEntry::Unary(activation) => {
                    push_string(&mut canonical, "planned-activation@1");
                    push_string(&mut canonical, &activation.activation_id);
                    push_string(&mut canonical, activation.owner_placement_id.as_str());
                    push_string(&mut canonical, activation.selected_plan_id.as_str());
                    push_activation_front(&mut canonical, &activation.input);
                    push_activation_front(&mut canonical, &activation.output);
                    canonical.extend_from_slice(&activation.limits.maximum_active.to_le_bytes());
                    canonical
                        .extend_from_slice(&activation.limits.maximum_queue_items.to_le_bytes());
                    push_u32(&mut canonical, activation.limits.maximum_queue_bytes);
                    canonical.extend_from_slice(&activation.limits.maximum_items.to_le_bytes());
                    canonical.push(activation.terminal_policy as u8);
                    canonical.push(activation.cancellation_policy as u8);
                    canonical.push(activation.effect_multiplicity as u8);
                    canonical.extend_from_slice(
                        &activation
                            .per_activation_sign_budget
                            .item_capacity
                            .to_le_bytes(),
                    );
                    push_u32(
                        &mut canonical,
                        activation.per_activation_sign_budget.byte_capacity,
                    );
                }
                PlannedActivationEntry::Fold(activation) => {
                    push_string(&mut canonical, "planned-fold-activation@1");
                    push_string(&mut canonical, &activation.activation_id);
                    push_string(&mut canonical, activation.owner_placement_id.as_str());
                    push_string(&mut canonical, activation.selected_plan_id.as_str());
                    push_activation_front(&mut canonical, &activation.accumulator_input);
                    push_activation_front(&mut canonical, &activation.item_input);
                    push_activation_front(&mut canonical, &activation.output);
                    push_u32(&mut canonical, activation.initial_accumulator.len() as u32);
                    canonical.extend_from_slice(&activation.initial_accumulator);
                    push_u32(&mut canonical, activation.retained_accumulator_bytes);
                    push_u32(&mut canonical, activation.retained_item_bytes);
                    canonical.extend_from_slice(&activation.limits.maximum_active.to_le_bytes());
                    canonical
                        .extend_from_slice(&activation.limits.maximum_queue_items.to_le_bytes());
                    push_u32(&mut canonical, activation.limits.maximum_queue_bytes);
                    canonical.extend_from_slice(&activation.limits.maximum_items.to_le_bytes());
                    canonical.push(activation.terminal_policy as u8);
                    canonical.push(activation.abnormal_policy as u8);
                    canonical.push(activation.cancellation_policy as u8);
                    canonical.push(activation.effect_multiplicity as u8);
                    canonical.extend_from_slice(
                        &activation
                            .per_activation_sign_budget
                            .item_capacity
                            .to_le_bytes(),
                    );
                    push_u32(
                        &mut canonical,
                        activation.per_activation_sign_budget.byte_capacity,
                    );
                }
                PlannedActivationEntry::Scan(activation) => {
                    push_string(&mut canonical, "planned-scan-activation@1");
                    push_string(&mut canonical, &activation.activation_id);
                    push_string(&mut canonical, activation.owner_placement_id.as_str());
                    push_string(&mut canonical, activation.selected_plan_id.as_str());
                    push_activation_front(&mut canonical, &activation.accumulator_input);
                    push_activation_front(&mut canonical, &activation.item_input);
                    push_activation_front(&mut canonical, &activation.output);
                    push_u32(&mut canonical, activation.initial_accumulator.len() as u32);
                    canonical.extend_from_slice(&activation.initial_accumulator);
                    push_u32(&mut canonical, activation.retained_accumulator_bytes);
                    push_u32(&mut canonical, activation.retained_item_bytes);
                    canonical.extend_from_slice(&activation.limits.maximum_active.to_le_bytes());
                    canonical
                        .extend_from_slice(&activation.limits.maximum_queue_items.to_le_bytes());
                    push_u32(&mut canonical, activation.limits.maximum_queue_bytes);
                    canonical.extend_from_slice(&activation.limits.maximum_items.to_le_bytes());
                    canonical.push(activation.terminal_policy as u8);
                    canonical.push(activation.abnormal_policy as u8);
                    canonical.push(activation.cancellation_policy as u8);
                    canonical.push(activation.effect_multiplicity as u8);
                    canonical.extend_from_slice(
                        &activation
                            .per_activation_sign_budget
                            .item_capacity
                            .to_le_bytes(),
                    );
                    push_u32(
                        &mut canonical,
                        activation.per_activation_sign_budget.byte_capacity,
                    );
                }
            }
        }
    }
    push_u32(&mut canonical, input.commitments.len() as u32);
    for commitment in input.commitments.iter() {
        push_string(&mut canonical, commitment.host_id);
        push_string(&mut canonical, commitment.fragment_id);
    }
    canonical.finish()
}

#[cfg(test)]
#[path = "root_tests.rs"]
mod tests;
