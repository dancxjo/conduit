//! Real checked Quantity expression offers; these are selection, not Play proofs.
use super::*;
use conduit_core::{ConfigurationValue, PortTemporal, QuantityValueProfile};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, ProfileCatalog, StartupCatalog,
};

fn expression(name: &str, body: &str) -> (CheckedGear, PortableExpressionProgram) {
    let source =
        alloc::format!("plot {name} (\n >> value: Distance\n result: Distance >>\n) = ({body})\n");
    let checked =
        check_syntax_document(&parse_syntax_document(&source), &StartupCatalog::new()).unwrap();
    let mut expanded = expand_canonical_plot_for_authoring(&checked, name, &ProfileCatalog::new())
        .unwrap()
        .expanded;
    let gear = expanded.gears.remove(0);
    let ConfigurationValue::Text(encoded) = &gear.configuration[0].value else {
        panic!("program")
    };
    let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
    (gear, program)
}
fn offer(program: &PortableExpressionProgram, exact: bool, id: &str) -> CapabilityOffer {
    let mut offer = if exact {
        conduit_std_offers::pure_exact_quantity_std_offer(program, PortTemporal::Value).unwrap()
    } else {
        conduit_std_offers::pure_expression_std_offer(program, PortTemporal::Value).unwrap()
    };
    offer.capability_id = id.into();
    offer
}
fn fixture(
    producer_alternative: bool,
    consumer_alternative: bool,
) -> (
    Vec<CheckedGear>,
    Vec<CheckedConnection>,
    Vec<HostAdvertisement>,
) {
    let (producer, first) = expression("divide", ". / 3");
    let (consumer, second) = expression("restore", ". * 3");
    let connection = CheckedConnection {
        source_gear_id: producer.gear_id.clone(),
        source_port_id: "output".into(),
        sink_gear_id: consumer.gear_id.clone(),
        sink_port_id: "input".into(),
        value_kind: conduit_core::kind_id(conduit_core::DISTANCE_INFO_ID),
        track: ConnectionTrack::Payload,
        temporal: PortTemporal::Value,
    };
    let mut host = crate::tests::host();
    host.capabilities = vec![
        offer(&first, true, "a-producer-exact"),
        offer(&second, false, "a-consumer-native"),
    ];
    if producer_alternative {
        host.capabilities
            .push(offer(&first, false, "z-producer-native"));
    }
    if consumer_alternative {
        host.capabilities
            .push(offer(&second, true, "z-consumer-exact"));
    }
    (vec![producer, consumer], vec![connection], vec![host])
}
#[test]
fn default_selection_skips_incompatible_preferred_consumer_in_either_gear_order() {
    let (mut gears, connections, hosts) = fixture(false, true);
    let consumer = gears[1].gear_id.clone();
    for _ in 0..2 {
        let placements =
            super::super::default_placements_unvalidated(&gears, &connections, &hosts).unwrap();
        assert_eq!(
            placements.by_gear[&consumer].capability_id.as_str(),
            "z-consumer-exact"
        );
        gears.reverse();
    }
}
#[test]
fn selection_backtracks_to_compatible_producer_instead_of_weakening_consumer() {
    let (gears, connections, hosts) = fixture(true, false);
    let placements = select(&gears, &connections, &hosts).unwrap();
    assert_eq!(
        placements.by_gear[&gears[0].gear_id].capability_id.as_str(),
        "z-producer-native"
    );
    assert_eq!(
        placements.by_gear[&gears[1].gear_id].capability_id.as_str(),
        "a-consumer-native"
    );
}
#[test]
fn incompatible_wire_and_exhausted_search_have_distinct_refusals() {
    let (gears, connections, hosts) = fixture(false, false);
    assert!(matches!(
        select(&gears, &connections, &hosts),
        Err(PlannerError::QuantityWireProfileMismatch(_))
    ));
    let candidates = rows(&gears, &hosts).unwrap();
    let edges = edges(&gears, &connections).unwrap();
    let why = search(&gears, &candidates, &edges, 0).unwrap_err();
    assert!(matches!(why, PlannerError::PlannerLimitExceeded(_)));
    assert!(why
        .to_string()
        .contains("quantity/wire-selection-attempts-bound@1"));
}
#[test]
fn malformed_port_declaration_is_not_an_eligible_alternative() {
    let (gears, connections, mut hosts) = fixture(false, true);
    let candidate = hosts[0].capabilities.last_mut().unwrap();
    candidate
        .realization_properties
        .retain(|p| p.profile() != &QuantityWireContract::property_kind());
    candidate.realization_properties.push(
        QuantityWireContract::new(vec![("absent".into(), QuantityValueProfile::Exact)], vec![])
            .unwrap()
            .to_property()
            .unwrap(),
    );
    candidate
        .realization_properties
        .sort_by(|a, b| a.profile().cmp(b.profile()));
    assert!(matches!(
        select(&gears, &connections, &hosts),
        Err(PlannerError::InvalidRealizationProperties(_))
    ));
}
#[test]
fn instance_limits_are_checked_while_backtracking() {
    let (mut gears, _, mut hosts) = fixture(false, false);
    gears[1] = gears[0].clone();
    gears[1].gear_id = "second-instance".into();
    hosts[0].capabilities.truncate(1);
    hosts[0].capabilities[0].limits.max_active_instances = 1;
    let mut alternative = hosts[0].capabilities[0].clone();
    alternative.capability_id = "z-alternative-instance".into();
    hosts[0].capabilities.push(alternative);
    let placements = select(&gears, &[], &hosts).unwrap();
    assert_ne!(
        placements.by_gear[&gears[0].gear_id].capability_id,
        placements.by_gear[&gears[1].gear_id].capability_id
    );
}
