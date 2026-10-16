//! Authored complementary filters; deterministic software evidence only.
use conduit_core::{
    ConfigurationValue, StructuredFieldValue, StructuredInfoTypeShape, StructuredInfoValue,
    StructuredInfoValueShape,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, PreparedPortableExpressionEvaluator, ProfileCatalog, StartupCatalog,
};
#[path = "common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
const SOURCE: &str = include_str!("../../../semantics/audio/dsp/one-pole.conduit");
fn program() -> PortableExpressionProgram {
    let source = parse_syntax_document(SOURCE);
    assert_eq!(source.round_trip(), SOURCE);
    let checked = check_syntax_document(&source, &StartupCatalog::new()).unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, "dsp/one-pole", &ProfileCatalog::new())
            .unwrap();
    assert_eq!(expanded.expanded.gears.len(), 1);
    let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!()
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}
fn input(
    p: &PortableExpressionProgram,
    x: i32,
    y: i32,
    a: i32,
    remaining: i32,
    reset: bool,
    cancelled: bool,
) -> Vec<u8> {
    let StructuredInfoTypeShape::Record { fields, .. } = p.input_type.shape() else {
        panic!()
    };
    StructuredInfoValue::record(
        p.input_type.clone(),
        fields
            .iter()
            .map(|f| {
                let bytes = match f.name() {
                    "reset" => vec![u8::from(reset)],
                    "cancelled" => vec![u8::from(cancelled)],
                    name => match name {
                        "sample" => x,
                        "history" => y,
                        "retention_q14" => a,
                        "remaining" => remaining,
                        _ => panic!(),
                    }
                    .to_le_bytes()
                    .to_vec(),
                };
                StructuredFieldValue::new(
                    f.name(),
                    StructuredInfoValue::leaf(f.value_type().clone(), bytes).unwrap(),
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}
fn decode(bytes: &[u8]) -> (String, [i32; 4]) {
    let value = StructuredInfoValue::from_canonical_bytes(bytes).unwrap();
    let StructuredInfoValueShape::Variant { tag, payload } = value.shape() else {
        panic!()
    };
    let mut out = [0; 4];
    if tag == "samples" {
        let StructuredInfoValueShape::Record(fields) = payload.shape() else {
            panic!()
        };
        for (i, name) in ["low", "high", "history", "remaining"].iter().enumerate() {
            let field = fields.iter().find(|f| f.name() == *name).unwrap();
            let StructuredInfoValueShape::Leaf(bytes) = field.value().shape() else {
                panic!()
            };
            out[i] = i32::from_le_bytes(bytes.try_into().unwrap());
        }
    }
    (tag.to_string(), out)
}
fn reference(x: i32, y: i32, a: i32, remaining: i32, reset: bool) -> [i32; 4] {
    let y = if reset { 0 } else { y };
    let low = (i64::from(x) * (16384 - i64::from(a)) + i64::from(y) * i64::from(a)) / 16384;
    [
        low as i32,
        (i64::from(x) - low).clamp(-32767, 32767) as i32,
        low as i32,
        remaining - 1,
    ]
}
#[test]
fn extrema_and_reset_match_independent_wide_recurrence_in_both_evaluators() {
    let p = program();
    let mut e = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    for x in [-32767, -16384, -1, 0, 1, 16384, 32767] {
        for y in [-32767, -1, 0, 1, 32767] {
            for a in [0, 1, 8191, 8192, 16382, 16383] {
                for reset in [false, true] {
                    let input = input(&p, x, y, a, 65536, reset, false);
                    let output = e.evaluate(&input).unwrap();
                    assert_eq!(output, p.evaluate(&input).unwrap());
                    assert_eq!(
                        decode(output),
                        ("samples".into(), reference(x, y, a, 65536, reset))
                    );
                }
            }
        }
    }
}
fn history(blocks: &[usize]) -> Vec<Vec<u8>> {
    let p = program();
    let mut out = Vec::new();
    let mut y = 0;
    let mut i = 0;
    for &block in blocks {
        let mut e = PreparedPortableExpressionEvaluator::new(&p).unwrap();
        for _ in 0..block {
            let x = ((i * 7919) % 65535) as i32 - 32767;
            let a = [0, 16383, 8192, 1][i % 4];
            let reset = i == 0 || i == 31;
            let bytes = e
                .evaluate(&input(&p, x, y, a, (64 - i) as i32, reset, false))
                .unwrap()
                .to_vec();
            let (tag, state) = decode(&bytes);
            assert_eq!(tag, "samples");
            assert_eq!(state, reference(x, y, a, (64 - i) as i32, reset));
            assert!(state[0].abs() <= x.abs().max(if reset { 0 } else { y.abs() }));
            y = state[2];
            out.push(bytes);
            i += 1;
        }
    }
    assert_eq!(i, 64);
    out
}
#[test]
fn admitted_coefficient_modulation_and_serialized_state_survive_fresh_windows() {
    assert_eq!(history(&[64]), history(&[1, 15, 17, 31]));
}
#[test]
fn fixed_pole_impulse_dc_and_nyquist_match_independent_response_profile() {
    let p = program();
    let mut e = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    let mut y = 0;
    for i in 0..16 {
        let x = if i == 0 { 16384 } else { 0 };
        let (_, s) = decode(
            e.evaluate(&input(&p, x, y, 8192, 64 - i, false, false))
                .unwrap(),
        );
        assert_eq!(s[0], if i < 14 { 8192 >> i } else { 0 });
        y = s[2];
    }
    y = 0;
    for i in 0..64 {
        let (_, s) = decode(
            e.evaluate(&input(&p, 16384, y, 8192, 64 - i, false, false))
                .unwrap(),
        );
        y = s[2];
    }
    assert!((16384 - y).abs() <= 2); // quantization bias, not an ideal zero-error DC claim
    y = 0;
    for i in 0..128 {
        let x = if i % 2 == 0 { 32767 } else { -32767 };
        let (_, s) = decode(
            e.evaluate(&input(&p, x, y, 8192, 128 - i, false, false))
                .unwrap(),
        );
        if i > 32 {
            assert!((f64::from(s[0]) - f64::from(x) / 3.0).abs() <= 2.0);
            assert_eq!(s[1], x - s[0]);
        }
        y = s[2];
    }
}
#[test]
fn invalid_operands_exhaustion_and_cancellation_refuse_without_hidden_growth() {
    let p = program();
    let mut e = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    for (x, y, a, n, tag) in [
        (-32768, 0, 0, 1, "invalid_profile"),
        (0, 32768, 0, 1, "invalid_profile"),
        (0, 0, -1, 1, "invalid_profile"),
        (0, 0, 16384, 1, "invalid_profile"),
        (0, 0, 0, -1, "invalid_profile"),
        (0, 0, 0, 65537, "invalid_profile"),
        (0, 0, 0, 0, "budget_exhausted"),
    ] {
        let bytes = input(&p, x, y, a, n, false, false);
        assert_eq!(decode(e.evaluate(&bytes).unwrap()).0, tag);
        assert_eq!(e.evaluate(&bytes).unwrap(), p.evaluate(&bytes).unwrap());
    }
    let cancel = input(&p, 0, 0, 0, 1, false, true);
    assert_eq!(decode(e.evaluate(&cancel).unwrap()).0, "cancelled");
    let value = input(&p, 32767, -32767, 8192, 64, false, false);
    let expected = e.evaluate(&value).unwrap().to_vec();
    let capacity = e.output_capacity();
    let (same, count) = allocation_probe::observe(|| {
        let mut same = true;
        for _ in 0..100 {
            same &= e.evaluate(&value).unwrap() == expected;
        }
        same
    });
    assert!(same);
    assert_eq!((count.allocations, count.reallocations), (0, 0));
    assert_eq!(e.output_capacity(), capacity);
}
