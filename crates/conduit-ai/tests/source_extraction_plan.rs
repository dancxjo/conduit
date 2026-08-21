#![cfg(feature = "form-catalog")]

use std::collections::BTreeMap;

use conduit_ai::{
    deterministic_source_extraction_offer, install_source_extraction_catalog,
    MAXIMUM_EXTRACTION_WORK_UNITS, SOURCE_EXTRACTION_OPERATION, SOURCE_READER_RESOURCE_CLASS,
    SOURCE_READER_RESOURCE_ROLE, SOURCE_READ_AUTHORITY,
};
use conduit_core::{
    authority_grant, bind_active_play, bind_sign, resource_offer, verify_plan, BootId,
    ConnectionBase, HostAdvertisement, HostId, HostProfileId, OfferGeneration,
    ProtectedResourceAccess, ProtectedResourceCommitPolicy, ProtectedResourceGrant,
    ResourceBindingRoleId, ResourceClassId, ResourceHandleId, PROTOCOL_VERSION,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};
use conduit_planner::{
    default_expanded_placements, plan_expanded_canonical_with_options, PlannerError,
    PlanningOptions,
};

const FORM: &str = "form chunk {\n extract: retrieval/extract-source(\"text-utf8\", 4096, 32, 8192, 512, 16, 16384)\n}\n";

fn expanded() -> conduit_form::ExpandedCanonicalForm {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_source_extraction_catalog(&mut startup, &mut profile).unwrap();
    let checked = check_syntax_document(&parse_syntax_document(FORM), &startup).unwrap();
    expand_canonical_form(&checked, "chunk", &profile).unwrap()
}

fn host() -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("host/extraction"),
        boot_id: BootId::from("boot/extraction/1"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("host/extraction@1"),
        resources: vec![resource_offer(
            "pool/source-reader",
            SOURCE_READER_RESOURCE_CLASS,
            MAXIMUM_EXTRACTION_WORK_UNITS,
        )],
        capabilities: vec![deterministic_source_extraction_offer("pid-7").unwrap()],
        planner_capabilities: vec![],
    }
}

fn resource_grant(
    expanded: &conduit_form::ExpandedCanonicalForm,
    host: &HostAdvertisement,
) -> ProtectedResourceGrant {
    ProtectedResourceGrant {
        role_id: ResourceBindingRoleId::from(SOURCE_READER_RESOURCE_ROLE),
        handle_id: ResourceHandleId::from("handle/source-reader/7"),
        gear_id: expanded.gears[0].gear_id.clone(),
        host_id: host.host_id.clone(),
        boot_id: host.boot_id.clone(),
        capability_id: host.capabilities[0].capability_id.clone(),
        class_id: ResourceClassId::from(SOURCE_READER_RESOURCE_CLASS),
        access: ProtectedResourceAccess::ReadExisting,
        maximum_bytes: u64::from(MAXIMUM_EXTRACTION_WORK_UNITS),
        commit_policy: ProtectedResourceCommitPolicy::NotApplicable,
    }
}

fn plan(
    expanded: &conduit_form::ExpandedCanonicalForm,
    host: &HostAdvertisement,
    authority: &[conduit_core::AuthorityGrant],
    resources: &[ProtectedResourceGrant],
) -> Result<conduit_core::Plan, PlannerError> {
    let placements = default_expanded_placements(expanded, core::slice::from_ref(host)).unwrap();
    plan_expanded_canonical_with_options(
        expanded,
        core::slice::from_ref(host),
        &placements,
        &[ConnectionBase::Local],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 1,
            authority_grants: authority,
            protected_resource_grants: resources,
            line_offers: &[],
        },
    )
}

#[test]
fn ordinary_form_seals_exact_operation_authority_resource_and_bounds() {
    let expanded = expanded();
    let host = host();
    let offer = &host.capabilities[0];
    let authority = authority_grant(
        "grant/source-read/7",
        &offer.authority_requirements[0],
        host.host_id.clone(),
        host.boot_id.clone(),
        offer.capability_id.clone(),
    );
    let resource = resource_grant(&expanded, &host);
    let planned = plan(&expanded, &host, &[authority], &[resource]).unwrap();
    assert!(verify_plan(&planned));

    let gear = &planned.fragments[0].placements[0];
    assert_eq!(gear.gear_id, expanded.gears[0].gear_id);
    assert_eq!(
        gear.host_operations[0].contract_id.as_str(),
        SOURCE_EXTRACTION_OPERATION
    );
    assert_eq!(
        gear.authority[0].contract_id.as_str(),
        SOURCE_READ_AUTHORITY
    );
    let protected = gear.resources[0].protected.as_ref().unwrap();
    assert_eq!(protected.role_id.as_str(), SOURCE_READER_RESOURCE_ROLE);
    assert_eq!(protected.handle_id.as_str(), "handle/source-reader/7");
    assert_eq!(protected.access, ProtectedResourceAccess::ReadExisting);
    assert_eq!(gear.configuration.len(), 7);

    let play = bind_active_play(
        &planned.plan_id,
        &planned.fragments[0].host_id,
        &planned.fragments[0].boot_id,
        1,
    );
    let sign = bind_sign(&play.host_id, &play.boot_id, Some(&play.active_play_id), 1);
    assert_eq!(play.plan_id, planned.plan_id);
    assert_eq!(sign.active_play_id, Some(play.active_play_id));

    let debug = format!("{planned:?}");
    for forbidden in ["file://", "https://", "/home/", "credential", "provider"] {
        assert!(!debug.contains(forbidden));
    }
}

#[test]
fn planning_refuses_missing_or_stale_authority_and_resource_grants() {
    let expanded = expanded();
    let host = host();
    let offer = &host.capabilities[0];
    let authority = authority_grant(
        "grant/source-read/7",
        &offer.authority_requirements[0],
        host.host_id.clone(),
        host.boot_id.clone(),
        offer.capability_id.clone(),
    );
    let resource = resource_grant(&expanded, &host);

    assert!(matches!(
        plan(&expanded, &host, &[], core::slice::from_ref(&resource)),
        Err(PlannerError::AuthorityGrantMissing(_))
    ));
    assert!(matches!(
        plan(&expanded, &host, core::slice::from_ref(&authority), &[]),
        Err(PlannerError::ProtectedResourceGrantMissing(_))
    ));

    let mut stale_authority = authority.clone();
    stale_authority.boot_id = BootId::from("boot/stale");
    assert!(matches!(
        plan(
            &expanded,
            &host,
            &[stale_authority],
            core::slice::from_ref(&resource),
        ),
        Err(PlannerError::AuthorityGrantMissing(_))
    ));

    let mut stale_resource = resource;
    stale_resource.boot_id = BootId::from("boot/stale");
    assert!(matches!(
        plan(
            &expanded,
            &host,
            core::slice::from_ref(&authority),
            &[stale_resource],
        ),
        Err(PlannerError::ProtectedResourceGrantMissing(_))
    ));

    let fresh_resource = resource_grant(&expanded, &host);
    let exact = plan(
        &expanded,
        &host,
        core::slice::from_ref(&authority),
        core::slice::from_ref(&fresh_resource),
    )
    .unwrap();
    let mut another_resource = fresh_resource;
    another_resource.handle_id = ResourceHandleId::from("handle/source-reader/8");
    let changed = plan(
        &expanded,
        &host,
        core::slice::from_ref(&authority),
        &[another_resource],
    )
    .unwrap();
    assert_ne!(exact.plan_id, changed.plan_id);

    let mut tampered = exact;
    tampered.fragments[0].placements[0].configuration[0].value =
        conduit_core::ConfigurationValue::Text("structured-items".into());
    assert!(!verify_plan(&tampered));
}
