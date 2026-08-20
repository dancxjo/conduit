use conduit_core::{
    kind_id, BootId, BoundedResourceRef, ConnectionBase, HostAdvertisement, HostId, HostProfileId,
    OfferGeneration, PortTemporal, ResourceClassId, ResourceExtent, ResourceLifetime,
    ResourceSemanticIdentity, ResourceVersionIdentity, StructuredInfoValue,
    StructuredInfoValueShape, PROTOCOL_VERSION,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    structured_selector_definition, CheckedCordStage, ProfileCatalog, StartupCatalog,
};
use conduit_std_catalog::{
    deterministic_person_provider, deterministic_query_error, deterministic_query_result,
    filter_active_rows, install_tabular_catalogs, materialized_query_outcome, tabular_std_offers,
    PersonRow, TabularRefusal, TABULAR_FILTER_KIND, TABULAR_HOST_OPERATION, TABULAR_MAXIMUM_ROWS,
    TABULAR_PROVIDER_KIND,
};

const SOURCE: &str = include_str!("../../../examples/tabular-query.conduit");

#[test]
fn canonical_form_filters_and_projects_rows_without_sql_or_json() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_tabular_catalogs(&mut startup, &mut profile).unwrap();
    let syntax = parse_syntax_document(SOURCE);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let mut selector_offers = Vec::new();
    for stage in checked.forms[0]
        .cords
        .iter()
        .flat_map(|cord| cord.stages.iter())
    {
        if let CheckedCordStage::StructuredSelector { selector, .. } = stage {
            profile
                .insert(structured_selector_definition(
                    selector,
                    PortTemporal::Value,
                ))
                .unwrap();
            selector_offers.push(conduit_std_catalog::structured_selector_std_offer(
                selector,
                PortTemporal::Value,
            ));
        }
    }
    assert_eq!(selector_offers.len(), 4);
    let authored =
        expand_canonical_form_for_authoring(&checked, "tabular-query", &profile).unwrap();
    let mut offers = tabular_std_offers();
    offers.extend(selector_offers);
    let host = host(offers);
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
    let filter = plan.fragments[0]
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == TABULAR_FILTER_KIND)
        .unwrap();
    assert_eq!(
        filter.host_operations[0].contract_id.as_str(),
        TABULAR_HOST_OPERATION
    );
    assert!(plan.fragments[0]
        .placements
        .iter()
        .any(|placement| placement.kind_id.as_str() == TABULAR_PROVIDER_KIND));
}

#[test]
fn deterministic_provider_preserves_types_null_and_end_of_results() {
    let result = deterministic_person_provider().unwrap();
    let rows = collection_field(&result, "rows");
    assert_eq!(rows.len(), usize::from(TABULAR_MAXIMUM_ROWS));
    let first = variant_payload(&rows[0], "row");
    assert_eq!(leaf_text(record_field(first, "name")), "Ada");
    assert_eq!(variant_tag(record_field(first, "nickname")), "null");
    assert_eq!(variant_tag(&rows[3]), "unused");
    let status = record_field(&result, "status");
    assert_eq!(variant_tag(status), "complete");
    let completion = variant_payload(status, "complete");
    assert_eq!(leaf_text(record_field(completion, "emitted_rows")), "3");
}

#[test]
fn filter_keeps_active_rows_without_string_coercion() {
    let filtered = filter_active_rows(&deterministic_person_provider().unwrap()).unwrap();
    let rows = collection_field(&filtered, "rows");
    let names = rows[..2]
        .iter()
        .map(|slot| leaf_text(record_field(variant_payload(slot, "row"), "name")))
        .collect::<Vec<_>>();
    assert_eq!(names, vec!["Ada", "Edsger"]);
    assert_eq!(variant_tag(&rows[2]), "unused");
}

#[test]
fn errors_oversize_and_materialization_remain_distinct() {
    let error = deterministic_query_error("provider/refused", "query refused").unwrap();
    assert_eq!(variant_tag(record_field(&error, "status")), "error");
    assert_eq!(filter_active_rows(&error).unwrap(), error);
    let rows = vec![
        PersonRow {
            id: 1,
            name: "row",
            nickname: None,
            active: true,
        };
        usize::from(TABULAR_MAXIMUM_ROWS) + 1
    ];
    assert_eq!(
        deterministic_query_result(&rows),
        Err(TabularRefusal::TooManyRows {
            maximum: TABULAR_MAXIMUM_ROWS,
            actual: 5,
        })
    );
    let reference = BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest([1; 32]),
        content_profile: kind_id("tabular/person-rows@1"),
        access_class: ResourceClassId::from("content/read@1"),
        extent: ResourceExtent {
            bytes: 4096,
            items: Some(64),
        },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest([2; 32]),
            expires_at: None,
        },
    };
    let materialized = materialized_query_outcome(&reference).unwrap();
    assert_eq!(variant_tag(&materialized), "materialized");
    assert!(format!("{materialized:?}").find("SELECT").is_none());
}

fn host(capabilities: Vec<conduit_core::CapabilityOffer>) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("host/tabular-proof"),
        boot_id: BootId::from("boot/tabular-proof"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("std/tabular-proof@1"),
        resources: vec![],
        planner_capabilities: vec![],
        capabilities,
    }
}

fn record_field<'a>(value: &'a StructuredInfoValue, name: &str) -> &'a StructuredInfoValue {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!("expected record")
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .unwrap()
        .value()
}

fn collection_field<'a>(value: &'a StructuredInfoValue, name: &str) -> &'a [StructuredInfoValue] {
    let StructuredInfoValueShape::Collection(values) = record_field(value, name).shape() else {
        panic!("expected collection")
    };
    values
}

fn variant_tag(value: &StructuredInfoValue) -> &str {
    let StructuredInfoValueShape::Variant { tag, .. } = value.shape() else {
        panic!("expected variant")
    };
    tag
}

fn variant_payload<'a>(value: &'a StructuredInfoValue, expected: &str) -> &'a StructuredInfoValue {
    let StructuredInfoValueShape::Variant { tag, payload } = value.shape() else {
        panic!("expected variant")
    };
    assert_eq!(tag, expected);
    payload
}

fn leaf_text(value: &StructuredInfoValue) -> &str {
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        panic!("expected leaf")
    };
    core::str::from_utf8(bytes).unwrap()
}
