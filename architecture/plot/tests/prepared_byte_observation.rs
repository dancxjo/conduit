//! Packed bytes are indexed by actual extent, independent of collection framing.
#[path = "prepared_structured_payload/allocation.rs"]
mod allocation;
use conduit_core::{
    ConfigurationValue, StructuredFieldValue, StructuredInfoType, StructuredInfoTypeShape,
    StructuredInfoValue,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionEvaluationRefusal, PortableExpressionProgram,
    PreparedPortableExpressionEvaluator, ProfileCatalog, StartupCatalog,
};

const SOURCE: &str = "
type Request = {
 bytes: Bytes <= 2048B
 index: U64
}
type Result =
 octet U8
 | short
plot read (
 value: Request >> result: U8
) = (bytes/at(.bytes, .index))
plot length (
 value: Request >> result: U64
) = (bytes/length(.bytes))
plot guarded (
 value: Request >> result: Result
) = (.index < bytes/length(.bytes) ? octet(bytes/at(.bytes, .index)) : short(unit))
plot computed (
 value: Request >> result: U8
) = (bytes/at(.index == 0 ? .bytes : .bytes, .index))
";
fn program(entry: &str) -> PortableExpressionProgram {
    let syntax = parse_syntax_document(SOURCE);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let expanded = expand_canonical_plot_for_authoring(&checked, entry, &ProfileCatalog::new())
        .unwrap()
        .expanded;
    let ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value else {
        panic!("pure program")
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}
fn request(program: &PortableExpressionProgram, bytes: &[u8], index: u64) -> Vec<u8> {
    let StructuredInfoTypeShape::Record { fields, .. } = program.input_type.shape() else {
        panic!("record")
    };
    let ty = |name: &str| -> StructuredInfoType {
        fields
            .iter()
            .find(|field| field.name() == name)
            .unwrap()
            .value_type()
            .clone()
    };
    StructuredInfoValue::record(
        program.input_type.clone(),
        vec![
            StructuredFieldValue::new(
                "bytes",
                StructuredInfoValue::leaf(ty("bytes"), bytes.to_vec()).unwrap(),
            )
            .unwrap(),
            StructuredFieldValue::new(
                "index",
                StructuredInfoValue::leaf(ty("index"), index.to_le_bytes().to_vec()).unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}

#[test]
fn byte_observations_preserve_actual_extent_above_the_collection_ceiling() {
    let read = program("read");
    let length = program("length");
    let guarded = program("guarded");
    let computed = program("computed");
    let mut prepared = PreparedPortableExpressionEvaluator::new(&read).unwrap();
    let mut prepared_length = PreparedPortableExpressionEvaluator::new(&length).unwrap();
    let mut prepared_guard = PreparedPortableExpressionEvaluator::new(&guarded).unwrap();
    let mut prepared_computed = PreparedPortableExpressionEvaluator::new(&computed).unwrap();
    let mut cases = Vec::new();
    for extent in [0_usize, 1, 8, 1024, 1025, 2048] {
        let bytes: Vec<_> = (0..extent).map(|index| (index % 251) as u8).collect();
        for index in [
            0,
            extent.saturating_sub(1) as u64,
            extent as u64,
            extent as u64 + 1,
            u64::MAX,
        ] {
            let input = request(&read, &bytes, index);
            let expected = usize::try_from(index)
                .ok()
                .and_then(|i| bytes.get(i))
                .copied();
            let observed = read.evaluate(&input);
            match expected {
                Some(octet) => assert_eq!(observed.unwrap(), [octet]),
                None => assert_eq!(
                    observed.unwrap_err(),
                    PortableExpressionEvaluationRefusal::InvalidInput
                ),
            }
            assert_eq!(
                length.evaluate(&input).unwrap(),
                (extent as u64).to_le_bytes()
            );
            let guarded_bytes = guarded.evaluate(&input).unwrap();
            cases.push((input, expected, guarded_bytes, extent));
        }
    }
    let capacities = [
        prepared.output_capacity(),
        prepared_length.output_capacity(),
        prepared_guard.output_capacity(),
        prepared_computed.output_capacity(),
    ];
    let allocations = allocation::allocations(|| {
        for _ in 0..256 {
            for (input, expected, guarded_bytes, extent) in &cases {
                match expected {
                    Some(octet) => {
                        assert_eq!(prepared.evaluate(input).unwrap(), [*octet]);
                        assert_eq!(prepared_computed.evaluate(input).unwrap(), [*octet]);
                    }
                    None => {
                        assert_eq!(
                            prepared.evaluate(input).unwrap_err(),
                            PortableExpressionEvaluationRefusal::InvalidInput
                        );
                        assert_eq!(
                            prepared_computed.evaluate(input).unwrap_err(),
                            PortableExpressionEvaluationRefusal::InvalidInput
                        );
                    }
                }
                assert_eq!(
                    prepared_length.evaluate(input).unwrap(),
                    (*extent as u64).to_le_bytes()
                );
                assert_eq!(prepared_guard.evaluate(input).unwrap(), guarded_bytes);
            }
        }
    });
    assert_eq!(allocations, 0);
    assert_eq!(
        capacities,
        [
            prepared.output_capacity(),
            prepared_length.output_capacity(),
            prepared_guard.output_capacity(),
            prepared_computed.output_capacity()
        ]
    );
}

#[test]
fn byte_calls_require_exact_source_index_and_arity() {
    for source in [
        "plot bad (\n v: U64 >> n: U64\n) = (bytes/length(.))",
        "plot bad (\n v: Bytes >> n: U8\n) = (bytes/at(.))",
        "plot bad (\n v: Bytes >> n: U8\n) = (bytes/at(., true))",
        "plot bad (\n v: Bytes >> n: U8\n) = (bytes/at(., 0, 1))",
        "plot bad (\n v: Bytes >> n: U8\n) = (bytes/length(.))",
    ] {
        let syntax = parse_syntax_document(source);
        assert!(
            syntax.diagnostics.is_empty(),
            "invalid negative fixture: {:?}",
            syntax.diagnostics
        );
        let refused = match check_syntax_document(&syntax, &StartupCatalog::new()) {
            Err(_) => true,
            Ok(checked) => {
                expand_canonical_plot_for_authoring(&checked, "bad", &ProfileCatalog::new())
                    .is_err()
            }
        };
        assert!(refused, "accepted {source}");
    }
}

#[test]
fn malformed_programs_are_refused_before_play_and_bad_frames_do_not_poison_reuse() {
    use conduit_plot::PortableExpressionOperation;
    for mutation in 0..3 {
        let mut forged = program("read");
        let PortableExpressionOperation::SemanticCall { arguments, .. } =
            &mut forged.root.operation
        else {
            panic!("byte call")
        };
        match mutation {
            0 => {
                arguments.pop();
            }
            1 => {
                arguments[0].value_type =
                    StructuredInfoType::leaf(conduit_core::kind_id("value/u64")).unwrap();
            }
            _ => {
                arguments[1].value_type =
                    StructuredInfoType::leaf(conduit_core::kind_id("value/u8")).unwrap();
            }
        }
        assert!(matches!(
            PreparedPortableExpressionEvaluator::new(&forged),
            Err(PortableExpressionEvaluationRefusal::InvalidProgram)
        ));
    }
    let program = program("read");
    let valid = request(&program, &[0, 255], 1);
    let mut prepared = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    let capacity = prepared.output_capacity();
    assert_eq!(
        prepared.evaluate(&valid[..valid.len() - 1]).unwrap_err(),
        PortableExpressionEvaluationRefusal::InvalidInput
    );
    assert_eq!(prepared.evaluate(&valid).unwrap(), [255]);
    assert_eq!(prepared.output_capacity(), capacity);
}
