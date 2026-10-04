//! Checked bounded payloads use the ordinary pure-expression runtime.
#[path = "prepared_structured_payload/allocation.rs"]
mod allocation;
use conduit_core::{
    kind_id, ConfigurationValue, StructuredFieldValue, StructuredInfoType, StructuredInfoTypeShape,
    StructuredInfoValue,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionOperation, PortableExpressionProgram, PreparedPortableExpressionEvaluator,
    ProfileCatalog, StartupCatalog,
};

fn program(entry: &str) -> PortableExpressionProgram {
    let types = include_str!("../../../targets/conduitos/plots/usb/control-types.conduit");
    let checked_types =
        check_syntax_document(&parse_syntax_document(types), &StartupCatalog::new()).unwrap();
    let request = &checked_types
        .native_types
        .iter()
        .find(|ty| ty.name == "UsbControlRequest")
        .unwrap()
        .value_type;
    let StructuredInfoTypeShape::Record { fields, .. } = request.shape() else {
        panic!("record");
    };
    let bytes = fields
        .iter()
        .find(|field| field.name() == "output")
        .unwrap()
        .value_type();
    let mut startup = StartupCatalog::new();
    startup
        .insert_structured_type("test/request", request.clone())
        .unwrap();
    startup
        .insert_structured_type("test/bytes", bytes.clone())
        .unwrap();
    let source = "with test/request as Request\nwith test/bytes as Bytes\n\
        plot identity (\n value: Request >> result: Request\n) = (.)\n\
        plot payload (\n value: Request >> result: Bytes\n) = (.output)\n";
    let syntax = parse_syntax_document(source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let expanded = expand_canonical_plot_for_authoring(&checked, entry, &ProfileCatalog::new())
        .unwrap()
        .expanded;
    assert_eq!(expanded.gears.len(), 1);
    let ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value else {
        panic!("ordinary pure expression program");
    };
    let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
    assert_eq!(
        &program.input_type, request,
        "pure program must preserve the exact imported request Type"
    );
    program
}

fn value(program: &PortableExpressionProgram, length: usize) -> (Vec<u8>, Vec<u8>) {
    let StructuredInfoTypeShape::Record { fields, .. } = program.input_type.shape() else {
        panic!("request record");
    };
    let field = |name| {
        fields
            .iter()
            .find(|field| field.name() == name)
            .unwrap()
            .value_type()
            .clone()
    };
    let octet = StructuredInfoType::leaf(kind_id("value/u8")).unwrap();
    let payload = StructuredInfoValue::sequence(
        field("output"),
        (0..length)
            .map(|index| StructuredInfoValue::leaf(octet.clone(), vec![index as u8]).unwrap())
            .collect(),
    )
    .unwrap();
    let encoded_payload = payload.canonical_bytes().unwrap();
    let request = StructuredInfoValue::record(
        program.input_type.clone(),
        vec![
            StructuredFieldValue::new(
                "setup",
                StructuredInfoValue::leaf(
                    field("setup"),
                    0x0102_0304_0506_0708_u64.to_le_bytes().to_vec(),
                )
                .unwrap(),
            )
            .unwrap(),
            StructuredFieldValue::new("output", payload).unwrap(),
        ],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    (request, encoded_payload)
}

#[test]
fn checked_source_preserves_complete_request_and_selects_every_bounded_payload_length() {
    for entry in ["identity", "payload"] {
        let p = program(entry);
        let mut prepared = PreparedPortableExpressionEvaluator::new(&p).unwrap();
        let capacity = prepared.output_capacity();
        for length in 0..=256 {
            let (request, payload) = value(&p, length);
            let expected = if entry == "identity" {
                &request
            } else {
                &payload
            };
            assert_eq!(p.evaluate(&request).unwrap(), *expected);
            assert_eq!(prepared.evaluate(&request).unwrap(), expected);
            assert_eq!(prepared.output_capacity(), capacity);
        }
    }
}

#[test]
fn malformed_payload_and_substituted_type_remain_refusals_after_reuse() {
    let p = program("payload");
    let mut prepared = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    let (request, payload) = value(&p, 256);
    let capacity = prepared.output_capacity();
    let allocations = allocation::allocations(|| {
        for _ in 0..10_000 {
            assert_eq!(prepared.evaluate(&request).unwrap(), payload);
        }
    });
    assert_eq!(
        allocations, 0,
        "prepared payload projection must not allocate during Play"
    );
    assert_eq!(prepared.output_capacity(), capacity);
    assert!(prepared.evaluate(&request[..request.len() - 1]).is_err());
    let mut trailing = request.clone();
    trailing.push(0);
    assert!(prepared.evaluate(&trailing).is_err());
    assert!(prepared.evaluate(&payload).is_err());
    assert_eq!(prepared.evaluate(&request).unwrap(), payload);
}

#[test]
fn complete_request_reuse_and_malformed_refusals_do_not_allocate() {
    let p = program("identity");
    let mut prepared = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    let (request, _) = value(&p, 256);
    let (empty, _) = value(&p, 0);
    let allocations = allocation::allocations(|| {
        for _ in 0..10_000 {
            assert_eq!(prepared.evaluate(&request).unwrap(), request);
            assert!(prepared.evaluate(&request[..request.len() - 1]).is_err());
            assert_eq!(prepared.evaluate(&empty).unwrap(), empty);
        }
    });
    assert_eq!(
        allocations, 0,
        "request reuse and refusals must be allocation-free"
    );
}

#[test]
fn serialized_identity_cannot_relabel_a_different_structured_type() {
    let mut p = program("identity");
    p.output_type = program("payload").output_type;
    assert!(PreparedPortableExpressionEvaluator::new(&p).is_err());
    p.root.value_type = p.output_type.clone();
    p.root.operation = PortableExpressionOperation::Input;
    assert!(PreparedPortableExpressionEvaluator::new(&p).is_err());
}
