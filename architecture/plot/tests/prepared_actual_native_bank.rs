//! Exact retained generated-law readiness, without model inference.
use conduit_plot::rust_binding::PreparedNativeInvariantAdmission;
use conduit_plot::{
    PortableExpressionNode, PortableExpressionOperation, PortableExpressionProgram,
    PreparedPortableExpressionEvaluator,
};

#[test]
#[ignore = "requires an immutable generated-law snapshot TSV"]
fn retained_actual_native_law_bank_readiness() {
    let path = std::env::var("CONDUIT_ACTUAL_NATIVE_LAW_BANKS").unwrap();
    let manifest = std::fs::read_to_string(path).unwrap();
    let mut banks = std::collections::BTreeMap::<String, (usize, usize)>::new();
    let mut complete = std::collections::BTreeMap::<String, Vec<PortableExpressionProgram>>::new();
    for row in manifest.lines() {
        let columns: Vec<_> = row.split('\t').collect();
        assert_eq!(columns.len(), 3);
        let bytes = std::fs::read(columns[2]).unwrap();
        let start = std::time::Instant::now();
        let law = PortableExpressionProgram::from_canonical_bytes(&bytes).unwrap();
        let decode_us = start.elapsed().as_micros();
        let start = std::time::Instant::now();
        let prepared = PreparedPortableExpressionEvaluator::new(&law);
        if prepared.is_err() {
            let mut calls = std::collections::BTreeSet::new();
            inspect(&law.root, &mut calls);
            eprintln!("unsupported_operations={:?}", calls);
        }
        let stats = banks.entry(columns[0].into()).or_default();
        stats.0 += 1;
        stats.1 += usize::from(prepared.is_ok());
        eprintln!(
            "bank={} law={} bytes={} decode_us={} prepare_us={} result={:?}",
            columns[0],
            columns[1],
            bytes.len(),
            decode_us,
            start.elapsed().as_micros(),
            prepared.err()
        );
        complete.entry(columns[0].into()).or_default().push(law);
    }
    for (name, laws) in complete {
        for (index, law) in laws.iter().enumerate() {
            eprintln!(
                "bank_shape={name} law={index} input_matches_first={} output={:?}",
                law.input_type == laws[0].input_type,
                law.output_type.shape()
            );
        }
        let bank = PreparedNativeInvariantAdmission::new(
            &laws[0].input_type,
            &laws,
            laws.len(),
            usize::MAX,
        );
        eprintln!(
            "exact_complete_admission={name} ready={} refusal={:?}",
            bank.is_ok(),
            bank.err()
        );
    }
    assert!(!banks.is_empty());
    for (name, (total, accepted)) in banks {
        eprintln!(
            "complete_bank={name} total={total} prepared={accepted} ready={}",
            total == accepted
        );
    }
}

fn inspect(node: &PortableExpressionNode, calls: &mut std::collections::BTreeSet<String>) {
    use PortableExpressionOperation as O;
    match &node.operation {
        O::SemanticCall { kind, arguments } => {
            calls.insert(format!("call:{kind}"));
            for child in arguments {
                inspect(child, calls);
            }
        }
        O::Binary {
            operator,
            left,
            right,
            ..
        } => {
            if !matches!(
                left.value_type.shape(),
                conduit_core::StructuredInfoTypeShape::Leaf(_)
            ) {
                calls.insert(format!("nonleaf_binary:{operator:?}"));
            }
            inspect(left, calls);
            inspect(right, calls);
        }
        O::Unary { operand, .. } => inspect(operand, calls),
        O::Projection { value, .. } => inspect(value, calls),
        O::Conditional {
            condition,
            when_true,
            when_false,
        } => {
            inspect(condition, calls);
            inspect(when_true, calls);
            inspect(when_false, calls);
        }
        O::Tuple(values) | O::Collection(values) => {
            for child in values {
                inspect(child, calls);
            }
        }
        O::Record(values) => {
            for (_, child) in values {
                inspect(child, calls);
            }
        }
        O::Variant { payload, .. } => inspect(payload, calls),
        O::Input | O::Literal(_) => (),
    }
}

#[test]
#[ignore = "requires original actual fact and its immutable complete generated law bank"]
fn retained_actual_stable_lexical_complete_bank_parity() {
    let manifest =
        std::fs::read_to_string(std::env::var("CONDUIT_ACTUAL_NATIVE_LAW_BANKS").unwrap()).unwrap();
    let laws = manifest
        .lines()
        .filter_map(|row| {
            let columns: Vec<_> = row.split('\t').collect();
            (columns[0] == "LanguageParserWindow8StableLexicalFact").then(|| {
                PortableExpressionProgram::from_canonical_bytes(&std::fs::read(columns[2]).unwrap())
                    .unwrap()
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(laws.len(), 8);
    let bytes =
        std::fs::read(std::env::var("CONDUIT_ACTUAL_STABLE_LEXICAL_FACT").unwrap()).unwrap();
    let value = conduit_core::StructuredInfoValue::from_canonical_bytes(&bytes).unwrap();
    assert_eq!(
        value.value_type(),
        &laws[0].input_type,
        "exact original Type; no nominal substitution"
    );
    let began = std::time::Instant::now();
    let expected = reference_validate(&value, &laws);
    let allocating_us = began.elapsed().as_micros();
    assert_eq!(expected, Ok(()));
    let began = std::time::Instant::now();
    let mut bank =
        PreparedNativeInvariantAdmission::new(value.value_type(), &laws, laws.len(), usize::MAX)
            .unwrap();
    let preparation_us = began.elapsed().as_micros();
    let began = std::time::Instant::now();
    assert_eq!(bank.validate(&bytes), expected);
    let prepared_us = began.elapsed().as_micros();
    let negative = replace_zero(
        &value,
        &[
            "query",
            "snapshot",
            "candidate3",
            "hypothesis",
            "choices",
            "1",
        ],
    );
    let negative_bytes = negative.canonical_bytes().unwrap();
    let negative_expected = reference_validate(&negative, &laws);
    assert_eq!(
        negative_expected,
        Err(conduit_plot::rust_binding::NativeBindingRefusal::ViolatedInvariant { index: 7 })
    );
    assert_eq!(bank.validate(&negative_bytes), negative_expected);
    assert_eq!(
        conduit_plot::rust_binding::validate_native_invariants(&negative, &laws),
        negative_expected
    );
    assert_eq!(
        value.canonical_bytes().unwrap(),
        bytes,
        "original actual receipt unchanged"
    );

    let began = std::time::Instant::now();
    assert_eq!(
        conduit_plot::rust_binding::validate_native_invariants(&value, &laws),
        expected
    );
    let constructor_validator_us = began.elapsed().as_micros();
    eprintln!(
        "actual_stablelex_bytes={} laws={} allocating_us={} preparation_us={} prepared_us={} constructor_validator_us={}",
        bytes.len(),
        laws.len(),
        allocating_us,
        preparation_us,
        prepared_us,
        constructor_validator_us
    );
}

fn reference_validate(
    value: &conduit_core::StructuredInfoValue,
    laws: &[PortableExpressionProgram],
) -> Result<(), conduit_plot::rust_binding::NativeBindingRefusal> {
    use conduit_plot::rust_binding::NativeBindingRefusal as R;
    let bytes = value.canonical_bytes().map_err(R::InvalidValue)?;
    for (index, law) in laws.iter().enumerate() {
        let result = law.evaluate(&bytes).map_err(R::InvalidInvariant)?;
        let accepted = conduit_core::InfoBool::decode(&result)
            .map(conduit_core::InfoBool::get)
            .map_err(|_| {
                R::InvalidInvariant(
                    conduit_plot::PortableExpressionEvaluationRefusal::InvalidProgram,
                )
            })?;
        if !accepted {
            return Err(R::ViolatedInvariant { index });
        }
    }
    Ok(())
}

// Deliberately invalid derived fixture; it never overwrites the actual receipt.
fn replace_zero(
    value: &conduit_core::StructuredInfoValue,
    path: &[&str],
) -> conduit_core::StructuredInfoValue {
    use conduit_core::{
        StructuredFieldValue, StructuredInfoValue as V, StructuredInfoValueShape as S,
    };
    if path.is_empty() {
        let S::Leaf(bytes) = value.shape() else {
            panic!("choice is primitive")
        };
        assert_ne!(bytes, &vec![0; bytes.len()]);
        return V::leaf(value.value_type().clone(), vec![0; bytes.len()]).unwrap();
    }
    match value.shape() {
        S::Record(fields) => V::record(
            value.value_type().clone(),
            fields
                .iter()
                .map(|field| {
                    StructuredFieldValue::new(
                        field.name(),
                        if field.name() == path[0] {
                            replace_zero(field.value(), &path[1..])
                        } else {
                            field.value().clone()
                        },
                    )
                    .unwrap()
                })
                .collect(),
        )
        .unwrap(),
        S::Collection(values) => {
            let index: usize = path[0].parse().unwrap();
            V::collection(
                value.value_type().clone(),
                values
                    .iter()
                    .enumerate()
                    .map(|(i, value)| {
                        if i == index {
                            replace_zero(value, &path[1..])
                        } else {
                            value.clone()
                        }
                    })
                    .collect(),
            )
            .unwrap()
        }
        _ => panic!("fixture path has no selected field"),
    }
}
