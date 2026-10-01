use conduit_core::{
    authority_grant, kind_id, resource_offer, BaseImplementationId, BootId, HostAdvertisement,
    HostBaseId, HostId, HostProfileId, OfferGeneration, PlannedGear, ResourceAccessMode,
    ResourceContentOffer, ResourceContentRequirement, ResourceHandleId,
    ResourceReferenceAvailability, ResourceReferenceBinding, ResourceRetention, ResourceSharing,
    DEFAULT_CONNECTION_BYTE_CAPACITY, DEFAULT_CONNECTION_ITEM_CAPACITY, PROTOCOL_VERSION,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use conduit_semantic_catalog::{
    install_job_catalogs, JobRequest, JOB_EXECUTABLE_AUTHORITY, JOB_RUN_KIND,
};
use conduit_std_host::hosted_job::TrustedJobProvider;
use std::collections::BTreeMap;
use std::path::PathBuf;

const SOURCE: &str = include_str!("../../../../forms/bounded-job/main.conduit");

pub fn planned_job(request: &JobRequest) -> PlannedGear {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_job_catalogs(&mut startup, &mut profile).unwrap();
    let syntax = parse_syntax_document(SOURCE);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let authored = expand_canonical_form_for_authoring(&checked, "bounded-job", &profile).unwrap();
    let host_id = HostId::from("host/job-proof");
    let boot_id = BootId::from("boot/job-proof");
    let executable_contract = ResourceContentRequirement {
        identity: request.executable().get().identity,
        version: request.executable().get().lifetime.version,
        content_profile: request.executable().get().content_profile.clone(),
        maximum_bytes: request.executable().get().extent.bytes as u32,
        maximum_items: request.executable().get().extent.items.unwrap_or(1) as u32,
        retention: ResourceRetention::Boot,
        sharing: ResourceSharing::ImmutableReadMany,
        access: ResourceAccessMode::ReadPublished,
        generation_slots: 1,
        reader_leases: 1,
        publication_slots: 0,
        sensitive: false,
    };
    let offers = conduit_std_offers::job_std_offers(executable_contract.clone());
    let run_offer = offers
        .iter()
        .find(|offer| offer.kind_id.as_str() == JOB_RUN_KIND)
        .unwrap();
    let authority_requirement = run_offer.authority_requirements[0].clone();
    let run_capability_id = run_offer.capability_id.clone();
    let mut executable_resource = resource_offer(
        "pool/job-executable",
        conduit_semantic_catalog::JOB_EXECUTABLE_ACCESS_CLASS,
        1,
    );
    executable_resource.content = Some(ResourceContentOffer {
        contract: executable_contract,
        owner_host: host_id.clone(),
        owner_boot: boot_id.clone(),
        base_id: HostBaseId::from("base/job-proof"),
        residence_profile: kind_id("std/process-executable-residence@1"),
    });
    let host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id,
        boot_id,
        offer_generation: OfferGeneration(7),
        profile: HostProfileId::from("std/job-proof@1"),
        bases: vec![],
        resources: vec![executable_resource],
        planner_capabilities: vec![],
        capabilities: offers,
    };
    let grant = authority_grant(
        "grant/job-execute",
        &authority_requirement,
        host.host_id.clone(),
        host.boot_id.clone(),
        run_capability_id,
    );
    let placements = conduit_planner::default_expanded_placements(
        &authored.expanded,
        core::slice::from_ref(&host),
    )
    .unwrap();
    let plan = conduit_planner::plan_expanded_canonical_with_options(
        &authored.expanded,
        core::slice::from_ref(&host),
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: DEFAULT_CONNECTION_ITEM_CAPACITY,
            connection_byte_capacity: DEFAULT_CONNECTION_BYTE_CAPACITY,
            authority_grants: &[grant],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .unwrap();
    plan.fragments[0]
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == JOB_RUN_KIND)
        .unwrap()
        .clone()
}

pub fn provider(request: &JobRequest, placement: &PlannedGear, path: &str) -> TrustedJobProvider {
    provider_with_availability(
        request,
        placement,
        path,
        ResourceReferenceAvailability::Available,
    )
}

pub fn provider_with_availability(
    request: &JobRequest,
    placement: &PlannedGear,
    path: &str,
    availability: ResourceReferenceAvailability,
) -> TrustedJobProvider {
    let authority = &placement.authority[0];
    assert_eq!(authority.contract_id.as_str(), JOB_EXECUTABLE_AUTHORITY);
    let mut provider = TrustedJobProvider::new(
        placement.host_id.clone(),
        placement.boot_id.clone(),
        placement.offer_generation,
        placement.capability_id.clone(),
    );
    provider
        .register_executable(
            placement.resources[0].pool_id.clone(),
            ResourceReferenceBinding {
                identity: request.executable().get().identity,
                version: request.executable().get().lifetime.version,
                content_profile: request.executable().get().content_profile.clone(),
                access_class: request.executable().get().access_class.clone(),
                handle: ResourceHandleId::from(format!("handle:{path}")),
                authority_contract: authority.contract_id.clone(),
                authority_grant: authority.grant_id.clone(),
                maximum_bytes: request.executable().get().extent.bytes,
                maximum_items: request.executable().get().extent.items,
                availability,
            },
            PathBuf::from(path),
        )
        .unwrap();
    provider
}
