use conduit_core::{
    BootId, ConnectionBase, HostAdvertisement, HostId, HostProfileId, OfferGeneration,
    StructuredInfoTypeShape, PROTOCOL_VERSION,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use conduit_std_catalog::{
    install_job_catalogs, job_lifecycle_type, job_request_type, job_std_offers, JOB_ARGUMENT_SLOTS,
    JOB_ENVIRONMENT_SLOTS, JOB_EXECUTABLE_AUTHORITY, JOB_EXECUTABLE_RESOURCE_CLASS, JOB_RUN_KIND,
    JOB_RUN_OPERATION,
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
    let host = host(job_std_offers());
    let placements = conduit_planner::default_expanded_placements(
        &authored.expanded,
        core::slice::from_ref(&host),
    )
    .unwrap();
    let plan = conduit_planner::plan_expanded_canonical(
        &authored.expanded,
        &[host],
        &placements,
        &[ConnectionBase::Local],
    )
    .unwrap();
    let run = plan.fragments[0]
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == JOB_RUN_KIND)
        .unwrap();
    assert_eq!(
        run.host_operations[0].contract_id.as_str(),
        JOB_RUN_OPERATION
    );

    let offer = job_std_offers()
        .into_iter()
        .find(|offer| offer.kind_id.as_str() == JOB_RUN_KIND)
        .unwrap();
    assert_eq!(offer.resource_requirements.len(), 1);
    assert_eq!(
        offer.resource_requirements[0].class_id.as_str(),
        JOB_EXECUTABLE_RESOURCE_CLASS
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
        resources: vec![],
        planner_capabilities: vec![],
        capabilities,
    }
}
