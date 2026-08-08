use conduit_core::{
    verify_plan, BootId, ConnectionProvider, HostAdvertisement, HostId, OperationId,
    PlannerCapabilityOffer, PlannerLimits, PlannerProfileId, ProtectedResourceAccess,
    ProtectedResourceCommitPolicy, ProtectedResourceGrant, ResourceBindingRoleId, ResourceClassId,
    ResourceHandleId,
};
use conduit_planner::{
    default_placements, plan_with_advertised_profile, PlannerError, PlanningOptions,
    BROWSER_PLANNER_PROFILE, FULL_PLANNER_LIMITS, FULL_PLANNER_PROFILE,
};
use conduit_runtime::lowering::lower_plan_fragment;
use std::collections::BTreeMap;

fn portable_inputs() -> (conduit_form::CheckedForm, Vec<HostAdvertisement>) {
    let form = conduit_form::parse(
        include_str!("../../../examples/signal-demo.form"),
        &conduit_signal::signal_profile_catalog(),
    )
    .expect("portable Signal form checks");
    let target = conduit_signal::pico_local_advertisement();
    assert!(target.planner_capabilities.is_empty());
    (form, vec![target])
}

fn planner_host(host: &str, boot: &str, profile: &str, limits: PlannerLimits) -> HostAdvertisement {
    let mut host_advertisement = conduit_signal::pico_local_advertisement();
    host_advertisement.host_id = HostId::from(host);
    host_advertisement.boot_id = BootId::from(boot);
    host_advertisement.capabilities.clear();
    host_advertisement.resources.clear();
    host_advertisement.planner_capabilities = vec![PlannerCapabilityOffer {
        profile_id: PlannerProfileId::from(profile),
        limits,
    }];
    host_advertisement
}

fn options<'a>(
    overrides: &'a BTreeMap<(OperationId, OperationId), ConnectionProvider>,
) -> PlanningOptions<'a> {
    PlanningOptions {
        connection_providers: overrides,
        connection_item_capacity: 1,
        connection_byte_capacity: 9,
        authority_grants: &[],
        protected_resource_grants: &[],
        link_bindings: &[],
    }
}

#[test]
fn full_and_browser_profiles_make_the_same_plan_without_planner_identity() {
    let (form, realm) = portable_inputs();
    let placements = default_placements(&form, &realm).expect("target placement");
    let overrides = BTreeMap::new();
    let full = planner_host(
        "std-planner-a",
        "std-boot-a",
        FULL_PLANNER_PROFILE,
        FULL_PLANNER_LIMITS,
    );
    let browser = planner_host(
        "browser-planner-b",
        "browser-boot-b",
        BROWSER_PLANNER_PROFILE,
        PlannerLimits {
            maximum_host_advertisements: 2,
            maximum_operations: 2,
            maximum_connections: 1,
            maximum_authority_grants: 0,
            maximum_protected_resource_grants: 0,
            maximum_link_bindings: 0,
        },
    );

    let full_plan = plan_with_advertised_profile(
        &full,
        &PlannerProfileId::from(FULL_PLANNER_PROFILE),
        &form,
        &realm,
        &placements,
        &[ConnectionProvider::Local],
        options(&overrides),
    )
    .expect("full profile plans");
    let browser_plan = plan_with_advertised_profile(
        &browser,
        &PlannerProfileId::from(BROWSER_PLANNER_PROFILE),
        &form,
        &realm,
        &placements,
        &[ConnectionProvider::Local],
        options(&overrides),
    )
    .expect("browser profile plans locally");

    assert_eq!(full_plan, browser_plan);
    assert!(verify_plan(&browser_plan));
    assert!(browser_plan.fragments.iter().all(|fragment| {
        fragment.host_id == realm[0].host_id
            && fragment.boot_id == realm[0].boot_id
            && lower_plan_fragment(fragment).is_ok()
    }));
}

#[test]
fn bounded_profile_refuses_before_planning_without_delegation() {
    let (form, realm) = portable_inputs();
    let placements = default_placements(&form, &realm).expect("target placement");
    let overrides = BTreeMap::new();
    let bounded = planner_host(
        "bounded-planner",
        "bounded-boot",
        BROWSER_PLANNER_PROFILE,
        PlannerLimits {
            maximum_host_advertisements: 1,
            maximum_operations: 1,
            maximum_connections: 1,
            maximum_authority_grants: 0,
            maximum_protected_resource_grants: 0,
            maximum_link_bindings: 0,
        },
    );

    let error = plan_with_advertised_profile(
        &bounded,
        &PlannerProfileId::from(BROWSER_PLANNER_PROFILE),
        &form,
        &realm,
        &placements,
        &[ConnectionProvider::Local],
        options(&overrides),
    )
    .expect_err("two-operation form exceeds bounded offer");

    assert_eq!(
        error,
        PlannerError::PlannerLimitExceeded(
            "profile input has 2 operations, above advertised maximum 1".to_string()
        )
    );
}

#[test]
fn host_must_truthfully_advertise_the_requested_profile() {
    let (form, realm) = portable_inputs();
    let placements = default_placements(&form, &realm).expect("target placement");
    let overrides = BTreeMap::new();

    let error = plan_with_advertised_profile(
        &realm[0],
        &PlannerProfileId::from(BROWSER_PLANNER_PROFILE),
        &form,
        &realm,
        &placements,
        &[ConnectionProvider::Local],
        options(&overrides),
    )
    .expect_err("non-planner target cannot execute planner capability");

    assert!(matches!(
        error,
        PlannerError::PlannerCapabilityNotAdvertised(_)
    ));
}

#[test]
fn portable_profile_admits_protected_grants_before_planning() {
    let (form, realm) = portable_inputs();
    let placements = default_placements(&form, &realm).expect("target placement");
    let overrides = BTreeMap::new();
    let bounded = planner_host(
        "bounded-planner",
        "bounded-boot",
        BROWSER_PLANNER_PROFILE,
        PlannerLimits {
            maximum_host_advertisements: 1,
            maximum_operations: 2,
            maximum_connections: 1,
            maximum_authority_grants: 0,
            maximum_protected_resource_grants: 0,
            maximum_link_bindings: 0,
        },
    );
    let grant = ProtectedResourceGrant {
        role_id: ResourceBindingRoleId::from("source"),
        handle_id: ResourceHandleId::from("opaque/source"),
        operation_id: OperationId::from("pulse"),
        host_id: realm[0].host_id.clone(),
        boot_id: realm[0].boot_id.clone(),
        capability_id: realm[0].capabilities[0].capability_id.clone(),
        class_id: ResourceClassId::from("conduit.resource/test@1"),
        access: ProtectedResourceAccess::ReadExisting,
        maximum_bytes: 1,
        commit_policy: ProtectedResourceCommitPolicy::NotApplicable,
    };
    let mut request = options(&overrides);
    request.protected_resource_grants = core::slice::from_ref(&grant);

    assert_eq!(
        plan_with_advertised_profile(
            &bounded,
            &PlannerProfileId::from(BROWSER_PLANNER_PROFILE),
            &form,
            &realm,
            &placements,
            &[ConnectionProvider::Local],
            request,
        ),
        Err(PlannerError::PlannerLimitExceeded(
            "profile input has 1 protected resource grants, above advertised maximum 0".to_string()
        ))
    );
}
