//! Exact opaque latest-tuple shapes for a stateless chart constructor family.
use alloc::{format, string::String, vec, vec::Vec};
use conduit_core::*;
pub const MEASUREMENT_CHART_CONSTRUCTOR_KIND: &str = "data/measurement-chart";
pub fn measurement_chart_constructor_arity(kind: &str) -> Option<usize> {
    match kind {
        "data/measurement-chart/1" => Some(1),
        "data/measurement-chart/2" => Some(2),
        "data/measurement-chart/3" => Some(3),
        "data/measurement-chart/4" => Some(4),
        _ => None,
    }
}
pub fn measurement_chart_constructor_input(
    count: usize,
) -> Result<(StructuredInfoType, u32), &'static str> {
    if !(1..=super::MAXIMUM_MEASUREMENT_CHART_SERIES).contains(&count) {
        return Err("chart requires 1..4 series");
    }
    let series = super::measurement_plot_series_type();
    let series_bound = (series.canonical_bytes().map_err(|_| "series Type")?.len()
        + 5
        + super::MAXIMUM_MEASUREMENT_PLOT_SERIES_BYTES) as u32;
    let mut ty = series.clone();
    let mut bound = series_bound;
    for _ in 1..count {
        let pair = PreparedTuplePairEncoder::new(
            ty.profile()
                .map_err(|_| "pair profile")?
                .value_kind()
                .clone(),
            bound,
            series
                .profile()
                .map_err(|_| "series profile")?
                .value_kind()
                .clone(),
            series_bound,
        )
        .map_err(|_| "pair bound")?;
        ty = pair.value_type().map_err(|_| "pair Type")?;
        bound = pair.maximum_bytes();
    }
    Ok((ty, bound))
}
pub fn measurement_chart_labels_type() -> StructuredInfoType {
    StructuredInfoType::bounded_sequence(
        StructuredInfoType::leaf(kind_id(TEXT_INFO_ID)).unwrap(),
        1,
        4,
    )
    .expect("bounded chart labels")
}
pub fn measurement_chart_constructor_semantic_contract(count: usize) -> Result<Kind, &'static str> {
    let (input, bound) = measurement_chart_constructor_input(count)?;
    let labels = measurement_chart_labels_type();
    // Required Source startup has NO default; this valid Type-level template
    // merely supplies the configuration field's representation, never labels a Host.
    let template = StructuredInfoValue::sequence(
        labels.clone(),
        (0..count)
            .map(|i| {
                StructuredInfoValue::leaf(
                    StructuredInfoType::leaf(kind_id(TEXT_INFO_ID)).unwrap(),
                    format!("series-{}", i + 1).into_bytes(),
                )
                .unwrap()
            })
            .collect(),
    )
    .map_err(|_| "label template")?
    .canonical_bytes()
    .map_err(|_| "label template")?;
    let labels_profile = labels.profile().unwrap().value_kind().clone();
    let input = PortDescriptor {
        port_id: port_id("latest"),
        value_kind: input.profile().unwrap().value_kind().clone(),
        direction: PortDirection::Input,
        temporal: PortTemporal::Flow { closes: true },
        abnormal_kind: Some(kind_id(TERMINAL_INFO_ID)),
    };
    let output = PortDescriptor {
        port_id: port_id("chart"),
        value_kind: super::measurement_chart_type()
            .profile()
            .unwrap()
            .value_kind()
            .clone(),
        direction: PortDirection::Output,
        temporal: PortTemporal::Flow { closes: true },
        abnormal_kind: Some(kind_id(TERMINAL_INFO_ID)),
    };
    let terminal = CheckedValueContract::new(
        kind_id(TERMINAL_INFO_ID),
        TERMINAL_INFO_ENCODED_LEN as u32,
        vec![],
    )
    .unwrap();
    Ok(Kind {
        kind_id: kind_id(&format!("{MEASUREMENT_CHART_CONSTRUCTOR_KIND}/{count}")),
        kind_contract_revision: format!("conduit.data/measurement-chart-{count}@1").into(),
        startup_parameters: vec![
            FrontStartupParameter {
                name: "labels".into(),
                value_type: labels_profile.clone(),
                has_default: false,
            },
            FrontStartupParameter {
                name: "maximum-updates".into(),
                value_type: kind_id(COUNT_INFO_ID),
                has_default: true,
            },
        ],
        shorthand: None,
        configuration: vec![
            KindConfigurationField {
                key: "labels".into(),
                default_value: ConfigurationValue::Structured(
                    StructuredConfigurationValue::new(labels_profile.clone(), template)
                        .ok_or("label template")?,
                ),
                rule: KindConfigurationRule::Structured {
                    profile: labels_profile.clone(),
                },
            },
            KindConfigurationField {
                key: "maximum-updates".into(),
                default_value: ConfigurationValue::U64(1024),
                rule: KindConfigurationRule::U64Range {
                    minimum: 1,
                    maximum: 4096,
                },
            },
        ],
        inputs: vec![input.clone()],
        outputs: vec![output.clone()],
        semantic_laws: vec![
            KindSemanticLaw::ValueContracts(vec![
                FrontValueContract {
                    location: FrontValueLocation::Startup("labels".into()),
                    contract: CheckedValueContract::new(labels_profile, 512, vec![]).unwrap(),
                },
                FrontValueContract {
                    location: FrontValueLocation::Input(input.port_id.clone()),
                    contract: CheckedValueContract::new(input.value_kind, bound, vec![]).unwrap(),
                },
                FrontValueContract {
                    location: FrontValueLocation::Output(output.port_id.clone()),
                    contract: CheckedValueContract::new(
                        output.value_kind,
                        super::measurement_chart_canonical_bytes() as u32,
                        vec![],
                    )
                    .unwrap(),
                },
                FrontValueContract {
                    location: FrontValueLocation::InputAbnormal(input.port_id.clone()),
                    contract: terminal.clone(),
                },
                FrontValueContract {
                    location: FrontValueLocation::OutputAbnormal(output.port_id.clone()),
                    contract: terminal,
                },
            ]),
            KindSemanticLaw::TerminalTransduction(TerminalTransductionProfile {
                input_port_id: input.port_id,
                output_port_id: output.port_id,
                normal_close: NormalCloseTransduction::FlushThenPropagate(FiniteTerminalEmission {
                    maximum_items: 1,
                    maximum_bytes: super::measurement_chart_canonical_bytes() as u32,
                }),
                abnormal: AbnormalTerminalTransduction::FinalizeThenPropagate(
                    FiniteTerminalEmission {
                        maximum_items: 1,
                        maximum_bytes: super::measurement_chart_canonical_bytes() as u32,
                    },
                ),
                cancellation: CancellationTransduction::NotCancellable,
            }),
        ],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: bound.max(super::measurement_chart_canonical_bytes() as u32),
        },
    })
}
pub fn install_measurement_chart_constructor(
    count: usize,
    startup: &mut conduit_plot::StartupCatalog,
    profiles: &mut conduit_plot::ProfileCatalog,
) -> Result<KindId, String> {
    let kind = measurement_chart_constructor_semantic_contract(count).map_err(String::from)?;
    startup.ensure_structured_type("ChartLabels", measurement_chart_labels_type())?;
    startup.insert(conduit_plot::KindSignature {
        kind: kind.kind_id.as_str().into(),
        startup_parameters: vec![
            conduit_plot::StartupParameterSignature {
                name: "labels".into(),
                value_type: "ChartLabels".into(),
                default: None,
            },
            conduit_plot::StartupParameterSignature {
                name: "maximum-updates".into(),
                value_type: "Count".into(),
                default: Some("1024".into()),
            },
        ],
    })?;
    profiles
        .insert_kind(kind.clone())
        .map_err(|e| format!("{e}"))?;
    Ok(kind.kind_id)
}
/// Validate immutable authored metadata once during preparation.
pub fn measurement_chart_constructor_labels(
    configuration: &[ConfigurationEntry],
    count: usize,
) -> Result<Vec<String>, &'static str> {
    if !(1..=4).contains(&count) {
        return Err("chart label count bound");
    }
    if configuration.len() != 2 {
        return Err("chart requires exact labels and maximum-updates startup");
    }
    measurement_chart_constructor_maximum_updates(configuration)?;
    let value = configuration
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            ("labels", ConfigurationValue::Structured(value)) => Some(value),
            _ => None,
        })
        .ok_or("chart requires labels startup")?;
    let ty = measurement_chart_labels_type();
    let bytes = ty.canonical_bytes().map_err(|_| "labels Type")?;
    let validator =
        BorrowedStructuredValueValidator::new(&bytes, 512).map_err(|_| "labels bound")?;
    let value = validator
        .validate(value.canonical_value())
        .map_err(|_| "labels encoding")?;
    let mut labels = Vec::with_capacity(count);
    let length = value.collection_length().map_err(|_| "labels sequence")?;
    for index in 0..length {
        let item = value
            .collection_index(index as u16)
            .map_err(|_| "labels sequence")?
            .ok_or("labels sequence")?;
        let label = core::str::from_utf8(
            item.primitive_bytes(TEXT_INFO_ID)
                .map_err(|_| "label Type")?,
        )
        .map_err(|_| "labels UTF8")?;
        if label.trim().is_empty()
            || label.len() > 64
            || label.chars().any(char::is_control)
            || labels.iter().any(|prior| prior == label)
        {
            return Err("chart labels must be unique bounded names");
        }
        labels.push(String::from(label));
    }
    if labels.len() != count {
        return Err("chart label count differs from exact series shape");
    }
    Ok(labels)
}

/// Finite execution scope; reaching the limit retires/refuses this Play rather than enlarging it.
pub fn measurement_chart_constructor_maximum_updates(
    configuration: &[ConfigurationEntry],
) -> Result<u32, &'static str> {
    match configuration
        .iter()
        .find(|entry| entry.key == "maximum-updates")
        .map(|entry| &entry.value)
    {
        Some(ConfigurationValue::U64(value)) if (1..=4096).contains(value) => Ok(*value as u32),
        _ => Err("chart requires maximum-updates in 1..4096"),
    }
}
