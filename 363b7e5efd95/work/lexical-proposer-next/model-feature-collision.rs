use conduit_language::*;
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_plot::{PortableExpressionProgram, PreparedPortableExpressionEvaluator};
fn relation(base: LanguageUniversalDependencyRelation) -> LanguageParserRelation {
    LanguageParserRelation::new(base, LanguageParserSubtype::new(String::new()).unwrap()).unwrap()
}
fn query(object: bool) -> LanguageParserWindow8RawFeatureQuery {
    let name = if object {
        "constructed-object"
    } else {
        "constructed-vocative"
    };
    let text = LanguageTextId::new(name.into()).unwrap();
    let revision = LanguageTextRevisionId::new(format!("{name}/r0")).unwrap();
    let basis = LanguageParserBasis::new(
        LanguageAnalysisRevisionId::new(format!("{name}/analysis")).unwrap(),
        revision.clone(),
        text.clone(),
    )
    .unwrap();
    let positions = if object {
        [10, 2, 15, 11, 12]
    } else {
        [10, 15, 12, 11, 12]
    };
    let codes = std::array::from_fn(|index| {
        LanguageParserWindow8RawCodes::new(
            if index < 5 {
                [positions[index], 17, 17, 17]
            } else {
                [17; 4]
            },
            u64::from(index < 5),
            index as u64,
        )
        .unwrap()
    });
    let projection = LanguageParserWindow8RawProjection::new(
        "constructed-observability-profile".into(),
        0,
        revision,
        text,
        5,
        codes,
    )
    .unwrap();
    let root = if object { 2 } else { 1 };
    let mut heads = [9; 9];
    heads[0] = root;
    heads[root as usize] = 8;
    heads[8] = 8;
    heads[if object { 1 } else { 2 }] = root;
    let default = relation(LanguageUniversalDependencyRelation::Dep);
    let mut relations = [
        default.clone(),
        default.clone(),
        default.clone(),
        default.clone(),
        default.clone(),
        default.clone(),
        default.clone(),
        default,
    ];
    relations[0] = relation(LanguageUniversalDependencyRelation::Nsubj);
    relations[root as usize] = relation(LanguageUniversalDependencyRelation::Root);
    relations[if object { 1 } else { 2 }] = relation(if object {
        LanguageUniversalDependencyRelation::Advmod
    } else {
        LanguageUniversalDependencyRelation::Punct
    });
    let [r0, r1, r2, r3, r4, r5, r6, r7] = relations;
    let state = LanguageParserWindow8RawState::new(
        basis.clone(),
        0,
        2,
        heads,
        r0,
        r1,
        r2,
        r3,
        r4,
        r5,
        r6,
        r7,
        [8, root, 8, 8, 8, 8, 8, 8, 8],
        5,
        3,
    )
    .unwrap();
    // Fixture witness proposal; every original generated Source ancestry/state
    // invariant is evaluated by the Reference constructors before use.
    let ancestry = std::array::from_fn(|index| {
        let mut path = [index as u64; 9];
        for hop in 1..9 {
            path[hop] = if path[hop - 1] < 8 {
                heads[path[hop - 1] as usize]
            } else {
                path[hop - 1]
            };
        }
        LanguageParserWindow8Ancestry::new(heads, path, index as u64).unwrap()
    });
    LanguageParserWindow8StateProof::new(ancestry, 1, state.clone()).unwrap();
    LanguageParserWindow8RawFeatureQuery::new([0; 8], basis, projection, state).unwrap()
}
fn program(name: &str) -> (PortableExpressionProgram, Vec<u8>) {
    let path = format!("target/debug/build/conduit-language-66b63170b7b5d838/out/{name}.hex");
    let hex = std::fs::read_to_string(path).unwrap();
    let bytes = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect::<Vec<_>>();
    (
        PortableExpressionProgram::from_canonical_bytes(&bytes).unwrap(),
        bytes,
    )
}
fn main() {
    let (context, context_bytes) = program("window8_feature_context");
    let (values, values_bytes) = program("window8_feature_values");
    let mut context_prepared = PreparedPortableExpressionEvaluator::new(&context).unwrap();
    let mut values_prepared = PreparedPortableExpressionEvaluator::new(&values).unwrap();
    let mut outputs = Vec::new();
    for object in [false, true] {
        let query = query(object);
        let input = query.clone().encode().unwrap();
        let reference_context = context.evaluate(&input).unwrap();
        let reference_output = values.evaluate(&reference_context).unwrap();
        let expected = reference_output;
        let actual_context = context_prepared.evaluate(&input).unwrap();
        let actual = values_prepared.evaluate(actual_context).unwrap().to_vec();
        assert_eq!(actual, expected);
        let native = LanguageParserWindow8RawModelFeatures::decode(&actual).unwrap();
        LanguageParserWindow8Features::new(native.clone()).unwrap();
        std::fs::write(
            format!(
                "work/lexical-proposer-next/collision-{}.bin",
                if object { "object" } else { "vocative" }
            ),
            &input,
        )
        .unwrap();
        println!(
            "constructed={} indices={:?}",
            if object {
                "They now record Travis."
            } else {
                "They record, Travis."
            },
            native.indices()
        );
        outputs.push(*native.indices());
    }
    assert_eq!(outputs[0], outputs[1]);
    println!("COLLISION: complete25-index integer categorical inputs identical; legal constructed states, not actual model trajectories; no model inference invoked");
    std::fs::write(
        "work/lexical-proposer-next/collision-context-program.bin",
        context_bytes,
    )
    .unwrap();
    std::fs::write(
        "work/lexical-proposer-next/collision-values-program.bin",
        values_bytes,
    )
    .unwrap();
}
