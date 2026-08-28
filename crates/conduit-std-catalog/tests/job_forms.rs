use conduit_core::{
    authority_grant, resource_offer, BaseImplementationId, BootId, HostAdvertisement, HostId,
    HostProfileId, OfferGeneration, StructuredInfoTypeShape, DEFAULT_CONNECTION_BYTE_CAPACITY,
    DEFAULT_CONNECTION_ITEM_CAPACITY, PROTOCOL_VERSION,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use conduit_std_catalog::{
    install_job_catalogs, job_lifecycle_type, job_request_type, JOB_ARGUMENT_SLOTS,
    JOB_ENVIRONMENT_SLOTS, JOB_EXECUTABLE_AUTHORITY, JOB_RUN_KIND,
};

const SOURCE: &str = include_str!("../../../examples/bounded-job.conduit");

#[test]
fn ordinary_form_plans_one_bounded_admitted_job() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_job_catalogs(&mut startup, &mut profile).unwrap();
    let syntax = parse_syntax_document(SOURCE);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let authored = expand_canonical_form_for_authoring(&checked, "bounded-job", &profile).unwrap();
    let host = host(common::job_proof_offers());
    let run_offer = host
        .capabilities
        .iter()
        .find(|offer| offer.kind_id.as_str() == JOB_RUN_KIND)
        .unwrap();
    let grant = authority_grant(
        "grant/job-execute",
        &run_offer.authority_requirements[0],
        host.host_id.clone(),
        host.boot_id.clone(),
        run_offer.capability_id.clone(),
    );
    let placements = conduit_planner::default_expanded_placements(
        &authored.expanded,
        core::slice::from_ref(&host),
    )
    .unwrap();
    let connection_bases = std::collections::BTreeMap::new();
    let line_candidates = std::collections::BTreeMap::new();
    let plan = conduit_planner::plan_expanded_canonical_with_options(
        &authored.expanded,
        core::slice::from_ref(&host),
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &connection_bases,
            line_candidates: &line_candidates,
            connection_item_capacity: DEFAULT_CONNECTION_ITEM_CAPACITY,
            connection_byte_capacity: DEFAULT_CONNECTION_BYTE_CAPACITY,
            authority_grants: &[grant],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .unwrap();
    let run = plan.fragments[0]
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == JOB_RUN_KIND)
        .unwrap();
    assert_eq!(
        run.host_operations[0].contract_id.as_str(),
        common::JOB_PROOF_RUN_OPERATION
    );

    let offer = common::job_proof_offers()
        .into_iter()
        .find(|offer| offer.kind_id.as_str() == JOB_RUN_KIND)
        .unwrap();
    assert_eq!(offer.resource_requirements.len(), 1);
    assert_eq!(
        offer.resource_requirements[0].class_id.as_str(),
        common::JOB_PROOF_RESOURCE_CLASS
    );
    assert_eq!(offer.resource_requirements[0].units, 1);
    assert_eq!(offer.authority_requirements.len(), 1);
    assert_eq!(
        offer.authority_requirements[0].contract_id.as_str(),
        JOB_EXECUTABLE_AUTHORITY
    );
}

#[test]
fn schemas_make_all_collections_and_terminal_outcomes_finite() {
    let request_type = job_request_type();
    let StructuredInfoTypeShape::Record { fields, .. } = request_type.shape() else {
        panic!("expected request record")
    };
    let arguments = fields
        .iter()
        .find(|field| field.name() == "arguments")
        .unwrap();
    let environment = fields
        .iter()
        .find(|field| field.name() == "environment")
        .unwrap();
    assert!(matches!(
        arguments.value_type().shape(),
        StructuredInfoTypeShape::Collection { length, .. } if usize::from(length) == JOB_ARGUMENT_SLOTS
    ));
    assert!(matches!(
        environment.value_type().shape(),
        StructuredInfoTypeShape::Collection { length, .. } if usize::from(length) == JOB_ENVIRONMENT_SLOTS
    ));

    let lifecycle_type = job_lifecycle_type();
    let StructuredInfoTypeShape::Variant { cases, .. } = lifecycle_type.shape() else {
        panic!("expected lifecycle variant")
    };
    let tags: Vec<_> = cases.iter().map(|case| case.tag()).collect();
    for expected in [
        "started",
        "running",
        "completed",
        "failed",
        "cancelled",
        "timed_out",
        "provider_lost",
    ] {
        assert!(tags.contains(&expected));
    }
}

fn host(capabilities: Vec<conduit_core::CapabilityOffer>) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("host/job-proof"),
        boot_id: BootId::from("boot/job-proof"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("std/job-proof@1"),
        resources: vec![resource_offer(
            "pool/job-executable",
            common::JOB_PROOF_RESOURCE_CLASS,
            1,
        )],
        planner_capabilities: vec![],
        capabilities,
    }
}
mod common;
