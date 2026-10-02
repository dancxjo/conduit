use super::{host, plot};
use crate::{
    default_placements, plan_with_options, PlacementChoices, PlannerError, PlanningOptions,
};
use conduit_core::{
    seal_plan_with_realization_backs_and_completion, verify_plan, BaseImplementationId,
    HostAdvertisement, HostId, PlotIdentity, ProtectedResourceAccess,
    ProtectedResourceCommitPolicy, ProtectedResourceGrant, ResourceBindingRoleId, ResourceHandleId,
};
use std::collections::BTreeMap;

fn protected_grant(handle: &str) -> ProtectedResourceGrant {
    ProtectedResourceGrant {
        role_id: ResourceBindingRoleId::from("source"),
        handle_id: ResourceHandleId::from(handle),
        gear_id: conduit_core::GearId::from("signal-demo/pulse"),
        host_id: HostId::from("std-host-1"),
        boot_id: conduit_core::BootId::from("boot-1"),
        capability_id: conduit_core::CapabilityId::from("pulse-1"),
        class_id: conduit_core::ResourceClassId::from(conduit_core::TIMER_RESOURCE_CLASS),
        access: ProtectedResourceAccess::ReadExisting,
        maximum_bytes: 1024,
        commit_policy: ProtectedResourceCommitPolicy::NotApplicable,
    }
}

fn plan_with_protected_test_grants(
    plot: &conduit_plot::CheckedPlot,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
    grants: &[ProtectedResourceGrant],
) -> Result<conduit_core::Plan, PlannerError> {
    let base_overrides = BTreeMap::new();
    plan_with_options(
        plot,
        hosts,
        placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        PlanningOptions {
            connection_bases: &base_overrides,
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 4,
            connection_byte_capacity: 64,
            authority_grants: &[],
            protected_resource_grants: grants,
            line_offers: &[],
        },
    )
}

#[test]
fn choices_are_exact_boot_scoped_plan_bindings() {
    let plot = plot();
    let mut target = host();
    target.capabilities[0].resource_requirements[0].protected_role =
        Some(ResourceBindingRoleId::from("source"));
    let hosts = vec![target];
    let placements = default_placements(&plot, &hosts).expect("placements resolve");

    assert!(matches!(
        plan_with_protected_test_grants(&plot, &hosts, &placements, &[]),
        Err(PlannerError::ProtectedResourceGrantMissing(_))
    ));

    let mut stale = protected_grant("handle/source");
    stale.boot_id = conduit_core::BootId::from("stale-boot");
    assert!(matches!(
        plan_with_protected_test_grants(&plot, &hosts, &placements, &[stale]),
        Err(PlannerError::ProtectedResourceGrantMissing(_))
    ));

    let grant = protected_grant("handle/source");
    let plan =
        plan_with_protected_test_grants(&plot, &hosts, &placements, core::slice::from_ref(&grant))
            .expect("exact protected grant plans");
    assert!(verify_plan(&plan));
    let binding = plan.fragments[0].placements[0].resources[0]
        .protected
        .as_ref()
        .expect("protected binding is sealed");
    assert_eq!(binding.role_id.as_str(), "source");
    assert_eq!(binding.handle_id.as_str(), "handle/source");
    assert_eq!(binding.maximum_bytes, 1024);
    assert_eq!(binding.access, ProtectedResourceAccess::ReadExisting);

    let different_handle = protected_grant("handle/other-source");
    let changed = plan_with_protected_test_grants(
        &plot,
        &hosts,
        &placements,
        core::slice::from_ref(&different_handle),
    )
    .expect("other exact handle plans");
    assert_ne!(plan.plan_id, changed.plan_id);

    let mut changed_contract = plan.clone();
    changed_contract.fragments[0].placements[0]
        .semantic_contract
        .laws
        .push(conduit_core::KindSemanticLaw::ExternalEffects(
            conduit_core::ExternalEffectBehavior::Observable,
        ));
    assert!(!verify_plan(&changed_contract));
    let resealed_contract = seal_plan_with_realization_backs_and_completion(
        PlotIdentity {
            source_document_id: plan.source_document_id.clone(),
            checked_plot_id: plan.checked_plot_id.clone(),
            expanded_plot_id: plan.expanded_plot_id.clone(),
        },
        plan.completion_policy,
        plan.realization_backs.clone(),
        changed_contract.fragments,
    );
    assert!(verify_plan(&resealed_contract));
    assert_ne!(plan.plan_id, resealed_contract.plan_id);

    let mut mutated = plan;
    mutated.fragments[0].placements[0].resources[0]
        .protected
        .as_mut()
        .expect("protected binding exists")
        .handle_id = ResourceHandleId::from("mutated/after-seal");
    assert!(!verify_plan(&mutated));
}

#[test]
fn grants_reject_incoherent_policy_and_handle_reuse() {
    let plot = plot();
    let mut target = host();
    target.capabilities[0].resource_requirements[0].protected_role =
        Some(ResourceBindingRoleId::from("source"));
    let hosts = vec![target];
    let placements = default_placements(&plot, &hosts).expect("placements resolve");

    let mut incoherent = protected_grant("handle/source");
    incoherent.commit_policy = ProtectedResourceCommitPolicy::ReplaceExisting;
    assert!(matches!(
        plan_with_protected_test_grants(&plot, &hosts, &placements, &[incoherent]),
        Err(PlannerError::InvalidProtectedResourceGrant(_))
    ));

    let first = protected_grant("handle/reused");
    let mut second = first.clone();
    second.role_id = ResourceBindingRoleId::from("destination");
    assert!(matches!(
        plan_with_protected_test_grants(&plot, &hosts, &placements, &[first, second]),
        Err(PlannerError::InvalidProtectedResourceGrant(_))
    ));
}
