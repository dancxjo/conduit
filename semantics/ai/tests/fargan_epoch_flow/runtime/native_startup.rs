//! Native realization preparation and ordinary Source runtime proof.
use super::*;

pub(in super::super) fn run_native_first_feature(
    samples: &[i16; 80],
    period: &StructuredInfoValue,
) -> StructuredInfoValue {
    run_first_feature(samples, period, false)
}
pub(in super::super) fn run_native_first_feature16k(
    samples: &[i16; 160],
    period: &StructuredInfoValue,
) -> StructuredInfoValue {
    run_first_feature(samples, period, true)
}
fn run_first_feature<const N: usize>(
    samples: &[i16; N],
    period: &StructuredInfoValue,
    direct16k: bool,
) -> StructuredInfoValue {
    let (context, ids) = if direct16k {
        super::super::direct16k::prepare_first()
    } else {
        super::super::feature_cycle::prepare_first_feature()
    };
    let profile = context
        .native
        .iter()
        .find(|profile| profile.kind_identity(true) == ids["__FEATURE_EVENT_NATIVE__"])
        .unwrap();
    let ty = profile.value_type();
    fn pcm(ty: &StructuredInfoType, samples: &[i16]) -> StructuredInfoValue {
        match ty.shape() {
            StructuredInfoTypeShape::Nominal { representation, .. } => {
                StructuredInfoValue::nominal(ty.clone(), pcm(representation, samples)).unwrap()
            }
            StructuredInfoTypeShape::Collection { element, length } => {
                assert_eq!(samples.len(), length as usize);
                StructuredInfoValue::collection(
                    ty.clone(),
                    samples
                        .iter()
                        .map(|sample| {
                            StructuredInfoValue::leaf(
                                element.clone(),
                                sample.to_le_bytes().to_vec(),
                            )
                            .unwrap()
                        })
                        .collect(),
                )
                .unwrap()
            }
            _ => panic!("exact PCM80 collection"),
        }
    }
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("event")
    };
    let event = StructuredInfoValue::record(
        ty.clone(),
        fields
            .iter()
            .map(|field| {
                let value = match field.name() {
                    "samples" => pcm(field.value_type(), samples),
                    "period" => {
                        assert_eq!(field.value_type(), period.value_type());
                        period.clone()
                    }
                    "epoch" => StructuredInfoValue::leaf(
                        field.value_type().clone(),
                        0u64.to_le_bytes().to_vec(),
                    )
                    .unwrap(),
                    _ => panic!("event field"),
                };
                StructuredFieldValue::new(field.name(), value).unwrap()
            })
            .collect(),
    )
    .unwrap();
    let definition = super::super::declarations::exact_epoch_declarations()
        + "\n"
        + include_str!("../../../../speech/fargan_feature_epoch_contracts.conduit")
        + "\n"
        + include_str!("../../../../speech/fargan_feature_direct16k_contracts.conduit");
    let checked = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(&definition),
        &conduit_plot::StartupCatalog::new(),
    )
    .unwrap();
    let encoded = event.canonical_bytes().unwrap();
    super::super::interface::admit_retained_session_native(
        &checked,
        if direct16k {
            "FarganFeaturePcmEpoch16k"
        } else {
            "FarganFeaturePcmEpoch"
        },
        &encoded,
    )
    .unwrap();
    let source = if direct16k {
        super::super::direct16k::first_source(&ids)
    } else {
        super::super::feature_cycle::first_feature_source(&ids)
    };
    let (plan, context) = super::super::prepare_authored_epoch_entry(
        context,
        source,
        if direct16k {
            "speech/flow-fargan-feature-first16k-analysis"
        } else {
            "speech/flow-fargan-feature-first-analysis"
        },
        true,
        vec![],
    )
    .unwrap();
    let result = run_epoch_stream_plan(
        plan,
        &context,
        &analysis_resources(),
        BTreeMap::from([("value".into(), vec![encoded])]),
        None,
        1,
        ExecutionMode::Normal,
    )
    .unwrap();
    assert_eq!(result.values.len(), 1);
    result.values.into_iter().next().unwrap()
}

#[test]
fn first_native_feature_uses_source_cold_history_and_explicit_prior_memories() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| {
            let periods = run_native_period_controls(&[(false, 20480)]);
            let proposal = run_native_first_feature(&[1000; 80], &periods[0]);
            let history = super::super::case_state::field(&proposal, "history");
            let StructuredInfoValueShape::Collection(values) = history.shape() else {
                panic!("history")
            };
            assert_eq!(values.len(), 640);
            assert!(values[..480].iter().all(|value| {
                let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
                    panic!("scalar")
                };
                f32::from_le_bytes(bytes.try_into().unwrap()) == 0.
            }));
            assert_eq!(
                super::super::case_state::field(&proposal, "period"),
                &periods[0]
            );
        })
        .unwrap()
        .join()
        .unwrap();
}

pub(in super::super) fn run_native_warm_startup(
    model: &super::super::custody::RetainedSignalModel,
    first_proposal: &StructuredInfoValue,
) -> Vec<StructuredInfoValue> {
    assert_eq!(model.resources.len(), 33);
    let source = [
        include_str!("../../../../speech/fargan_conditioning.conduit"),
        include_str!("../../../../speech/fargan_signal.conduit"),
        include_str!("../../../../speech/fargan_pitch_history.conduit"),
        include_str!("../../../../speech/fargan_subframe.conduit"),
        include_str!("../../../../speech/fargan_warm_conditioning.conduit"),
        include_str!("../../../../speech/fargan_zero_continuation.conduit"),
        include_str!("../../../../speech/fargan_warm_startup.conduit"),
    ]
    .join("\n");
    let (plan, context) = super::super::prepare_authored_epoch_entry(
        super::super::prepared_epoch_profiles_with_capacity(true),
        source,
        "speech/fargan-warm-startup",
        true,
        vec![],
    )
    .unwrap();
    let inputs = BTreeMap::from([
        (
            "first_features".into(),
            vec![super::super::case_state::field(first_proposal, "features")
                .canonical_bytes()
                .unwrap()],
        ),
        (
            "first_period".into(),
            vec![super::super::case_state::field(first_proposal, "period")
                .canonical_bytes()
                .unwrap()],
        ),
    ]);
    let result = run_epoch_stream_plan(
        plan,
        &context,
        &model.resources,
        inputs,
        None,
        3,
        ExecutionMode::Normal,
    )
    .unwrap();
    assert_eq!(result.values.len(), 3);
    eprintln!(
        "native first-feature Source warm startup: {}nodes/{}cords; prep{:?}/execution{:?}",
        result.nodes, result.cords, result.preparation, result.execution
    );
    result.values
}

pub(in super::super) fn run_native_startup_feedback(
    warm: &[StructuredInfoValue],
    proposal: &StructuredInfoValue,
) -> Vec<StructuredInfoValue> {
    let (context, ids, offers) = super::super::native_startup::prepare_startup_profiles();
    let definition = super::super::native_startup::startup_definition();
    let checked = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(&definition),
        &conduit_plot::StartupCatalog::new(),
    )
    .unwrap();
    let ty = |name: &str| {
        &checked
            .native_types
            .iter()
            .find(|ty| ty.name == name)
            .unwrap()
            .value_type
    };
    let epoch = super::super::case_state::field(proposal, "epoch");
    let value = |name: &str| {
        warm.iter()
            .find(|value| value.value_type() == ty(name))
            .unwrap()
    };
    fn material(
        ty: &StructuredInfoType,
        values: &[(&str, &StructuredInfoValue)],
    ) -> StructuredInfoValue {
        let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
            panic!("startup domain")
        };
        StructuredInfoValue::record(
            ty.clone(),
            fields
                .iter()
                .map(|field| {
                    let value = values
                        .iter()
                        .find(|(name, _)| *name == field.name())
                        .unwrap()
                        .1;
                    assert_eq!(field.value_type(), value.value_type());
                    StructuredFieldValue::new(field.name(), value.clone()).unwrap()
                })
                .collect(),
        )
        .unwrap()
    }
    let signal = material(
        ty("FarganNativeSignalStartup"),
        &[
            ("state", value("FarganSubframeState")),
            ("conditioned_period", value("FarganPeriod")),
            ("first_epoch", epoch),
        ],
    );
    let conditioning = material(
        ty("FarganNativeConditioningStartup"),
        &[
            ("history", value("NumericHistory2x64")),
            ("first_epoch", epoch),
        ],
    );
    for (name, value) in [
        ("FarganNativeSignalStartup", &signal),
        ("FarganNativeConditioningStartup", &conditioning),
        ("FarganFeatureProposalEpoch", proposal),
    ] {
        super::super::interface::admit_retained_session_native(
            &checked,
            name,
            &value.canonical_bytes().unwrap(),
        )
        .unwrap();
    }
    let source =
        include_str!("../../../../speech/fargan_native_startup_feedback.conduit").to_owned();
    for id in ids.values() {
        assert!(source.contains(id));
    }
    let (plan, context) = super::super::prepare_authored_epoch_entry(
        context,
        source,
        "speech/fargan-native-startup-feedback",
        true,
        offers,
    )
    .unwrap();
    let resources: Resources = BTreeMap::new();
    let inputs = BTreeMap::from([
        ("signal".into(), vec![signal.canonical_bytes().unwrap()]),
        (
            "conditioning".into(),
            vec![conditioning.canonical_bytes().unwrap()],
        ),
        ("feature".into(), vec![proposal.canonical_bytes().unwrap()]),
    ]);
    let result = run_epoch_stream_plan(
        plan,
        &context,
        &resources,
        inputs,
        None,
        3,
        ExecutionMode::Normal,
    )
    .unwrap();
    assert_eq!(result.values.len(), 3);
    eprintln!(
        "Source native startup three feedback seeds: {}nodes/{}cords",
        result.nodes, result.cords
    );
    result.values
}

#[test]
fn native_startup_source_advances_three_separate_exact_domains_from_first_epoch() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| {
            let checked = conduit_plot::check_syntax_document(
                &conduit_plot::parse_syntax_document(
                    &super::super::native_startup::startup_definition(),
                ),
                &conduit_plot::StartupCatalog::new(),
            )
            .unwrap();
            let ty = |name: &str| {
                &checked
                    .native_types
                    .iter()
                    .find(|ty| ty.name == name)
                    .unwrap()
                    .value_type
            };
            let warm = ["FarganSubframeState", "NumericHistory2x64", "FarganPeriod"]
                .iter()
                .map(|name| super::super::declarations::fixture_value(ty(name)))
                .collect::<Vec<_>>();
            let proposal =
                super::super::declarations::fixture_value(ty("FarganFeatureProposalEpoch"));
            let seeds = run_native_startup_feedback(&warm, &proposal);
            for seed in seeds {
                let StructuredInfoValueShape::Leaf(bytes) =
                    super::super::case_state::field(&seed, "next_epoch").shape()
                else {
                    panic!("epoch")
                };
                assert_eq!(u64::from_le_bytes(bytes.try_into().unwrap()), 8);
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
