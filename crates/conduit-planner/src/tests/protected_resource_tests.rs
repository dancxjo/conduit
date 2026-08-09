use super::{form, host};
use crate::{
    default_placements, plan_with_options, PlacementChoices, PlannerError, PlanningOptions,
};
use conduit_core::{
    verify_plan, ConnectionBase, HostAdvertisement, HostId, ProtectedResourceAccess,
    ProtectedResourceCommitPolicy, ProtectedResourceGrant, ResourceBindingRoleId, ResourceHandleId,
};
use std::collections::BTreeMap;

fn protected_grant(handle: &str) -> ProtectedResourceGrant {
    ProtectedResourceGrant {
        role_id: ResourceBindingRoleId::from("source"),
        handle_id: ResourceHandleId::from(handle),
        gear_id: conduit_core::GearId::from("pulse"),
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
    form: &conduit_form::CheckedForm,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
    grants: &[ProtectedResourceGrant],
) -> Result<conduit_core::Plan, PlannerError> {
    let base_overrides = BTreeMap::new();
    plan_with_options(
        form,
        hosts,
        placements,
        &[ConnectionBase::Local],
        PlanningOptions {
            connection_bases: &base_overrides,
            route_candidates: &BTreeMap::new(),
            connection_item_capacity: 4,
            connection_byte_capacity: 64,
            authority_grants: &[],
            protected_resource_grants: grants,
            link_bindings: &[],
        },
    )
}

#[test]
fn choices_are_exact_boot_scoped_plan_bindings() {
    let form = form();
    let mut target = host();
    target.capabilities[0].resource_requirements[0].protected_role =
        Some(ResourceBindingRoleId::from("source"));
    let hosts = vec![target];
    let placements = default_placements(&form, &hosts).expect("placements resolve");

    assert!(matches!(
        plan_with_protected_test_grants(&form, &hosts, &placements, &[]),
        Err(PlannerError::ProtectedResourceGrantMissing(_))
    ));

    let mut stale = protected_grant("handle/source");
    stale.boot_id = conduit_core::BootId::from("stale-boot");
    assert!(matches!(
        plan_with_protected_test_grants(&form, &hosts, &placements, &[stale]),
        Err(PlannerError::ProtectedResourceGrantMissing(_))
    ));

    let grant = protected_grant("handle/source");
    let plan =
        plan_with_protected_test_grants(&form, &hosts, &placements, core::slice::from_ref(&grant))
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
        &form,
        &hosts,
        &placements,
        core::slice::from_ref(&different_handle),
    )
    .expect("other exact handle plans");
    assert_ne!(plan.plan_id, changed.plan_id);

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
    let form = form();
    let mut target = host();
    target.capabilities[0].resource_requirements[0].protected_role =
        Some(ResourceBindingRoleId::from("source"));
    let hosts = vec![target];
    let placements = default_placements(&form, &hosts).expect("placements resolve");

    let mut incoherent = protected_grant("handle/source");
    incoherent.commit_policy = ProtectedResourceCommitPolicy::ReplaceExisting;
    assert!(matches!(
        plan_with_protected_test_grants(&form, &hosts, &placements, &[incoherent]),
        Err(PlannerError::InvalidProtectedResourceGrant(_))
    ));

    let first = protected_grant("handle/reused");
    let mut second = first.clone();
    second.role_id = ResourceBindingRoleId::from("destination");
    assert!(matches!(
        plan_with_protected_test_grants(&form, &hosts, &placements, &[first, second]),
        Err(PlannerError::InvalidProtectedResourceGrant(_))
    ));
}
