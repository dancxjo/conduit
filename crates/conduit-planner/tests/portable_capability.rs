use conduit_core::{
    verify_plan, BootId, ConnectionBase, GearId, HostAdvertisement, HostId, PlannerCapabilityOffer,
    PlannerLimits, PlannerProfileId, ProtectedResourceAccess, ProtectedResourceCommitPolicy,
    ProtectedResourceGrant, ResourceBindingRoleId, ResourceClassId, ResourceHandleId,
};
use conduit_planner::{
    default_placements, plan_with_advertised_profile, PlannerError, PlanningOptions,
    BROWSER_PLANNER_PROFILE, FULL_PLANNER_LIMITS, FULL_PLANNER_PROFILE,
};
use conduit_runtime::lowering::lower_plan_fragment;
use std::collections::BTreeMap;

static EMPTY_ROUTE_CANDIDATES: BTreeMap<(GearId, GearId), Vec<conduit_core::LineId>> =
    BTreeMap::new();

fn portable_inputs() -> (conduit_form::CheckedForm, Vec<HostAdvertisement>) {
    let form = conduit_form::parse_with_startup(
        include_str!("../../../fixtures/forms/signal-demo.conduit"),
        &conduit_signal::signal_startup_catalog(),
        &conduit_signal::signal_profile_catalog(),
    )
    .expect("portable Signal form checks");
    let target = conduit_signal_conformance::pico_local_advertisement();
    assert!(target.planner_capabilities.is_empty());
    (form, vec![target])
}

fn planner_host(host: &str, boot: &str, profile: &str, limits: PlannerLimits) -> HostAdvertisement {
    let mut host_advertisement = conduit_signal_conformance::pico_local_advertisement();
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

fn options<'a>(overrides: &'a BTreeMap<(GearId, GearId), ConnectionBase>) -> PlanningOptions<'a> {
    PlanningOptions {
        connection_bases: overrides,
        line_candidates: &EMPTY_ROUTE_CANDIDATES,
        connection_item_capacity: 1,
        connection_byte_capacity: 9,
        authority_grants: &[],
        protected_resource_grants: &[],
        line_offers: &[],
    }
}

#[test]
fn full_and_browser_profiles_make_the_same_plan_without_planner_identity() {
    let (form, hosts) = portable_inputs();
    let placements = default_placements(&form, &hosts).expect("target placement");
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
            maximum_gears: 2,
            maximum_connections: 1,
            maximum_authority_grants: 0,
            maximum_protected_resource_grants: 0,
            maximum_line_offers: 0,
        },
    );

    let full_plan = plan_with_advertised_profile(
        &full,
        &PlannerProfileId::from(FULL_PLANNER_PROFILE),
        &form,
        &hosts,
        &placements,
        &[ConnectionBase::Local],
        options(&overrides),
    )
    .expect("full profile plans");
    let browser_plan = plan_with_advertised_profile(
        &browser,
        &PlannerProfileId::from(BROWSER_PLANNER_PROFILE),
        &form,
        &hosts,
        &placements,
        &[ConnectionBase::Local],
        options(&overrides),
    )
    .expect("browser profile plans locally");

    assert_eq!(full_plan, browser_plan);
    assert!(verify_plan(&browser_plan));
    assert!(browser_plan.fragments.iter().all(|fragment| {
        fragment.host_id == hosts[0].host_id
            && fragment.boot_id == hosts[0].boot_id
            && lower_plan_fragment(fragment).is_ok()
    }));
}

#[test]
fn bounded_profile_refuses_before_planning_without_delegation() {
    let (form, hosts) = portable_inputs();
    let placements = default_placements(&form, &hosts).expect("target placement");
    let overrides = BTreeMap::new();
    let bounded = planner_host(
        "bounded-planner",
        "bounded-boot",
        BROWSER_PLANNER_PROFILE,
        PlannerLimits {
            maximum_host_advertisements: 1,
            maximum_gears: 1,
            maximum_connections: 1,
            maximum_authority_grants: 0,
            maximum_protected_resource_grants: 0,
            maximum_line_offers: 0,
        },
    );

    let error = plan_with_advertised_profile(
        &bounded,
        &PlannerProfileId::from(BROWSER_PLANNER_PROFILE),
        &form,
        &hosts,
        &placements,
        &[ConnectionBase::Local],
        options(&overrides),
    )
    .expect_err("two-gear form exceeds bounded offer");

    assert_eq!(
        error,
        PlannerError::PlannerLimitExceeded(
            "profile input has 2 gears, above advertised maximum 1".to_string()
        )
    );
}

#[test]
fn host_must_truthfully_advertise_the_requested_profile() {
    let (form, hosts) = portable_inputs();
    let placements = default_placements(&form, &hosts).expect("target placement");
    let overrides = BTreeMap::new();

    let error = plan_with_advertised_profile(
        &hosts[0],
        &PlannerProfileId::from(BROWSER_PLANNER_PROFILE),
        &form,
        &hosts,
        &placements,
        &[ConnectionBase::Local],
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
    let (form, hosts) = portable_inputs();
    let placements = default_placements(&form, &hosts).expect("target placement");
    let overrides = BTreeMap::new();
    let bounded = planner_host(
        "bounded-planner",
        "bounded-boot",
        BROWSER_PLANNER_PROFILE,
        PlannerLimits {
            maximum_host_advertisements: 1,
            maximum_gears: 2,
            maximum_connections: 1,
            maximum_authority_grants: 0,
            maximum_protected_resource_grants: 0,
            maximum_line_offers: 0,
        },
    );
    let grant = ProtectedResourceGrant {
        role_id: ResourceBindingRoleId::from("source"),
        handle_id: ResourceHandleId::from("opaque/source"),
        gear_id: GearId::from("pulse"),
        host_id: hosts[0].host_id.clone(),
        boot_id: hosts[0].boot_id.clone(),
        capability_id: hosts[0].capabilities[0].capability_id.clone(),
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
            &hosts,
            &placements,
            &[ConnectionBase::Local],
            request,
        ),
        Err(PlannerError::PlannerLimitExceeded(
            "profile input has 1 protected resource grants, above advertised maximum 0".to_string()
        ))
    );
}
