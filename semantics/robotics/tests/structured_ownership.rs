use conduit_core::{Quantity, QuantityUnit, StructuredInfoValueShape};
use conduit_robotics::{
    range_sample_value, robotics_range_observation_type, robotics_structured_kind_contracts,
    RangeObservation, ROBOTICS_EXECUTE_MOTION_KIND,
};

#[test]
fn compact_and_enriched_range_observations_remain_distinct() {
    let compact = RangeObservation::new(420, 12).unwrap();
    assert_eq!(compact.encode().len(), 8);

    let enriched = range_sample_value(
        "sensor/front",
        43,
        Quantity::new(430, QuantityUnit::Millisecond),
        "sensor/front",
        compact.distance(),
        Quantity::new(5, QuantityUnit::Millimeter),
    )
    .unwrap();
    assert_eq!(enriched.value_type(), &robotics_range_observation_type());

    let StructuredInfoValueShape::Record(fields) = enriched.shape() else {
        panic!("enriched robotics observation must remain structured");
    };
    assert!(fields.iter().any(|field| field.name() == "sample"));
    assert!(fields.iter().any(|field| field.name() == "measurement"));
}

#[test]
fn structured_robotics_contracts_are_owned_by_robotics() {
    assert!(robotics_structured_kind_contracts()
        .iter()
        .any(|(kind, _, _)| kind.as_str() == ROBOTICS_EXECUTE_MOTION_KIND));
}
