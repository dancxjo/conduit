//! Native realization preparation and ordinary Source runtime proof.
use super::*;

#[test]
fn native_control_period_uses_source_rounding_and_exact_profile_in_ordinary_plan() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| {
            let result = run_native_period_controls(&[
                (false, 0u64),
                (false, u64::MAX),
                (true, u64::MAX),
                (false, 20480),
            ]);
            for (value, expected) in result.iter().zip([32u16, 255, 64, 80]) {
                let encoded = value.canonical_bytes().unwrap();
                assert_eq!(&encoded[encoded.len() - 2..], expected.to_le_bytes());
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

pub(in super::super) fn run_native_period_controls(
    observations: &[(bool, u64)],
) -> Vec<StructuredInfoValue> {
    let source = super::super::native_session::control_source();
    let context = super::super::prepared_epoch_profiles_with_capacity(true);
    let checked = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(&source),
        &context.startup,
    )
    .unwrap();
    let ty = &checked
        .native_types
        .iter()
        .find(|ty| ty.name == "FarganNativeCycleObservation")
        .unwrap()
        .value_type;
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("control observation")
    };
    let values = observations
        .iter()
        .map(|(boundary, q8)| {
            StructuredInfoValue::record(
                ty.clone(),
                fields
                    .iter()
                    .map(|field| {
                        StructuredFieldValue::new(
                            field.name(),
                            StructuredInfoValue::leaf(
                                field.value_type().clone(),
                                if field.name() == "boundary" {
                                    vec![u8::from(*boundary)]
                                } else {
                                    q8.to_le_bytes().to_vec()
                                },
                            )
                            .unwrap(),
                        )
                        .unwrap()
                    })
                    .collect(),
            )
            .unwrap()
            .canonical_bytes()
            .unwrap()
        })
        .collect();
    let (plan, context) = super::super::prepare_authored_epoch_entry(
        context,
        source,
        "speech/flow-fargan-native-control-period",
        true,
        vec![],
    )
    .unwrap();

    let resources: Resources = BTreeMap::new();
    let result = run_epoch_stream_plan(
        plan,
        &context,
        &resources,
        BTreeMap::from([("value".into(), values)]),
        None,
        observations.len(),
        ExecutionMode::Normal,
    )
    .unwrap();
    for value in &result.values {
        let validator =
            conduit_ai::fixed_numeric_u16_profile::PreparedU16Profile::check_definition(
                "type FarganPeriod = U16 in 32..=255\n",
            )
            .unwrap();
        assert_eq!(value.value_type(), validator.value_type());
    }
    eprintln!(
        "ordinary Source native control→period: {}nodes/{}cords; exact finite U16 profile after Source rounding",
        result.nodes, result.cords
    );
    result.values
}
