//! Actual BME Source schemas specialize the generic native finite join.
use super::*;
use conduit_composite::KernelOperationFactory;
use conduit_plot::{ProfileCatalog, StartupCatalog};

const SOURCE: &str = concat!(
    "with flow/zip/finite/paired as BmePair\n",
    include_str!("../../../../../../plots/device-protocols/bme280-lifecycle.conduit"),
    "\nplot bme280-join (\n pair: BmePair...| >> transition: BmeProtocolTransition...|\n) = ({ state: .0, event: .1 })\nplot native-state-event (\n >> state: BmeProtocolState...| <= 4096B\n >> event: BmeProtocolEvent...| <= 4096B\n >> request: I2cTransaction...|\n transition: BmeProtocolTransition...| >>\n result: I2cResult...| >>\n) {\n join: flow/zip/finite\n bus: machine/i2c/transact\n state >> join.left\n event >> join.right\n join.paired >> bme280-join() >> transition\n request >> bus >> result\n}\n"
);

pub(super) fn install(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> (CapabilityOffer, crate::flow_zip::FlowZipOperationFactory) {
    let checked = check_syntax_document(
        &parse_syntax_document(include_str!(
            "../../../../../../plots/device-protocols/bme280-lifecycle.conduit"
        )),
        startup,
    )
    .unwrap();
    let ty = |name| {
        &checked
            .native_types
            .iter()
            .find(|ty| ty.name == name)
            .unwrap()
            .value_type
    };
    let state = ty("BmeProtocolState");
    let event = ty("BmeProtocolEvent");
    let contract = |ty: &StructuredInfoType| {
        CheckedValueContract::new(ty.profile().unwrap().value_kind().clone(), 4096, vec![]).unwrap()
    };
    let left = contract(state);
    let right = contract(event);
    conduit_semantic_catalog::install_flow_zip_finite_kind(
        &left, state, &right, event, startup, profile,
    )
    .unwrap();
    let mut joins = crate::flow_zip::FlowZipOperationFactory::default();
    let offer = joins.install(&left, state, &right, event).unwrap();
    (offer, joins)
}

#[test]
fn source_state_event_join_selects_exact_native_schema_and_pure_storage() {
    let (plan, _, _, factory) =
        planned_named_source_with_joins(Provider, SOURCE, "native-state-event");
    factory.validate_plan(&plan).unwrap();
    assert!(
        crate::flow_zip::FlowZipOperationFactory::default()
            .validate_plan(&plan)
            .is_err()
    );
    let gear = plan.fragments[0]
        .placements
        .iter()
        .find(|gear| gear.implementation_id.as_str() == crate::flow_zip::IMPLEMENTATION)
        .unwrap();
    let budget = factory.budget(gear).unwrap();
    assert_eq!(budget.value_items, 3);
    assert_eq!(budget.host_requests, 0);
    assert!(budget.maximum_value_bytes <= crate::flow_zip::MAXIMUM_PAIR_BYTES);
    assert!(gear.host_calls.is_empty() && gear.resources.is_empty() && gear.authority.is_empty());
    let mut artifact = gear.clone();
    artifact.artifact_id = ArtifactId::from("forged/zip");
    let mut bounds = gear.clone();
    bounds.limits.max_queue_bytes += 1;
    let mut fore = gear.clone();
    fore.inputs[0].temporal = PortTemporal::Current;
    let mut hidden_call = gear.clone();
    hidden_call.host_calls = plan.fragments[0]
        .placements
        .iter()
        .find(|gear| gear.implementation_id.as_str() == I2C_IMPLEMENTATION)
        .unwrap()
        .host_calls
        .clone();
    for forged in [artifact, bounds, fore, hidden_call] {
        assert!(factory.budget(&forged).is_err());
    }
}

mod execution;
