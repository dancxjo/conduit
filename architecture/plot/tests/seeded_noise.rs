use conduit_core::{
    ConfigurationValue, StructuredFieldValue, StructuredInfoTypeShape, StructuredInfoValue,
    StructuredInfoValueShape,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, PreparedPortableExpressionEvaluator, ProfileCatalog, StartupCatalog,
};
const SOURCE: &str = include_str!("../../../semantics/audio/dsp/seeded-noise.conduit");
fn program() -> PortableExpressionProgram {
    let document = parse_syntax_document(SOURCE);
    assert_eq!(document.round_trip(), SOURCE);
    let checked = check_syntax_document(&document, &StartupCatalog::new()).unwrap();
    let expanded = expand_canonical_plot_for_authoring(
        &checked,
        "dsp/noise-step-candidate",
        &ProfileCatalog::new(),
    )
    .unwrap();
    assert_eq!(expanded.expanded.gears.len(), 1);
    let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!("program")
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}
fn input(
    program: &PortableExpressionProgram,
    word: i32,
    remaining: u32,
) -> Result<Vec<u8>, String> {
    let StructuredInfoTypeShape::Record { fields, .. } = program.input_type.shape() else {
        panic!("record")
    };
    let values = fields
        .iter()
        .map(|field| {
            let bytes = if field.name() == "word" {
                word.to_le_bytes().to_vec()
            } else {
                remaining.to_le_bytes().to_vec()
            };
            StructuredInfoValue::leaf(field.value_type().clone(), bytes)
                .and_then(|value| StructuredFieldValue::new(field.name(), value))
                .map_err(|e| format!("{e:?}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    StructuredInfoValue::record(program.input_type.clone(), values)
        .and_then(|v| v.canonical_bytes())
        .map_err(|e| format!("{e:?}"))
}
fn output(bytes: &[u8]) -> (i32, u32, i32) {
    let value = StructuredInfoValue::from_canonical_bytes(bytes).unwrap();
    let StructuredInfoValueShape::Variant { tag, payload } = value.shape() else {
        panic!("outcome")
    };
    assert_eq!(tag, "advanced");
    let StructuredInfoValueShape::Record(fields) = payload.shape() else {
        panic!("record")
    };
    let leaf = |name| {
        let field = fields.iter().find(|f| f.name() == name).unwrap();
        let StructuredInfoValueShape::Leaf(bytes) = field.value().shape() else {
            panic!("leaf")
        };
        <[u8; 4]>::try_from(bytes).unwrap()
    };
    (
        i32::from_le_bytes(leaf("word")),
        u32::from_le_bytes(leaf("remaining")),
        i32::from_le_bytes(leaf("sample")),
    )
}
#[test]
fn complete_admitted_period_matches_independent_reference_without_capacity_growth() {
    let program = program();
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    let capacity = evaluator.output_capacity();
    let mut word = 1i32;
    let mut seen = [false; 65536];
    for remaining in (1..=65536).rev() {
        let expected = ((u64::try_from(word).unwrap() * 25173 + 13849) & 0xffff) as i32;
        let (next, left, sample) = output(
            evaluator
                .evaluate(&input(&program, word, remaining).unwrap())
                .unwrap(),
        );
        assert_eq!(
            (next, left, sample),
            (expected, remaining - 1, expected - 32768)
        );
        assert!(!seen[next as usize]);
        seen[next as usize] = true;
        assert_eq!(evaluator.output_capacity(), capacity);
        word = next;
    }
    assert_eq!(word, 1);
    assert!(seen.iter().all(|v| *v));
}
#[test]
fn invalid_seed_and_work_profiles_refuse() {
    let program = program();
    for (word, remaining, expected) in [
        (-1, 8, "invalid_seed"),
        (65536, 8, "invalid_seed"),
        (1, 0, "exhausted"),
        (1, 65537, "invalid_profile"),
    ] {
        match input(&program, word, remaining) {
            Err(_) => {}
            Ok(bytes) => {
                let result =
                    StructuredInfoValue::from_canonical_bytes(&program.evaluate(&bytes).unwrap())
                        .unwrap();
                let StructuredInfoValueShape::Variant { tag, .. } = result.shape() else {
                    panic!("outcome")
                };
                assert_eq!(tag, expected);
            }
        }
    }
    assert!(program.evaluate(&[0; 3]).is_err());
}
