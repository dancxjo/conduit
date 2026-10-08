//! Capacity accounting is checked against actual requested live allocations.
use conduit_core::{
    ConfigurationValue, KindId, StructuredFieldType, StructuredInfoType, StructuredVariantCase,
};
use conduit_plot::rust_binding::{
    PreparedNativeInvariantAdmission, PreparedNativeInvariantStorageLimits,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    BinaryOperator, PortableExpressionEvaluationRefusal, PortableExpressionNode,
    PortableExpressionOperation, PortableExpressionProgram, PortableExpressionProgramRefusal,
    PortableExpressionProjection, PreparedExpressionStorageRefusal,
    PreparedPortableExpressionEvaluator, ProfileCatalog, StartupCatalog, UnaryOperator,
};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicIsize, Ordering};
static LIVE: AtomicIsize = AtomicIsize::new(0);
static PEAK: AtomicIsize = AtomicIsize::new(0);
struct Probe;
unsafe impl GlobalAlloc for Probe {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = System.alloc(layout);
        if !pointer.is_null() {
            let live =
                LIVE.fetch_add(layout.size() as isize, Ordering::SeqCst) + layout.size() as isize;
            PEAK.fetch_max(live, Ordering::SeqCst);
        }
        pointer
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size() as isize, Ordering::SeqCst);
        System.dealloc(pointer, layout);
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let result = System.realloc(pointer, layout, new_size);
        if !result.is_null() {
            let live = LIVE.fetch_add(new_size as isize - layout.size() as isize, Ordering::SeqCst)
                + new_size as isize
                - layout.size() as isize;
            PEAK.fetch_max(live, Ordering::SeqCst);
        }
        result
    }
}
#[global_allocator]
static ALLOCATOR: Probe = Probe;
fn live() -> isize {
    LIVE.load(Ordering::SeqCst)
}
#[test]
fn capacity_slack_and_recursive_prepared_storage_bound_actual_live_allocations() {
    let before = live();
    let mut schema = String::with_capacity(128);
    schema.push_str("fixture/storage-record");
    let mut name = String::with_capacity(256);
    name.push_str("value");
    let mut fields = Vec::with_capacity(32);
    fields.push(
        StructuredFieldType::new(
            name,
            StructuredInfoType::leaf(KindId::from("value/u64")).unwrap(),
        )
        .unwrap(),
    );
    let ty = StructuredInfoType::record(KindId(schema), fields).unwrap();
    assert_eq!(ty.owned_heap_bytes(), (live() - before) as usize);
    assert!(ty.owned_heap_bytes() > 128 + 256);
    assert_eq!(
        ty.canonical_byte_length().unwrap(),
        ty.canonical_bytes().unwrap().len()
    );
    drop(ty);
    assert_eq!(live(), before);

    let primitive = StructuredInfoType::leaf(KindId::from("value/u64")).unwrap();
    let nominal =
        StructuredInfoType::nominal(KindId::from("fixture/nominal-count"), primitive.clone())
            .unwrap();
    let malformed = PortableExpressionProgram {
        input_type: primitive.clone(),
        output_type: nominal,
        root: PortableExpressionNode {
            value_type: primitive,
            operation: PortableExpressionOperation::Input,
        },
    };
    assert_eq!(
        malformed.evaluate(&7u64.to_le_bytes()),
        Err(PortableExpressionEvaluationRefusal::InvalidProgram)
    );
    assert!(matches!(
        PreparedPortableExpressionEvaluator::new(&malformed),
        Err(PortableExpressionEvaluationRefusal::InvalidProgram)
    ));
    assert!(matches!(
        PreparedPortableExpressionEvaluator::new_with_storage_limits(
            &malformed,
            usize::MAX,
            usize::MAX,
            usize::MAX
        ),
        Err(PreparedExpressionStorageRefusal::Expression(
            PortableExpressionEvaluationRefusal::InvalidProgram
        ))
    ));
    drop(malformed);

    // Decode every operation shape, including capacity growth and boxed branches.
    // These supplied ASTs test representation accounting, not expression meaning.
    let scalar = StructuredInfoType::leaf(KindId::from("value/u64")).unwrap();
    let child = PortableExpressionNode {
        value_type: scalar.clone(),
        operation: PortableExpressionOperation::Input,
    };
    let operations = vec![
        PortableExpressionOperation::Input,
        PortableExpressionOperation::Literal("123".into()),
        PortableExpressionOperation::Projection {
            value: Box::new(child.clone()),
            member: PortableExpressionProjection::Field("member".into()),
        },
        PortableExpressionOperation::Unary {
            operator: UnaryOperator::Negate,
            operand: Box::new(child.clone()),
        },
        PortableExpressionOperation::Binary {
            operator: BinaryOperator::Add,
            proven: false,
            left: Box::new(child.clone()),
            right: Box::new(child.clone()),
        },
        PortableExpressionOperation::Conditional {
            condition: Box::new(child.clone()),
            when_true: Box::new(child.clone()),
            when_false: Box::new(child.clone()),
        },
        PortableExpressionOperation::Tuple(vec![child.clone(); 17]),
        PortableExpressionOperation::Record(vec![
            ("first".into(), child.clone()),
            ("second".into(), child.clone()),
        ]),
        PortableExpressionOperation::Collection(vec![child.clone(); 17]),
        PortableExpressionOperation::Variant {
            tag: "Only".into(),
            payload: Box::new(child.clone()),
        },
        PortableExpressionOperation::SemanticCall {
            kind: "fixture/decode-only".into(),
            arguments: vec![child; 17],
        },
    ];
    for operation in operations {
        let program = PortableExpressionProgram {
            input_type: scalar.clone(),
            output_type: scalar.clone(),
            root: PortableExpressionNode {
                value_type: scalar.clone(),
                operation,
            },
        };
        let encoded = program.canonical_bytes().unwrap();
        let bound = PortableExpressionProgram::canonical_decode_storage_bound(&encoded).unwrap();
        let before = live();
        PEAK.store(before, Ordering::SeqCst);
        let decoded =
            PortableExpressionProgram::from_canonical_bytes_with_storage_limit(&encoded, bound)
                .unwrap();
        assert_eq!(decoded, program);
        assert!(bound >= (PEAK.load(Ordering::SeqCst) - before) as usize);
        drop(decoded);
        assert_eq!(live(), before);
    }

    let unit = StructuredInfoType::leaf(KindId::from("value/unit")).unwrap();
    let variant = StructuredInfoType::variant(
        KindId::from("fixture/choice"),
        vec![StructuredVariantCase::new("Only", unit.clone()).unwrap()],
    )
    .unwrap();
    let literal = PortableExpressionProgram {
        input_type: unit,
        output_type: variant.clone(),
        root: PortableExpressionNode {
            value_type: variant,
            operation: PortableExpressionOperation::Literal("Only".into()),
        },
    };
    assert!(literal.evaluate(&[]).is_ok());
    assert!(PreparedPortableExpressionEvaluator::new(&literal).is_ok());
    assert!(matches!(
        PreparedPortableExpressionEvaluator::new_with_storage_limits(
            &literal,
            usize::MAX,
            usize::MAX,
            usize::MAX
        ),
        Err(PreparedExpressionStorageRefusal::UnsupportedTemporaryOperation)
    ));
    drop(literal);

    let source = "type Cell = {\n number: U64\n}\ntype Request = {\n cells: sequence Cell <= 4\n index: U64\n}\nplot read (\n value: Request >> result: U64\n) = (sequence/at(.cells, .index).number)\nplot equal (\n value: Request >> result: Boolean\n) = (.cells == .cells)\nplot length (\n value: Request >> result: U64\n) = (sequence/length(.cells))\nplot conditional (\n value: Request >> result: U64\n) = (.index + 1)\n";
    let checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    for entry in ["read", "equal", "length", "conditional"] {
        let expanded = expand_canonical_plot_for_authoring(&checked, entry, &ProfileCatalog::new())
            .unwrap()
            .expanded;
        let ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value else {
            panic!()
        };
        let before_program = live();
        let law = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
        assert!(law.owned_heap_bytes() >= (live() - before_program) as usize);
        assert_eq!(
            law.input_type.canonical_byte_length().unwrap(),
            law.input_type.canonical_bytes().unwrap().len()
        );
        assert_eq!(
            law.output_type.canonical_byte_length().unwrap(),
            law.output_type.canonical_bytes().unwrap().len()
        );
        let encoded_program = law.canonical_bytes().unwrap();
        let decode_bound =
            PortableExpressionProgram::canonical_decode_storage_bound(&encoded_program).unwrap();
        let before_decode = live();
        PEAK.store(before_decode, Ordering::SeqCst);
        let decoded = PortableExpressionProgram::from_canonical_bytes_with_storage_limit(
            &encoded_program,
            decode_bound,
        )
        .unwrap();
        assert_eq!(decoded, law);
        assert!(
            decode_bound >= (PEAK.load(Ordering::SeqCst) - before_decode) as usize,
            "{entry}: decoding peak exceeds structural bound"
        );
        drop(decoded);
        assert_eq!(live(), before_decode);
        assert_eq!(
            PortableExpressionProgram::from_canonical_bytes_with_storage_limit(
                &encoded_program,
                decode_bound - 1
            ),
            Err(PortableExpressionProgramRefusal::TooLarge)
        );
        assert_eq!(live(), before_decode);
        for length in 0..encoded_program.len().min(128) {
            let partial = &encoded_program[..length];
            assert_eq!(
                PortableExpressionProgram::from_canonical_bytes_with_storage_limit(
                    partial,
                    usize::MAX
                ),
                PortableExpressionProgram::from_canonical_bytes(partial)
            );
            assert_eq!(live(), before_decode);
        }
        let encoded_type = law.input_type.canonical_bytes().unwrap();
        let type_bound = StructuredInfoType::canonical_decode_storage_bound(&encoded_type).unwrap();
        let before_type = live();
        PEAK.store(before_type, Ordering::SeqCst);
        let decoded_type = StructuredInfoType::from_canonical_bytes(&encoded_type).unwrap();
        assert!(type_bound >= (PEAK.load(Ordering::SeqCst) - before_type) as usize);
        drop(decoded_type);
        assert_eq!(live(), before_type);
        for length in 0..encoded_type.len() {
            assert_eq!(
                StructuredInfoType::canonical_decode_storage_bound(&encoded_type[..length])
                    .unwrap_err(),
                StructuredInfoType::from_canonical_bytes(&encoded_type[..length]).unwrap_err()
            );
            assert_eq!(live(), before_type);
        }
        if entry == "equal" {
            let before = live();
            PEAK.store(before, Ordering::SeqCst);
            let limits = PreparedNativeInvariantStorageLimits {
                maximum_laws: 2,
                maximum_input_bytes: usize::MAX,
                maximum_retained_bytes: usize::MAX,
                maximum_preparation_peak_bytes: usize::MAX,
            };
            let (bank, receipt) =
                PreparedNativeInvariantAdmission::from_canonical_laws_with_storage_limits(
                    &law.input_type,
                    &[&encoded_program, &encoded_program],
                    limits,
                )
                .unwrap();
            assert!(receipt.retained_heap_bytes_bound >= (live() - before) as usize);
            assert!(
                receipt.preparation_peak_heap_bytes_bound
                    >= (PEAK.load(Ordering::SeqCst) - before) as usize
            );
            drop(bank);
            assert_eq!(live(), before);
            let refused = PreparedNativeInvariantStorageLimits {
                maximum_preparation_peak_bytes: receipt.preparation_peak_heap_bytes_bound - 1,
                ..limits
            };
            assert!(
                PreparedNativeInvariantAdmission::from_canonical_laws_with_storage_limits(
                    &law.input_type,
                    &[&encoded_program, &encoded_program],
                    refused
                )
                .is_err()
            );
            assert_eq!(live(), before);
        }
        let before = live();
        let evaluator = PreparedPortableExpressionEvaluator::new(&law).unwrap();
        let actual = (live() - before) as usize;
        assert!(
            evaluator.owned_heap_bytes() >= actual,
            "{entry}: bound {} actual {actual}",
            evaluator.owned_heap_bytes()
        );
        drop(evaluator);
        assert_eq!(live(), before);
        PEAK.store(before, Ordering::SeqCst);
        let (bounded, receipt) = PreparedPortableExpressionEvaluator::new_with_storage_limits(
            &law,
            usize::MAX,
            usize::MAX,
            usize::MAX,
        )
        .unwrap();
        assert!(
            receipt.preparation_requested_bytes_bound
                >= (PEAK.load(Ordering::SeqCst) - before) as usize,
            "{entry}: peak exceeds charged bound"
        );
        assert!(receipt.retained_heap_bytes_bound >= (live() - before) as usize);
        drop(bounded);
        assert_eq!(live(), before);
        for (decoded, peak, retained) in [
            (
                receipt.decoded_program_heap_bytes - 1,
                usize::MAX,
                usize::MAX,
            ),
            (usize::MAX, receipt.decoded_program_heap_bytes, usize::MAX),
            (usize::MAX, usize::MAX, 0),
            (
                usize::MAX,
                receipt.decoded_program_heap_bytes + receipt.preparation_requested_bytes_bound - 1,
                usize::MAX,
            ),
        ] {
            assert!(matches!(
                PreparedPortableExpressionEvaluator::new_with_storage_limits(
                    &law, decoded, peak, retained
                ),
                Err(PreparedExpressionStorageRefusal::Capacity)
            ));
            assert_eq!(live(), before);
        }
        let mut malformed = law.clone();
        malformed.output_type = StructuredInfoType::leaf(KindId::from("value/text")).unwrap();
        assert!(PreparedPortableExpressionEvaluator::new(&malformed).is_err());
        assert!(matches!(
            PreparedPortableExpressionEvaluator::new_with_storage_limits(
                &malformed,
                usize::MAX,
                usize::MAX,
                usize::MAX
            ),
            Err(PreparedExpressionStorageRefusal::Expression(_))
        ));
    }
}
