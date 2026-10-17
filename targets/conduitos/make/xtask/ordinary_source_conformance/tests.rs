use super::*;
use conduit_core::{seal_plan_with_completion, ConfigurationValue, PlanId, PlotIdentity};

fn reseal(plan: Plan) -> Plan {
    seal_plan_with_completion(
        PlotIdentity {
            source_document_id: plan.source_document_id,
            checked_plot_id: plan.checked_plot_id,
            expanded_plot_id: plan.expanded_plot_id,
        },
        plan.completion_policy,
        plan.fragments,
    )
}
fn protected(mut plan: Plan) -> Plan {
    let region = &mut plan.fragments[0].execution_regions[0];
    region.execution_profile_id = conduitos::ordinary_plan::PROTECTED_REGION_PROFILE.into();
    region.isolation_required = true;
    region.preemption_required = true;
    reseal(plan)
}

#[test]
fn exact_source_meaning_survives_host_boot_build_and_placement_changes() {
    let original = protected(reference_plan().unwrap());
    let identities = conduitos::identity::BootIdentities {
        host: [3; 32],
        boot: [4; 32],
    };
    let offer = conduitos::offer::HostOffer::new(
        &identities,
        "another-build",
        conduitos::offer::CpuFeatures {
            sse2: true,
            rdrand: true,
            invariant_tsc: true,
        },
        1024 * 1024,
    );
    let rebound = protected(
        conduitos::dual_region_plan::prepare(&identities, &offer, "another-build")
            .unwrap()
            .plan,
    );
    assert_ne!(original.plan_id, rebound.plan_id);
    assert_ne!(
        original.fragments[0].placements[0].placement_id,
        rebound.fragments[0].placements[0].placement_id
    );
    assert_eq!(
        semantic_shape(&original).unwrap(),
        semantic_shape(&rebound).unwrap()
    );
    assert_eq!(
        capture(&original).unwrap()["semantic_shape_sha256"],
        capture(&rebound).unwrap()["semantic_shape_sha256"]
    );
}

#[test]
fn changed_sealed_configuration_or_cancellation_is_rejected_even_with_original_source_id() {
    let original = protected(reference_plan().unwrap());
    let mut changed = original.clone();
    let literal = changed.fragments[0]
        .placements
        .iter_mut()
        .find(|placement| placement.kind_id.as_str() == "text/literal")
        .unwrap();
    literal
        .configuration
        .iter_mut()
        .find(|entry| entry.key == "value")
        .unwrap()
        .value = ConfigurationValue::Text("Different literal".into());
    let changed = reseal(changed);
    assert!(conduit_core::verify_plan(&changed));
    assert_ne!(original.plan_id, changed.plan_id);
    assert!(capture(&changed).is_err());
    let mut changed = original.clone();
    changed.fragments[0].cancellation_policy = conduit_core::CancellationPolicy::DrainBeforeCancel;
    let changed = reseal(changed);
    assert!(conduit_core::verify_plan(&changed));
    assert!(capture(&changed).is_err());
}

#[test]
fn startup_ties_may_change_but_omissions_and_dependency_violations_refuse() {
    let original = protected(reference_plan().unwrap());
    let mut changed = original.clone();
    let fragment = &mut changed.fragments[0];
    let mut remaining = fragment
        .placements
        .iter()
        .map(|placement| placement.placement_id.clone())
        .collect::<BTreeSet<_>>();
    let mut chosen = Vec::new();
    while !remaining.is_empty() {
        let next = remaining
            .iter()
            .rev()
            .find(|id| {
                fragment.startup_dependencies.iter().all(|dependency| {
                    &dependency.dependent_placement_id != *id
                        || chosen.contains(&dependency.prerequisite_placement_id)
                })
            })
            .unwrap()
            .clone();
        remaining.remove(&next);
        chosen.push(next);
    }
    fragment.startup_order = chosen;
    let changed = reseal(changed);
    assert!(conduit_core::verify_plan(&changed));
    assert_eq!(
        semantic_shape(&original).unwrap(),
        semantic_shape(&changed).unwrap()
    );
    let mut omitted = original.clone();
    omitted.fragments[0].startup_order.pop();
    assert!(semantic_shape(&reseal(omitted)).is_err());
    let mut reversed = original.clone();
    reversed.fragments[0].startup_order.reverse();
    assert!(semantic_shape(&reseal(reversed)).is_err());
}

#[test]
fn unsealed_extension_and_protection_downgrade_cannot_be_conformance() {
    let original = protected(reference_plan().unwrap());
    let mut unsealed = original.clone();
    unsealed.plan_id = PlanId::from("unsealed");
    assert!(capture(&unsealed).is_err());
    let mut extension = original.clone();
    extension.fragments.push(extension.fragments[0].clone());
    assert!(capture(&reseal(extension)).is_err());
    for field in ["preemption_required", "isolation_required"] {
        let mut value = serde_json::to_value(&original).unwrap();
        value["fragments"][0]["execution_regions"][0][field] = false.into();
        let changed = reseal(serde_json::from_value(value).unwrap());
        assert!(conduit_core::verify_plan(&changed));
        assert!(capture(&changed).is_err());
    }
}
