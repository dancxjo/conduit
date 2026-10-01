use conduit_core::{
    kind_id, BaseImplementationId, CheckedValueContract, ResourceClassId, ResourceOffer,
    ResourcePoolId, StateLifetime, StructuredInfoType, StructuredInfoValue, TEXT_INFO_ID,
};
use std::collections::BTreeMap;

#[test]
fn canonical_durable_notebook_checks_and_seals_distinct_retention_publication_and_load_truth() {
    let text_contract = CheckedValueContract::new(
        kind_id("value/text"),
        conduit_data::MAXIMUM_DATA_TEXT_BYTES,
        vec![],
    )
    .unwrap();
    let save_request_contract =
        CheckedValueContract::new(kind_id("data/save-request@1"), 0, vec![]).unwrap();
    let text_type = StructuredInfoType::leaf(kind_id(TEXT_INFO_ID)).unwrap();
    let initial = StructuredInfoValue::leaf(text_type.clone(), Vec::new()).unwrap();
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    startup
        .insert_value_kind_alias("Text", kind_id(TEXT_INFO_ID))
        .unwrap();
    startup
        .insert_value_kind_alias("SaveRequest", kind_id("data/save-request@1"))
        .unwrap();
    conduit_semantic_catalog::state_value::install_state_value_kind(
        "Text",
        &text_type,
        &initial,
        &mut startup,
        &mut profile,
    )
    .unwrap();
    startup
        .insert(conduit_form::KindSignature {
            kind: "state/latest".into(),
            startup_parameters: vec![],
        })
        .unwrap();
    conduit_data::install_data_text_catalogs(&mut startup, &mut profile).unwrap();
    conduit_semantic_catalog::install_current_sample_kind(
        &text_contract,
        &save_request_contract,
        &mut startup,
        &mut profile,
    )
    .unwrap();
    let form = conduit_form::parse_with_startup(
        include_str!("../../../forms/durable-notebook/main.conduit"),
        &startup,
        &profile,
    )
    .expect("canonical Durable Notebook parses and checks");

    let mut advertisement = conduit_std_host::StdHost::new().advertisement().clone();
    advertisement.capabilities.push(
        conduit_std_offers::current_sample_offer(&text_contract, &save_request_contract).unwrap(),
    );
    advertisement.capabilities.retain(|offer| {
        offer.kind_id.as_str() != conduit_semantic_catalog::state_value::STATE_VALUE_KIND
    });
    advertisement.capabilities.push(
        conduit_std_offers::state_value_durable_std_offer("Text", &text_type, &initial).unwrap(),
    );
    advertisement.resources.push(ResourceOffer {
        pool_id: ResourcePoolId::from("pool/durable-notebook-state"),
        class_id: ResourceClassId::from(conduit_std_offers::STATE_VALUE_DURABLE_RESOURCE_CLASS),
        capacity_units: 1,
        compute: None,
        content: None,
    });
    advertisement
        .capabilities
        .sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
    advertisement
        .resources
        .sort_by(|left, right| left.pool_id.cmp(&right.pool_id));
    let hosts = [advertisement];
    let placements = conduit_planner::default_placements(&form, &hosts)
        .expect("Durable Notebook placements resolve");
    let plan = conduit_planner::plan_with_options(
        &form,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_data::MAXIMUM_DATA_TEXT_BYTES,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .expect("Durable Notebook plans through exact std Backs");

    let fragment = &plan.fragments[0];
    let [state] = fragment.states.as_slice() else {
        panic!("Durable Notebook must seal one retained State")
    };
    assert_eq!(state.lifetime, StateLifetime::Body);
    assert_eq!(
        state.maximum_value_bytes,
        conduit_data::MAXIMUM_DATA_TEXT_BYTES
    );

    let placement = |kind: &str| {
        fragment
            .placements
            .iter()
            .find(|placement| placement.kind_id.as_str() == kind)
            .unwrap_or_else(|| panic!("missing {kind} placement"))
    };
    let keep = fragment
        .placements
        .iter()
        .find(|placement| placement.gear_id == state.gear_id)
        .expect("retained State placement exists");
    assert_eq!(
        keep.implementation_id.as_str(),
        conduit_std_offers::STATE_VALUE_DURABLE_STD_IMPLEMENTATION
    );
    assert_eq!(keep.host_calls.len(), 2);
    assert_eq!(keep.resources.len(), 1);

    let sample = placement(conduit_semantic_catalog::CURRENT_SAMPLE_KIND);
    assert_eq!(
        sample.implementation_id.as_str(),
        conduit_std_offers::CURRENT_SAMPLE_IMPLEMENTATION
    );
    assert!(sample.host_calls.is_empty());

    let save = placement(conduit_data::DATA_SAVE_TEXT_KIND);
    let load = placement(conduit_data::DATA_LOAD_TEXT_KIND);
    assert_eq!(save.resources.len(), 1);
    assert_eq!(load.resources.len(), 1);
    assert_eq!(save.resources[0].pool_id, load.resources[0].pool_id);
    assert_ne!(save.resources[0].pool_id, keep.resources[0].pool_id);
    assert!(save.authority.is_empty());
    assert!(load.authority.is_empty());
}
