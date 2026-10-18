use super::{
    tests::{authored, source},
    StdHost,
};
use conduit_core::{
    BaseImplementationId, ConnectionTrack, PortDirection, StructuredInfoValue,
    StructuredInfoValueShape, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};
use std::collections::BTreeMap;

#[test]
fn the_selected_plan_retains_why_the_reference_profile_was_offered() {
    let text = source(". == 1 ? \"yes\" : \"no\"").replace("result: U64", "result: Text");
    let authored = authored(&text);
    let mut host = StdHost::new();
    host.prepare_expression_realizations(&authored.expanded)
        .unwrap();
    let offer = host
        .advertisement()
        .capabilities
        .iter()
        .find(|offer| super::owns(offer))
        .unwrap();
    assert_eq!(
        offer.implementation.implementation_id.as_str(),
        conduit_std_offers::PURE_EXPRESSION_STD_IMPLEMENTATION
    );
    let evidence = StructuredInfoValue::from_canonical_bytes(
        offer.realization_properties[0].canonical_value(),
    )
    .unwrap();
    let StructuredInfoValueShape::Record(fields) = evidence.shape() else {
        panic!()
    };
    assert_eq!(fields[0].name(), "tape-profile-refusal");
    let StructuredInfoValueShape::Leaf(bytes) = fields[0].value().shape() else {
        panic!()
    };
    assert!(std::str::from_utf8(bytes)
        .unwrap()
        .contains("scalar-tape/fixed-integer-bool@1"));
    let hosts = [host.advertisement().clone()];
    let placements =
        conduit_planner::default_expanded_placements(&authored.expanded, &hosts).unwrap();
    let boundaries = [
        (PortDirection::Input, "value"),
        (PortDirection::Output, "result"),
    ]
    .into_iter()
    .map(|(direction, name)| {
        (
            conduit_planner::ForeBoundaryKey {
                direction,
                front_port_id: name.into(),
                track: ConnectionTrack::Payload,
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            },
        )
    })
    .collect();
    let plan = conduit_planner::plan_expanded_authoring_with_options(
        &authored,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap();
    assert!(conduit_core::verify_plan(&plan));
    assert!(
        plan.fragments[0].placements[0].realization_properties == offer.realization_properties,
        "the selected Plan dropped eligibility evidence"
    );
}

#[test]
fn unrelated_source_keeps_the_advertisement_unchanged() {
    let mut host = StdHost::new();
    let before = host.advertisement().clone();
    host.prepare_expression_realizations(&authored("plot arithmetic {\n}\n").expanded)
        .unwrap();
    assert!(
        host.advertisement() == &before,
        "unrelated Source changed the Host"
    );
}

#[test]
fn an_unused_native_record_input_is_not_automatically_advertised() {
    let plot = authored("type Small = U64 in 0..=10\ntype Envelope = {\n value: Small\n}\nplot arithmetic (\n >> value: Envelope\n result: U64 >>\n) = (7)\n");
    let mut host = StdHost::new();
    let before = host.advertisement().clone();
    let why = host
        .prepare_expression_realizations(&plot.expanded)
        .unwrap_err();
    assert!(why.contains("native-law-witness-unavailable"), "{why}");
    assert!(
        host.advertisement() == &before,
        "unsupported field laws changed the Host"
    );
}
