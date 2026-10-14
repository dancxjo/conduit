use conduit_core::{
    ConfigurationValue, StructuredFieldValue, StructuredInfoTypeShape, StructuredInfoValue,
    StructuredInfoValueShape,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, PreparedPortableExpressionEvaluator, ProfileCatalog, StartupCatalog,
};
const GAIN: &str = include_str!("../../../semantics/audio/dsp/gain.conduit");
const ENVELOPE: &str = include_str!("../../../semantics/audio/dsp/envelope.conduit");
fn program(source: &str, name: &str) -> PortableExpressionProgram {
    let document = parse_syntax_document(source);
    assert_eq!(document.round_trip(), source);
    let checked = check_syntax_document(&document, &StartupCatalog::new()).unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, name, &ProfileCatalog::new()).unwrap();
    assert_eq!(expanded.expanded.gears.len(), 1);
    let ConfigurationValue::Text(bytes) = &expanded.expanded.gears[0].configuration[0].value else {
        panic!("program")
    };
    PortableExpressionProgram::from_canonical_hex(bytes).unwrap()
}
fn input(program: &PortableExpressionProgram, values: &[(&str, i32)], cancelled: bool) -> Vec<u8> {
    let StructuredInfoTypeShape::Record { fields, .. } = program.input_type.shape() else {
        panic!("record")
    };
    StructuredInfoValue::record(
        program.input_type.clone(),
        fields
            .iter()
            .map(|field| {
                let bytes = if field.name() == "cancelled" {
                    vec![u8::from(cancelled)]
                } else {
                    values
                        .iter()
                        .find(|(name, _)| *name == field.name())
                        .unwrap()
                        .1
                        .to_le_bytes()
                        .to_vec()
                };
                StructuredFieldValue::new(
                    field.name(),
                    StructuredInfoValue::leaf(field.value_type().clone(), bytes).unwrap(),
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}
fn result(bytes: &[u8]) -> (String, Option<i32>) {
    let value = StructuredInfoValue::from_canonical_bytes(bytes).unwrap();
    let StructuredInfoValueShape::Variant { tag, payload } = value.shape() else {
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
fn gain_matches_wide_reference_and_symmetric_saturation() {
    let p = program(GAIN, "dsp/gain");
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    let capacity = evaluator.output_capacity();
    for sample in [-32768, -32767, -257, -1, 0, 1, 257, 32767] {
        for gain in [0, 1, 128, 256, 257, 4096] {
            let expected = (i64::from(sample) * i64::from(gain) / 256).clamp(-32767, 32767) as i32;
            assert_eq!(
                result(
                    evaluator
                        .evaluate(&input(&p, &[("sample", sample), ("gain_q8", gain)], false))
                        .unwrap()
                ),
                ("sample".into(), Some(expected))
            );
            assert_eq!(evaluator.output_capacity(), capacity);
        }
    }
}
#[test]
fn envelope_endpoints_and_complete_curves_match_reference() {
    let p = program(ENVELOPE, "dsp/envelope");
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    let capacity = evaluator.output_capacity();
    for (total, attack, release) in [
        (64, 16, 16),
        (1, 0, 0),
        (128, 0, 32),
        (128, 32, 0),
        (64, 32, 32),
        (65536, 32768, 32768),
    ] {
        for frame in 0..=total {
            let expected = if frame == total {
                0
            } else if attack > 0 && frame < attack {
                i64::from(frame) * 256 / i64::from(attack)
            } else if release > 0 && frame > total - release {
                i64::from(total - frame) * 256 / i64::from(release)
            } else {
                256
            };
            let bytes = input(
                &p,
                &[
                    ("frame", frame),
                    ("total", total),
                    ("attack", attack),
                    ("release", release),
                ],
                false,
            );
            assert_eq!(
                result(evaluator.evaluate(&bytes).unwrap()),
                ("level_q8".into(), Some(expected as i32))
            );
            assert_eq!(evaluator.output_capacity(), capacity);
        }
    }
}
#[test]
fn invalid_profiles_cancel_and_restart_have_explicit_outcomes() {
    let gain = program(GAIN, "dsp/gain");
    for (sample, value) in [(i32::MIN, 256), (i32::MAX, 256), (1, -1), (1, i32::MAX)] {
        let values = [("sample", sample), ("gain_q8", value)];
        assert_eq!(
            result(&gain.evaluate(&input(&gain, &values, false)).unwrap()),
            ("invalid_profile".into(), None)
        );
        assert_eq!(
            result(&gain.evaluate(&input(&gain, &values, true)).unwrap()),
            ("cancelled".into(), None)
        );
    }
    let envelope = program(ENVELOPE, "dsp/envelope");
    for (frame, total, attack, release) in [
        (-1, 64, 16, 16),
        (65, 64, 16, 16),
        (0, 0, 0, 0),
        (0, i32::MAX, 0, 0),
        (0, 64, -1, 0),
        (0, 64, 0, i32::MIN),
        (0, 64, i32::MAX, i32::MAX),
        (0, 64, 33, 32),
    ] {
        let values = [
            ("frame", frame),
            ("total", total),
            ("attack", attack),
            ("release", release),
        ];
        assert_eq!(
            result(
                &envelope
                    .evaluate(&input(&envelope, &values, false))
                    .unwrap()
            ),
            ("invalid_profile".into(), None)
        );
        assert_eq!(
            result(&envelope.evaluate(&input(&envelope, &values, true)).unwrap()),
            ("cancelled".into(), None)
        );
    }
    let start = input(
        &envelope,
        &[("frame", 0), ("total", 64), ("attack", 16), ("release", 16)],
        false,
    );
    let before = envelope.evaluate(&start).unwrap();
    envelope
        .evaluate(&input(
            &envelope,
            &[
                ("frame", 32),
                ("total", 64),
                ("attack", 16),
                ("release", 16),
            ],
            true,
        ))
        .unwrap();
    assert_eq!(envelope.evaluate(&start).unwrap(), before);
}
