use super::*;
fn labels(names: &[&str]) -> ConfigurationValue {
    let leaf = StructuredInfoType::leaf(kind_id(TEXT_INFO_ID)).unwrap();
    let value = StructuredInfoValue::sequence(
        measurement_chart_labels_type(),
        names
            .iter()
            .map(|name| StructuredInfoValue::leaf(leaf.clone(), name.as_bytes().to_vec()).unwrap())
            .collect(),
    )
    .unwrap();
    ConfigurationValue::Structured(
        StructuredConfigurationValue::new(
            value.value_type().profile().unwrap().value_kind().clone(),
            value.canonical_bytes().unwrap(),
        )
        .unwrap(),
    )
}
fn placement(count: usize) -> PlannedGear {
    let offer = measurement_chart_constructor_std_offer(count).unwrap();
    let names = ["Blue", "Green", "Yellow", "White"];
    let terminal_transductions = measurement_chart_constructor_semantic_contract(count)
        .unwrap()
        .terminal_transductions()
        .cloned()
        .collect();
    conduit_core::planned_gear_from_parts! {placement_id:"constructor".into(),gear_id:"constructor".into(),kind_id:offer.kind_id,kind_contract_revision:offer.kind_contract_revision,execution_profile_id:offer.implementation.execution_profile_id,configuration:vec![ConfigurationEntry {key:"labels".into(),value:labels(&names[..count])},ConfigurationEntry {key:"maximum-updates".into(),value:ConfigurationValue::U64(16)}],host_id:"host/test".into(),boot_id:"boot/test".into(),offer_generation:OfferGeneration(1),capability_id:offer.capability_id,implementation_id:offer.implementation.implementation_id,artifact_id:offer.implementation.artifact_id,base:None,realization_characteristics:vec![],limits:offer.limits,inputs:offer.inputs,outputs:offer.outputs,terminal_transductions:terminal_transductions,semantic_contract:offer.semantic_contract,host_calls:offer.host_calls,resources:vec![],authority:vec![],pool_references:vec![]}
}
fn input(count: usize) -> Vec<u8> {
    let series = MeasurementPlotSeries::from_projected(
        vec![MeasurementPlotPoint::new(0, 0, 250_000).unwrap()],
        1,
        0,
    )
    .unwrap();
    let series_type = measurement_plot_series_type();
    let series_bytes = StructuredInfoValue::leaf(
        series_type.clone(),
        encode_measurement_plot_series(&series).unwrap(),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let mut current = series_bytes.clone();
    let mut ty = series_type.clone();
    for _ in 1..count {
        let mut encoder = PreparedTuplePairEncoder::new(
            ty.profile().unwrap().value_kind().clone(),
            current.len() as u32,
            series_type.profile().unwrap().value_kind().clone(),
            series_bytes.len() as u32,
        )
        .unwrap();
        ty = encoder.value_type().unwrap();
        current = encoder.encode(&current, &series_bytes).unwrap().to_vec();
    }
    current
}
#[test]
fn exact_one_through_four_opaque_tuples_construct_a_single_labelled_chart() {
    for count in 1..=4 {
        let placement = placement(count);
        let mut provider = PreparedMeasurementChartConstructor::prepare(&placement).unwrap();
        let bytes = input(count);
        let value = provider.execute(&bytes).unwrap();
        let value = validate_canonical_structured_value(value).unwrap();
        let chart =
            decode_measurement_chart(value.primitive_bytes(MEASUREMENT_CHART_INFO_ID).unwrap())
                .unwrap();
        assert_eq!(chart.series().len(), count);
        assert_eq!(chart.series()[0].label(), "Blue");
        assert!(chart
            .series()
            .iter()
            .all(|entry| entry.series().source_samples() == 1));
        let mut wrong = bytes.clone();
        wrong[0] ^= 1;
        assert!(provider.execute(&wrong).is_err());
        assert!(provider
            .execute(&vec![0; provider.input_bound() as usize + 1])
            .is_err());
    }
}
#[test]
fn immutable_label_count_names_and_exact_placement_are_checked_before_execution() {
    for names in [
        &["Blue", "Blue", "Yellow"][..],
        &["Blue", "Green"][..],
        &["Blue", "\n", "Yellow"][..],
    ] {
        let mut placement = placement(3);
        placement.configuration[0].value = labels(names);
        assert!(PreparedMeasurementChartConstructor::prepare(&placement).is_err());
    }
    let mut placement = placement(3);
    placement.artifact_id = "different".into();
    assert!(PreparedMeasurementChartConstructor::prepare(&placement).is_err());
    assert!(measurement_chart_constructor_input(0).is_err());
    assert!(measurement_chart_constructor_input(5).is_err());
}
