//! Native retained-state specialization derived from the actual BME Source Type.
use super::*;
use conduit_composite::KernelOperationFactory;

const SOURCE: &str = concat!(
    include_str!("../../../../../../plots/device-protocols/bme280-lifecycle.conduit"),
    "\nplot native-state-sample (\n >> current: $BmeProtocolState <= 4096B\n >> trigger: U64...| <= 8B\n >> request: I2cTransaction...|\n sampled: BmeProtocolState...| <= 4096B >>\n result: I2cResult...| >>\n) {\n sample: current/sample/finite\n bus: machine/i2c/transact\n current >> sample.current\n trigger >> sample.trigger\n sample.value >> sampled\n request >> bus >> result\n}\n"
);

#[test]
fn source_state_sampling_selects_exact_native_storage_without_effect_authority() {
    let (plan, _, _) = planned_named_source(Provider, SOURCE, "native-state-sample");
    assert!(verify_plan(&plan));
    let gear = plan.fragments[0]
        .placements
        .iter()
        .find(|gear| gear.implementation_id.as_str() == crate::current_sample::IMPLEMENTATION)
        .unwrap();
    let factory = crate::current_sample::CurrentSampleOperationFactory::default();
    let budget = factory.budget(gear).unwrap();
    assert!(budget.maximum_value_bytes <= crate::current_sample::MAXIMUM_BYTES);
    assert_eq!(budget.value_items, 1);
    assert_eq!(budget.host_requests, 0);
    assert!(gear.host_calls.is_empty() && gear.resources.is_empty() && gear.authority.is_empty());
    let mut wrong_back = gear.clone();
    wrong_back.artifact_id = ArtifactId::from("forged/sampler");
    let mut wrong_bound = gear.clone();
    wrong_bound.limits.max_queue_bytes += 1;
    let mut wrong_fore = gear.clone();
    wrong_fore.inputs[0].temporal = PortTemporal::Value;
    let mut hidden_call = gear.clone();
    hidden_call.host_calls = plan.fragments[0]
        .placements
        .iter()
        .find(|p| p.implementation_id.as_str() == I2C_IMPLEMENTATION)
        .unwrap()
        .host_calls
        .clone();
    for forged in [wrong_back, wrong_bound, wrong_fore, hidden_call] {
        assert!(factory.budget(&forged).is_err());
    }
}

mod execution;
