use super::{
    validate_native_invariants, NativeBindingRefusal, PreparedNativeInvariantAdmission,
    PreparedNativeInvariantRefusal, PreparedNativeInvariantStorageLimits,
};
use crate::{check_syntax_document, parse_syntax_document, StartupCatalog};
use conduit_core::{StructuredFieldValue, StructuredInfoTypeShape, StructuredInfoValue};

#[test]
fn prepared_native_bank_preserves_order_and_law_refusals() {
    let checked = check_syntax_document(&parse_syntax_document("type Interval = {\n start: U32\n end: U32\n where .start <= .end\n where .end < 10\n}\n"), &StartupCatalog::new()).unwrap();
    let ty = &checked.native_types[0];
    let mut prepared =
        PreparedNativeInvariantAdmission::new(&ty.value_type, &ty.invariants, 2, 65536).unwrap();
    let encoded = ty
        .invariants
        .iter()
        .map(|law| law.canonical_bytes().unwrap())
        .collect::<Vec<_>>();
    let encoded_refs = encoded.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let limits = PreparedNativeInvariantStorageLimits {
        maximum_laws: 2,
        maximum_input_bytes: 65536,
        maximum_retained_bytes: usize::MAX,
        maximum_preparation_peak_bytes: usize::MAX,
    };
    let (mut bounded, receipt) =
        PreparedNativeInvariantAdmission::from_canonical_laws_with_storage_limits(
            &ty.value_type,
            &encoded_refs,
            limits,
        )
        .unwrap();
    assert_eq!(
        bounded.owned_heap_bytes(),
        receipt.retained_heap_bytes_bound
    );
    for reduced in [
        PreparedNativeInvariantStorageLimits {
            maximum_retained_bytes: receipt.retained_heap_bytes_bound - 1,
            ..limits
        },
        PreparedNativeInvariantStorageLimits {
            maximum_preparation_peak_bytes: receipt.preparation_peak_heap_bytes_bound - 1,
            ..limits
        },
        PreparedNativeInvariantStorageLimits {
            maximum_laws: 1,
            ..limits
        },
        PreparedNativeInvariantStorageLimits {
            maximum_input_bytes: 1,
            ..limits
        },
    ] {
        assert!(matches!(
            PreparedNativeInvariantAdmission::from_canonical_laws_with_storage_limits(
                &ty.value_type,
                &encoded_refs,
                reduced
            ),
            Err(PreparedNativeInvariantRefusal::Capacity)
        ));
    }
    assert!(matches!(
        PreparedNativeInvariantAdmission::from_canonical_laws_with_storage_limits(
            &ty.value_type,
            &[&[0]],
            limits
        ),
        Err(PreparedNativeInvariantRefusal::InvalidEncoding { index: 0, .. })
    ));
    let StructuredInfoTypeShape::Record { fields, .. } = ty.value_type.shape() else {
        panic!()
    };
    for (start, end, expected) in [
        (4u32, 4u32, Ok(())),
        (
            5,
            4,
            Err(NativeBindingRefusal::ViolatedInvariant { index: 0 }),
        ),
        (
            4,
            10,
            Err(NativeBindingRefusal::ViolatedInvariant { index: 1 }),
        ),
        (
            11,
            10,
            Err(NativeBindingRefusal::ViolatedInvariant { index: 0 }),
        ),
    ] {
        let values = fields
            .iter()
            .map(|field| {
                let value = if field.name() == "start" { start } else { end };
                StructuredFieldValue::new(
                    field.name(),
                    super::primitive_into_structured(field.value_type().clone(), &value).unwrap(),
                )
                .unwrap()
            })
            .collect();
        let value = StructuredInfoValue::record(ty.value_type.clone(), values).unwrap();
        assert_eq!(validate_native_invariants(&value, &ty.invariants), expected);
        assert_eq!(
            bounded.validate(&value.canonical_bytes().unwrap()),
            expected
        );
        assert_eq!(
            prepared.validate(&value.canonical_bytes().unwrap()),
            expected
        );
    }
    assert!(matches!(
        PreparedNativeInvariantAdmission::new(&ty.value_type, &ty.invariants, 1, 65536),
        Err(PreparedNativeInvariantRefusal::Capacity)
    ));
    assert!(matches!(
        PreparedNativeInvariantAdmission::new(&ty.value_type, &ty.invariants, 2, 1),
        Err(PreparedNativeInvariantRefusal::Capacity)
    ));
    assert!(matches!(
        prepared.validate(&[0]),
        Err(NativeBindingRefusal::InvalidInvariant(
            crate::PortableExpressionEvaluationRefusal::InvalidInput
        ))
    ));
}

#[test]
fn prepared_native_bank_checks_nested_members_and_foreign_exact_types() {
    let source = "type Inner = {\n value: U32\n}\ntype Outer = {\n child: Inner\n where .child.value < 10\n}\ntype Foreign = {\n different: U32\n}\n";
    let checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    let outer = checked
        .native_types
        .iter()
        .find(|ty| ty.invariants.len() == 1)
        .unwrap();
    let mut prepared =
        PreparedNativeInvariantAdmission::new(&outer.value_type, &outer.invariants, 1, 65536)
            .unwrap();
    let encoded = outer.invariants[0].canonical_bytes().unwrap();
    let (mut bounded, _) =
        PreparedNativeInvariantAdmission::from_canonical_laws_with_storage_limits(
            &outer.value_type,
            &[&encoded],
            PreparedNativeInvariantStorageLimits {
                maximum_laws: 1,
                maximum_input_bytes: 65536,
                maximum_retained_bytes: usize::MAX,
                maximum_preparation_peak_bytes: usize::MAX,
            },
        )
        .unwrap();
    let StructuredInfoTypeShape::Record { fields, .. } = outer.value_type.shape() else {
        panic!()
    };
    let inner_type = fields[0].value_type();
    let StructuredInfoTypeShape::Record {
        fields: inner_fields,
        ..
    } = inner_type.shape()
    else {
        panic!()
    };
    for number in [3u32, 10] {
        let leaf = super::primitive_into_structured(inner_fields[0].value_type().clone(), &number)
            .unwrap();
        let inner = StructuredInfoValue::record(
            inner_type.clone(),
            vec![StructuredFieldValue::new(inner_fields[0].name(), leaf).unwrap()],
        )
        .unwrap();
        let value = StructuredInfoValue::record(
            outer.value_type.clone(),
            vec![StructuredFieldValue::new(fields[0].name(), inner).unwrap()],
        )
        .unwrap();
        assert_eq!(
            prepared.validate(&value.canonical_bytes().unwrap()),
            validate_native_invariants(&value, &outer.invariants)
        );
        assert_eq!(
            bounded.validate(&value.canonical_bytes().unwrap()),
            validate_native_invariants(&value, &outer.invariants)
        );
    }
    let foreign = checked.native_types.iter().find(|ty| matches!(ty.value_type.shape(), StructuredInfoTypeShape::Record { fields, .. } if fields[0].name() == "different")).unwrap();
    let StructuredInfoTypeShape::Record { fields, .. } = foreign.value_type.shape() else {
        panic!()
    };
    let value = StructuredInfoValue::record(
        foreign.value_type.clone(),
        vec![StructuredFieldValue::new(
            "different",
            super::primitive_into_structured(fields[0].value_type().clone(), &3u32).unwrap(),
        )
        .unwrap()],
    )
    .unwrap();
    assert_eq!(
        prepared.validate(&value.canonical_bytes().unwrap()),
        validate_native_invariants(&value, &outer.invariants)
    );
    assert_eq!(
        bounded.validate(&value.canonical_bytes().unwrap()),
        validate_native_invariants(&value, &outer.invariants)
    );
    assert!(matches!(
        prepared.validate(&value.canonical_bytes().unwrap()),
        Err(NativeBindingRefusal::InvalidInvariant(
            crate::PortableExpressionEvaluationRefusal::InvalidInput
        ))
    ));
}

/// Diagnostic readiness audit of the exact authored Language build closure.
/// It does not run a model or claim generated constructors use this bank.
#[test]
#[ignore = "checks the complete Language authored Source closure"]
fn actual_language_complete_native_bank_readiness() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../semantics/language");
    let build = std::fs::read_to_string(root.join("build.rs")).unwrap();
    let paths: Vec<_> = build
        .lines()
        .filter_map(|line| {
            line.trim()
                .strip_prefix("include_str!(\"")
                .and_then(|s| s.strip_suffix("\"),"))
        })
        .collect();
    assert!(!paths.is_empty());
    let source = paths
        .iter()
        .map(|path| std::fs::read_to_string(root.join(path)).unwrap())
        .collect::<Vec<_>>()
        .join("\n");
    let start = std::time::Instant::now();
    let checked =
        check_syntax_document(&parse_syntax_document(&source), &StartupCatalog::new()).unwrap();
    eprintln!(
        "exact_language_closure_files={} check_seconds={:.3}",
        paths.len(),
        start.elapsed().as_secs_f64()
    );
    let mut inspected = 0;
    for ty in &checked.native_types {
        if !matches!(
            ty.name.as_str(),
            "LanguageParserIndependentProtectedAdmission"
                | "LanguageParserStableDependencyAdmission"
                | "LanguageParserStableLexicalFact"
                | "LanguageParserSessionStableLexicalFact"
                | "LanguageParserSessionSnapshot"
                | "LanguageParserCheckedHypothesis"
                | "LanguageParserSessionCheckedHypothesis"
                | "LanguageParserJointStableFact"
                | "LanguageParserJointRuntimeHypothesis"
                | "LanguageParserJointSnapshot"
                | "LanguageParserJointStableLexicalFact"
        ) {
            continue;
        }
        inspected += 1;
        eprintln!("native_bank={} invariants={}", ty.name, ty.invariants.len());
        for (index, law) in ty.invariants.iter().enumerate() {
            let began = std::time::Instant::now();
            let result = crate::PreparedPortableExpressionEvaluator::new(law);
            eprintln!(
                "law={} prepared={} elapsed_us={} refusal={:?}",
                index,
                result.is_ok(),
                began.elapsed().as_micros(),
                result.err()
            );
        }
        let bank = PreparedNativeInvariantAdmission::new(
            &ty.value_type,
            &ty.invariants,
            ty.invariants.len(),
            usize::MAX,
        );
        eprintln!("complete_bank={} refusal={:?}", bank.is_ok(), bank.err());
    }
    assert!(inspected >= 2);
}

#[test]
fn bounded_native_bank_refuses_every_unsupported_law_at_its_original_index() {
    use crate::{
        BinaryOperator, PortableExpressionNode as Node, PortableExpressionOperation as Operation,
        PortableExpressionProgram as Program,
    };
    use conduit_core::{StructuredInfoType, StructuredVariantCase};
    let checked = check_syntax_document(
        &parse_syntax_document("type Item = {\n value: U32\n where .value < 10\n}\n"),
        &StartupCatalog::new(),
    )
    .unwrap();
    let ty = &checked.native_types[0];
    let unit = StructuredInfoType::leaf(conduit_core::kind_id(conduit_core::UNIT_INFO_ID)).unwrap();
    let variant = StructuredInfoType::variant(
        conduit_core::kind_id("fixture/bounded-constant"),
        vec![StructuredVariantCase::new("Only", unit).unwrap()],
    )
    .unwrap();
    let boolean =
        StructuredInfoType::leaf(conduit_core::kind_id(conduit_core::BOOL_INFO_ID)).unwrap();
    let constant = Node {
        value_type: variant,
        operation: Operation::Literal("Only".into()),
    };
    let law = Program {
        input_type: ty.value_type.clone(),
        output_type: boolean.clone(),
        root: Node {
            value_type: boolean,
            operation: Operation::Binary {
                operator: BinaryOperator::Equal,
                proven: false,
                left: alloc::boxed::Box::new(constant.clone()),
                right: alloc::boxed::Box::new(constant),
            },
        },
    };
    // This complete law is supported by the ordinary preparer; the quota entrance
    // refuses its unaccounted structured-literal temporary, never drops the law.
    crate::PreparedPortableExpressionEvaluator::new(&law).unwrap();
    let first = ty.invariants[0].canonical_bytes().unwrap();
    let second = law.canonical_bytes().unwrap();
    let limits = PreparedNativeInvariantStorageLimits {
        maximum_laws: 2,
        maximum_input_bytes: usize::MAX,
        maximum_retained_bytes: usize::MAX,
        maximum_preparation_peak_bytes: usize::MAX,
    };
    assert!(matches!(
        PreparedNativeInvariantAdmission::from_canonical_laws_with_storage_limits(
            &ty.value_type,
            &[&first, &second],
            limits
        ),
        Err(PreparedNativeInvariantRefusal::UnsupportedTemporaryLaw { index: 1 })
    ));
    let foreign = StructuredInfoType::leaf(conduit_core::kind_id("value/u64")).unwrap();
    assert!(matches!(
        PreparedNativeInvariantAdmission::from_canonical_laws_with_storage_limits(
            &foreign,
            &[&first],
            limits
        ),
        Err(PreparedNativeInvariantRefusal::InvalidLaw {
            index: 0,
            refusal: crate::PortableExpressionEvaluationRefusal::InvalidProgram
        })
    ));
}
