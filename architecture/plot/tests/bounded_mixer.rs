use conduit_core::{
    tuple_info_type, ConfigurationValue, StructuredFieldValue, StructuredInfoType,
    StructuredInfoTypeShape, StructuredInfoValue, StructuredInfoValueShape,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    prepare_source_types, PortableExpressionProgram, PreparedPortableExpressionEvaluator,
    ProfileCatalog, StartupCatalog,
};
const SOURCE: &str = include_str!("../../../semantics/audio/dsp/mixer.conduit");
fn program(name: &str) -> PortableExpressionProgram {
    let syntax = parse_syntax_document(SOURCE);
    assert_eq!(syntax.round_trip(), SOURCE);
    let types = prepare_source_types(&syntax, &StartupCatalog::new()).unwrap();
    let channel = types.named_type("MixerChannel").unwrap().clone();
    let pair = tuple_info_type(vec![channel.clone(), channel]).unwrap();
    let quad = tuple_info_type(vec![pair.clone(), pair.clone()]).unwrap();
    let mut startup = StartupCatalog::new();
    startup.insert_structured_type("MixerPair", pair).unwrap();
    startup.insert_structured_type("MixerQuad", quad).unwrap();
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let authored =
        expand_canonical_plot_for_authoring(&checked, name, &ProfileCatalog::new()).unwrap();
    assert_eq!(authored.expanded.gears.len(), 1);
    let ConfigurationValue::Text(encoded) = &authored.expanded.gears[0].configuration[0].value
    else {
        panic!("program")
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}
fn value(
    ty: &StructuredInfoType,
    channels: &mut std::slice::Iter<'_, [i32; 5]>,
) -> StructuredInfoValue {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("record")
    };
    let channel = fields
        .iter()
        .any(|f| f.name() == "sample")
        .then(|| channels.next().unwrap());
    StructuredInfoValue::record(
        ty.clone(),
        fields
            .iter()
            .map(|field| {
                let child = if let Some(channel) = channel {
                    let index = ["sample", "gain_q8", "timeline", "frame", "rate_hz"]
                        .iter()
                        .position(|name| *name == field.name())
                        .unwrap();
                    StructuredInfoValue::leaf(
                        field.value_type().clone(),
                        channel[index].to_le_bytes().to_vec(),
                    )
                    .unwrap()
                } else {
                    value(field.value_type(), channels)
                };
                StructuredFieldValue::new(field.name(), child).unwrap()
            })
            .collect(),
    )
    .unwrap()
}
fn evaluate(
    p: &PortableExpressionProgram,
    evaluator: &mut PreparedPortableExpressionEvaluator,
    channels: &[[i32; 5]],
) -> (String, Option<i32>) {
    let mut cursor = channels.iter();
    let input = value(&p.input_type, &mut cursor).canonical_bytes().unwrap();
    assert!(cursor.next().is_none());
    let result =
        StructuredInfoValue::from_canonical_bytes(evaluator.evaluate(&input).unwrap()).unwrap();
    let StructuredInfoValueShape::Variant { tag, payload } = result.shape() else {
        panic!("outcome")
    };
    let sample = match payload.shape() {
        StructuredInfoValueShape::Leaf(bytes) if bytes.len() == 4 => {
            Some(i32::from_le_bytes(bytes.try_into().unwrap()))
        }
        _ => None,
    };
    (tag.into(), sample)
}
#[test]
fn two_and_four_channels_match_wide_reference_with_fixed_prepared_capacity() {
    for (name, count) in [("dsp/mix-two", 2), ("dsp/mix-four", 4)] {
        let p = program(name);
        let mut evaluator = PreparedPortableExpressionEvaluator::new(&p).unwrap();
        let capacity = evaluator.output_capacity();
        let mut cancellation = vec![[0, 0, 7, 11, 48000]; count];
        cancellation[0] = [32767, 4096, 7, 11, 48000];
        cancellation[1] = [-32768, 4096, 7, 11, 48000];
        assert_eq!(
            evaluate(&p, &mut evaluator, &cancellation),
            ("sample".into(), Some(-16))
        );
        assert_eq!(evaluator.output_capacity(), capacity);
        for sample in [-32768, -32767, -257, -1, 0, 1, 257, 32767] {
            for gain in [0, 1, 128, 256, 257, 4096] {
                let channels: Vec<_> = (0..count)
                    .map(|i| {
                        [
                            if i % 2 == 0 { sample } else { sample / 2 },
                            gain,
                            7,
                            11,
                            48000,
                        ]
                    })
                    .collect();
                let expected = channels
                    .iter()
                    .map(|c| i64::from(c[0]) * i64::from(c[1]) / 256)
                    .sum::<i64>()
                    .clamp(-32767, 32767) as i32;
                assert_eq!(
                    evaluate(&p, &mut evaluator, &channels),
                    ("sample".into(), Some(expected))
                );
                assert_eq!(evaluator.output_capacity(), capacity);
            }
        }
    }
}
#[test]
fn invalid_profiles_and_unequal_timebases_refuse_without_a_sample() {
    for (name, count) in [("dsp/mix-two", 2), ("dsp/mix-four", 4)] {
        let p = program(name);
        let mut evaluator = PreparedPortableExpressionEvaluator::new(&p).unwrap();
        for index in 0..count {
            for (field, bad) in [
                (0, i32::MIN),
                (0, 32768),
                (1, -1),
                (1, 4097),
                (2, -1),
                (3, 65536),
                (4, 0),
                (4, i32::MAX),
            ] {
                let mut channels = vec![[1, 256, 7, 11, 48000]; count];
                channels[index][field] = bad;
                assert_eq!(
                    evaluate(&p, &mut evaluator, &channels),
                    ("invalid_profile".into(), None)
                );
            }
            if index > 0 {
                for field in [2, 3, 4] {
                    let mut channels = vec![[1, 256, 7, 11, 48000]; count];
                    channels[index][field] += 1;
                    assert_eq!(
                        evaluate(&p, &mut evaluator, &channels),
                        ("timebase_mismatch".into(), None)
                    );
                }
            }
        }
    }
}
