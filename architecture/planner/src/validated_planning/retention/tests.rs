use super::*;
use conduit_core::{ConfigurationEntry, ConfigurationValue, PortDirection, PortTemporal};
fn state_placement() -> PlannedGear {
    let plot = crate::tests::plot();
    let host = crate::tests::host();
    let choices = crate::default_placements(&plot, core::slice::from_ref(&host)).unwrap();
    let mut plan = crate::plan(&plot, &[host], &choices, &["conduit.base/local@1".into()]).unwrap();
    let mut gear = plan.fragments[0].placements.remove(0);
    gear.kind_id = "state/latest".into();
    gear.limits.max_queue_bytes = conduit_core::QUANTITY_ENCODED_LEN as u32;
    gear.outputs.truncate(1);
    gear.outputs[0].value_kind = conduit_core::kind_id(conduit_core::DISTANCE_INFO_ID);
    gear.inputs = gear.outputs.clone();
    gear.inputs[0].direction = PortDirection::Input;
    gear.inputs[0].temporal = PortTemporal::Value;
    gear.outputs[0].temporal = PortTemporal::Current;
    let initial = conduit_core::Quantity::new(3, conduit_core::Unit::Meter);
    gear.configuration = vec![
        ConfigurationEntry {
            key: "retained-duration".into(),
            value: ConfigurationValue::Text("wake".into()),
        },
        ConfigurationEntry {
            key: "maximum-bytes".into(),
            value: ConfigurationValue::U64(conduit_core::QUANTITY_ENCODED_LEN as u64),
        },
        ConfigurationEntry {
            key: "initial".into(),
            value: ConfigurationValue::Quantity(
                conduit_core::QuantityConfigurationValue::from_value(initial).unwrap(),
            ),
        },
    ];
    gear
}
#[test]
fn retiring_configuration_preserves_state_and_source_index() {
    let mut gear = state_placement();
    let expected = planned_keep_state(&gear).unwrap();
    assert!(expected.as_ref().unwrap().initial_value.is_some());
    let mut retention = ConfigurationRetention::new(true);
    retention.placement(3, &mut gear);
    assert!(gear.configuration.is_empty());
    assert_eq!(retention.indices[&gear.placement_id], 3);
    assert_eq!(retention.state(&gear).unwrap(), expected);
}
#[test]
fn state_refusal_waits_for_the_original_admission_phase_and_rich_mode_keeps_configuration() {
    let mut gear = state_placement();
    gear.configuration[0].value = ConfigurationValue::Bool(true);
    let expected = planned_keep_state(&gear).unwrap_err();
    let mut deferred = ConfigurationRetention::new(true);
    deferred.placement(0, &mut gear); // Capture does not return an early refusal.
    assert!(gear.configuration.is_empty());
    assert_eq!(deferred.state(&gear), Err(expected));
    let mut gear = state_placement();
    let original = gear.clone();
    let mut rich = ConfigurationRetention::new(false);
    rich.placement(0, &mut gear);
    assert_eq!(gear, original);
    assert!(rich.indices.is_empty());
    assert_eq!(rich.state(&gear), planned_keep_state(&original));
}
