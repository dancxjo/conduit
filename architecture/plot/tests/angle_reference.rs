//! Explicit Angle/unit boundary through the ordinary expression compiler.
use conduit_core::*;
use conduit_plot::*;
#[path = "common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn source(input: &str, body: &str) -> String {
    format!("plot bridge (\n >> angle: {input}\n radians: MathematicalValue >>\n) = ({body})\n")
}
fn compile(text: &str) -> PortableExpressionProgram {
    let checked =
        check_syntax_document(&parse_syntax_document(text), &StartupCatalog::new()).unwrap();
    let authored =
        expand_canonical_plot_for_authoring(&checked, "bridge", &ProfileCatalog::new()).unwrap();
    let ConfigurationValue::Text(encoded) = &authored.expanded.gears[0].configuration[0].value
    else {
        panic!("expression program");
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}
fn expected(original: &str) -> Vec<u8> {
    let receipt = PreparedAngleReferenceConversion::from_checked(
        Quantity::parse_plot_literal(original).unwrap(),
        Unit::Radian,
        original,
        "rad",
        &mut ExactNumericWork::new(100_000_000),
    )
    .unwrap();
    let mut bytes = vec![0; MATHEMATICAL_VALUE_MAX_ENCODED_BYTES];
    let length = receipt.denotation().encode(&mut bytes).unwrap();
    bytes.truncate(length);
    bytes
}
#[test]
fn prepared_reference_writer_matches_exact_receipts_without_play_allocations() {
    let program = compile(&source("Angle", "math/radians(.)"));
    let mut prepared = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    let capacity = prepared.output_capacity();
    for original in [
        "0turn", "0.5turn", "1turn", "-1turn", "180°", "0.17turn", "2rev",
    ] {
        let input = Quantity::parse_plot_literal(original).unwrap().encode();
        let expected = expected(original);
        assert_eq!(program.evaluate(&input).unwrap(), expected);
        let (actual, storage) = allocation_probe::observe(|| prepared.evaluate(&input).unwrap());
        assert_eq!(actual, expected);
        assert_eq!((storage.allocations, storage.reallocations), (0, 0));
        assert_eq!(prepared.output_capacity(), capacity);
    }
}
#[test]
fn explicit_projection_preserves_original_angle_in_the_source_record() {
    let text="type Observation = {\n angle: Angle\n radians: MathematicalValue\n}\nplot bridge (\n >> angle: Angle\n result: Observation >>\n) = ({angle: ., radians: math/radians(.)})\n";
    let program = compile(text);
    let mut prepared = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    let angle = Quantity::parse_plot_literal("180°").unwrap();
    let (actual, storage) =
        allocation_probe::observe(|| prepared.evaluate(&angle.encode()).unwrap());
    assert_eq!((storage.allocations, storage.reallocations), (0, 0));
    let observation = StructuredInfoValue::from_canonical_bytes(actual).unwrap();
    let StructuredInfoValueShape::Record(fields) = observation.shape() else {
        panic!("record");
    };
    for (name, expected) in [
        ("angle", angle.encode().to_vec()),
        ("radians", expected("180°")),
    ] {
        let field = fields.iter().find(|f| f.name() == name).unwrap();
        let StructuredInfoValueShape::Leaf(bytes) = field.value().shape() else {
            panic!("leaf");
        };
        assert_eq!(*bytes, expected);
    }
}
#[test]
fn scalars_wrong_dimensions_and_unrelated_origins_are_not_implicit_radians() {
    for (input, body) in [
        ("Scalar", "math/radians(.)"),
        ("Distance", "math/radians(.)"),
        ("Angle", "math/radians(1m)"),
        ("Angle", "math/radians(42)"),
    ] {
        let text = source(input, body);
        let checked =
            check_syntax_document(&parse_syntax_document(&text), &StartupCatalog::new()).unwrap();
        assert!(
            expand_canonical_plot_for_authoring(&checked, "bridge", &ProfileCatalog::new())
                .is_err()
        );
    }
    let constant = compile(&source("Angle", "math/radians(1rad)"));
    assert!(matches!(
        PreparedPortableExpressionEvaluator::new(&constant),
        Err(PortableExpressionEvaluationRefusal::AngleReference(
            AngleReferenceRefusal::Coordinate(_)
        ))
    ));
    let dynamic = compile(&source("Angle", "math/radians(.)"));
    let mut prepared = PreparedPortableExpressionEvaluator::new(&dynamic).unwrap();
    assert!(prepared
        .evaluate(&Quantity::new(1, Unit::Radian).encode())
        .is_err());
}
