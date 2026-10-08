use conduit_plot::*;

fn declaration(source: &str, name: &str) -> String {
    let prefix = format!("type {name} =");
    let mut selected = false;
    let mut lines = Vec::new();
    for line in source.lines() {
        if selected && (line.starts_with("type ") || line.starts_with("plot ")) {
            break;
        }
        if line.starts_with(&prefix) {
            selected = true;
        }
        if selected {
            lines.push(line);
        }
    }
    assert!(selected, "{name}");
    lines.join("\n")
}

#[test]
fn exact_source_trace_profiles_fit_separate_frames_and_refuse_combined_history() {
    let numeric = include_str!("../../../semantics/ai/fixed_numeric.conduit");
    let signal = include_str!("../../../semantics/ai/fixed_numeric_signal.conduit");
    let mut source = String::new();
    for name in [
        "NumericFiniteF32",
        "NumericF32Vector20",
        "NumericF32Vector64",
        "NumericF32Vector128",
        "NumericF32Vector320",
        "NumericHistory2x64",
    ] {
        source.push_str(&declaration(numeric, name));
        source.push('\n');
    }
    for name in [
        "NumericF32Vector1",
        "NumericF32Vector160",
        "NumericF32Vector164",
        "NumericF32Vector256",
        "NumericI16Vector160",
    ] {
        source.push_str(&declaration(signal, name));
        source.push('\n');
    }
    for (text, name) in [
        (
            include_str!("../../../semantics/speech/fargan_conditioning.conduit"),
            "FarganPeriod",
        ),
        (
            include_str!("../../../semantics/speech/fargan_signal.conduit"),
            "FarganSignalState",
        ),
        (
            include_str!("../../../semantics/speech/fargan_subframe.conduit"),
            "FarganSubframeState",
        ),
        (
            include_str!("../../../semantics/speech/fargan_epoch_contracts.conduit"),
            "FarganPcm16EpochResult",
        ),
    ] {
        source.push_str(&declaration(text, name));
        source.push('\n');
    }
    source.push_str(include_str!(
        "../../../semantics/speech/fargan_model_identity.conduit"
    ));
    source.push_str(include_str!(
        "../../../semantics/speech/fargan_epoch_trace.conduit"
    ));
    source.push_str(include_str!(
        "../../../semantics/speech/fargan_trace_flow.conduit"
    ));
    source.push_str("\ntype FarganOversizedTrace = {\n result: FarganPcm16EpochResult\n features: NumericF32Vector20\n condition: NumericF32Vector320\n history: NumericHistory2x64\n}\n");
    let checked =
        check_syntax_document(&parse_syntax_document(&source), &StartupCatalog::new()).unwrap();
    let ty = |name: &str| {
        &checked
            .native_types
            .iter()
            .find(|t| t.name == name)
            .unwrap()
            .value_type
    };
    for name in [
        "FarganFeatureConditionTrace",
        "FarganConditionHistoryTrace",
        "FarganCommittedResultTrace",
    ] {
        let maximum = maximum_prepared_transport_value_bytes(ty(name)).unwrap();
        eprintln!("{name}: {maximum} canonical transport bytes");
        assert!(maximum <= 16384);
    }
    assert_ne!(
        ty("FarganFeatureConditionTrace"),
        ty("FarganConditionHistoryTrace")
    );
    let maximum = maximum_prepared_transport_value_bytes(ty("FarganOversizedTrace")).unwrap();
    eprintln!("combined PCM/state/features/condition/history: {maximum}");
    assert!(maximum > 16384);
}

#[path = "../../../semantics/ai/tests/fargan_epoch_flow/allocation_probe.rs"]
mod allocation_probe;
#[path = "../../../proof/fargan/development_trace_recorder.rs"]
mod recorder;

#[test]
fn bounded_observer_refuses_gaps_foreign_anchor_nonfinite_and_overbooking() {
    use conduit_core::*;
    use recorder::*;
    let checked = check_syntax_document(&parse_syntax_document("type TraceAnchor = collection U8 = 2\ntype TraceRow = {\n epoch: U64\n model: TraceAnchor\n value: F32\n}\n"), &StartupCatalog::new()).unwrap();
    let ty = |name: &str| {
        checked
            .native_types
            .iter()
            .find(|ty| ty.name == name)
            .unwrap()
            .value_type
            .clone()
    };
    fn anchor_value(ty: &StructuredInfoType, byte: u8) -> StructuredInfoValue {
        match ty.shape() {
            StructuredInfoTypeShape::Nominal { representation, .. } => {
                StructuredInfoValue::nominal(ty.clone(), anchor_value(representation, byte))
                    .unwrap()
            }
            StructuredInfoTypeShape::Collection { element, length } => {
                StructuredInfoValue::collection(
                    ty.clone(),
                    (0..length)
                        .map(|_| StructuredInfoValue::leaf(element.clone(), vec![byte]).unwrap())
                        .collect(),
                )
                .unwrap()
            }
            _ => panic!("anchor"),
        }
    }
    let anchor = |byte| anchor_value(&ty("TraceAnchor"), byte);
    let selected = anchor(1);
    let row = |epoch: u64, model: StructuredInfoValue, sample: f32| {
        let row_type = ty("TraceRow");
        let StructuredInfoTypeShape::Record { fields, .. } = row_type.shape() else {
            panic!()
        };
        let values = fields
            .iter()
            .map(|field| {
                let value = match field.name() {
                    "epoch" => StructuredInfoValue::leaf(
                        field.value_type().clone(),
                        epoch.to_le_bytes().to_vec(),
                    )
                    .unwrap(),
                    "model" => model.clone(),
                    _ => StructuredInfoValue::leaf(
                        field.value_type().clone(),
                        sample.to_le_bytes().to_vec(),
                    )
                    .unwrap(),
                };
                StructuredFieldValue::new(field.name(), value).unwrap()
            })
            .collect();
        StructuredInfoValue::record(ty("TraceRow"), values)
            .unwrap()
            .canonical_bytes()
            .unwrap()
    };
    let maximum = maximum_prepared_transport_value_bytes(&ty("TraceRow")).unwrap() as usize;
    let mut recorder =
        DevelopmentTraceRecorder::prepare(&ty("TraceRow"), &selected, 1, 2, maximum).unwrap();
    assert_eq!(recorder.prepared_payload_capacity(), maximum * 2);
    assert_eq!(recorder.recorded_rows(), 0);
    assert_eq!(recorder.finish().err(), Some(TraceRefusal::Incomplete));
    assert_eq!(
        recorder.record(&row(2, selected.clone(), 0.0)),
        Err(TraceRefusal::Epoch)
    );
    assert_eq!(
        recorder.record(&row(1, anchor(2), 0.0)),
        Err(TraceRefusal::Anchor)
    );
    assert_eq!(
        recorder.record(&row(1, selected.clone(), f32::NAN)),
        Err(TraceRefusal::NonFinite)
    );
    assert_eq!(recorder.record(&[0]), Err(TraceRefusal::Structure));
    let first = row(1, selected.clone(), 0.25);
    let (accepted, allocations) = allocation_probe::measure(|| recorder.record(&first));
    assert_eq!(accepted, Ok(()));
    assert_eq!(allocations, 0);
    assert_eq!(recorder.record(&first), Err(TraceRefusal::Epoch));
    let second = row(2, selected.clone(), 0.5);
    let (accepted, allocations) = allocation_probe::measure(|| recorder.record(&second));
    assert_eq!(accepted, Ok(()));
    assert_eq!(allocations, 0);
    assert_eq!(recorder.finish().unwrap(), [first, second]);
    assert_eq!(
        recorder.record(&row(3, selected.clone(), 0.0)),
        Err(TraceRefusal::Capacity)
    );
    for (count, bound) in [(0, maximum), (257, maximum), (2, 16385)] {
        assert!(
            DevelopmentTraceRecorder::prepare(&ty("TraceRow"), &selected, 1, count, bound).is_err()
        );
    }
}

#[test]
fn complete_carrier_extent_preserves_original_pin_and_legitimate_slow_duration() {
    use recorder::*;
    for (samples, epochs, rows, aligned) in [
        (5040, 63, 64, 10080),
        (8240, 103, 104, 16480),
        (16240, 203, 204, 32480),
    ] {
        let extent = NativeTraceExtent::prepare(samples).unwrap();
        assert_eq!(
            (
                extent.native_epochs,
                extent.output_rows,
                extent.aligned_samples_16k
            ),
            (epochs, rows, aligned)
        );
    }
    for samples in [0, 79, 8241, 20480] {
        assert!(NativeTraceExtent::prepare(samples).is_err());
    }
}
