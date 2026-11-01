//! Frozen pre-view canonical root encoder, used only as a historical byte oracle.
use super::*;
use crate::*;
use alloc::{boxed::Box, vec, vec::Vec};
fn historical_plan_id(
    plot_identity: &PlotIdentity,
    realization_backs: &[PlotBack],
    activations: &[PlannedActivationEntry],
    commitments: &[FragmentCommitment],
    activation_preparations: &[PlannedActivationPreparationBinding],
) -> PlanId {
    let mut canonical = CanonicalHash::new();
    push_string(&mut canonical, plot_identity.source_document_id.as_str());
    push_string(&mut canonical, plot_identity.checked_plot_id.as_str());
    push_string(&mut canonical, plot_identity.expanded_plot_id.as_str());
    if !realization_backs.is_empty() {
        plan_realization::push_canonical(&mut canonical, realization_backs);
    }
    if !activation_preparations.is_empty() {
        push_u32(&mut canonical, activation_preparations.len() as u32);
        for binding in activation_preparations {
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
    if !activations.is_empty() {
        push_u32(&mut canonical, activations.len() as u32);
        for entry in activations {
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
    push_u32(&mut canonical, commitments.len() as u32);
    for commitment in commitments {
        push_string(&mut canonical, commitment.host_id.as_str());
        push_string(&mut canonical, commitment.fragment_id.as_str());
    }
    PlanId::from(crate::hex_digest(&canonical.finish()))
}

fn entries() -> Vec<PlannedActivationEntry> {
    let empty = || {
        Box::new(Plan {
            plan_id: "child-plan".into(),
            source_document_id: "child-source".into(),
            checked_plot_id: "child-checked".into(),
            expanded_plot_id: "child-expanded".into(),
            completion_policy: PlanCompletionPolicy::Live,
            realization_backs: vec![],
            activations: vec![],
            activation_preparations: vec![],
            fragments: vec![],
        })
    };
    let front = || PlannedActivationFront {
        front_port_id: "port\0é".into(),
        value_kind: "value".into(),
        abnormal_kind: Some("abnormal".into()),
    };
    let limits = PlannedActivationLimits {
        maximum_active: u16::MAX,
        maximum_queue_items: u16::MAX,
        maximum_queue_bytes: u32::MAX,
        maximum_items: u16::MAX,
    };
    let signs = SignStorageBudget {
        item_capacity: u16::MAX,
        byte_capacity: u32::MAX,
    };
    vec![
        PlannedActivationEntry::Unary(PlannedActivation {
            activation_id: "activation".into(),
            owner_placement_id: "owner".into(),
            selected_plan_id: "selected".into(),
            selected_plan: empty(),
            input: front(),
            output: front(),
            limits,
            terminal_policy: PlannedActivationTerminalPolicy::DrainThenPropagateExact,
            cancellation_policy:
                PlannedActivationCancellationPolicy::CancelActiveAndRejectLateCompletion,
            effect_multiplicity: PlannedActivationEffectMultiplicity::OncePerAcceptedInput,
            per_activation_sign_budget: signs,
        }),
        PlannedActivationEntry::Fold(PlannedFoldActivation {
            activation_id: "fold".into(),
            owner_placement_id: "owner".into(),
            selected_plan_id: "selected".into(),
            selected_plan: empty(),
            accumulator_input: front(),
            item_input: front(),
            output: front(),
            initial_accumulator: vec![0, 255, 1],
            retained_accumulator_bytes: u32::MAX,
            retained_item_bytes: u32::MAX,
            limits,
            terminal_policy: PlannedFoldTerminalPolicy::DrainThenEmitAccumulatorExactlyOnce,
            abnormal_policy: PlannedFoldAbnormalPolicy::DiscardAccumulatorAndPropagateExact,
            cancellation_policy: PlannedFoldCancellationPolicy::DiscardAccumulatorWithoutEmission,
            effect_multiplicity: PlannedActivationEffectMultiplicity::OncePerAcceptedInput,
            per_activation_sign_budget: signs,
        }),
        PlannedActivationEntry::Scan(PlannedScanActivation {
            activation_id: "scan".into(),
            owner_placement_id: "owner".into(),
            selected_plan_id: "selected".into(),
            selected_plan: empty(),
            accumulator_input: front(),
            item_input: front(),
            output: front(),
            initial_accumulator: vec![255, 0, 1],
            retained_accumulator_bytes: u32::MAX,
            retained_item_bytes: u32::MAX,
            limits,
            terminal_policy: PlannedScanTerminalPolicy::DrainThenCloseWithoutExtraEmission,
            abnormal_policy: PlannedScanAbnormalPolicy::DiscardAccumulatorAndPropagateExact,
            cancellation_policy: PlannedScanCancellationPolicy::DiscardAccumulatorWithoutEmission,
            effect_multiplicity: PlannedActivationEffectMultiplicity::OncePerAcceptedInput,
            per_activation_sign_budget: signs,
        }),
    ]
}
#[test]
fn all_root_optional_fields_and_activation_variants_preserve_historical_bytes() {
    let identity = PlotIdentity {
        source_document_id: "source\0é".into(),
        checked_plot_id: "checked".into(),
        expanded_plot_id: "expanded".into(),
    };
    let backs = [PlotBack {
        invocation_path: "invoke\0é".into(),
        kind_id: "kind".into(),
        kind_contract_revision: "revision".into(),
        source_document_id: "source".into(),
        checked_plot_id: "checked".into(),
    }];
    let commitments = [
        FragmentCommitment {
            host_id: "host1".into(),
            fragment_id: "fragment1".into(),
        },
        FragmentCommitment {
            host_id: "host2".into(),
            fragment_id: "fragment2".into(),
        },
    ];
    let preparations = [PlannedActivationPreparationBinding {
        activation_id: "act".into(),
        owner_placement_id: "owner".into(),
        owner_fragment_id: "fragment".into(),
        owner_host_id: "host".into(),
        owner_boot_id: "boot".into(),
        owner_offer_generation: OfferGeneration(u64::MAX),
        selected_plan_id: "selected".into(),
        child_fragments: vec![PlannedSubordinateFragmentBinding {
            fragment_id: "child-fragment".into(),
            host_id: "child-host".into(),
            boot_id: "child-boot".into(),
            offer_generation: OfferGeneration(u64::MAX),
            obligation_digest: [255; 32],
        }],
    }];
    let entries = entries();
    for mask in 0..8 {
        let backs = if mask & 1 == 0 {
            &backs[..0]
        } else {
            &backs[..]
        };
        let preparations = if mask & 2 == 0 {
            &preparations[..0]
        } else {
            &preparations[..]
        };
        for count in 0..=commitments.len() {
            for activations in [
                &entries[..0],
                &entries[..1],
                &entries[1..2],
                &entries[2..3],
                &entries[..],
            ] {
                let activations = if mask & 4 == 0 {
                    &activations[..0]
                } else {
                    activations
                };
                let borrowed: Vec<_> = commitments[..count].iter().map(Into::into).collect();
                let input = PlanFingerprintInputRef {
                    source_document_id: identity.source_document_id.as_str(),
                    checked_plot_id: identity.checked_plot_id.as_str(),
                    expanded_plot_id: identity.expanded_plot_id.as_str(),
                    realization_backs: backs,
                    activations,
                    commitments: FragmentCommitmentRefs::Borrowed(&borrowed),
                    activation_preparations: preparations,
                };
                assert_eq!(
                    compute_plan_fingerprint_ref(input),
                    historical_plan_id(
                        &identity,
                        backs,
                        activations,
                        &commitments[..count],
                        preparations
                    )
                );
            }
        }
    }
}
