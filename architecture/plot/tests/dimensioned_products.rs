//! Dimensional products retain ordinary physical capsules without Play allocation.
use conduit_core::*;
use conduit_plot::*;
#[path = "common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

fn compile(input: &str, body: &str) -> PortableExpressionProgram {
    let source = format!("plot derive (\n >> value: {input}\n result: Quantity >>\n) = ({body})\n");
    let syntax = parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let authored =
        expand_canonical_plot_for_authoring(&checked, "derive", &ProfileCatalog::new()).unwrap();
    let ConfigurationValue::Text(encoded) = &authored.expanded.gears[0].configuration[0].value
    else {
        panic!("program")
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}
fn quantity(text: &str) -> Quantity {
    Quantity::parse_plot_literal(text).unwrap()
}
fn evaluate(program: &PortableExpressionProgram, input: Quantity) -> Quantity {
    let expected = program.evaluate(&input.encode()).unwrap();
    let mut prepared = PreparedPortableExpressionEvaluator::new(program).unwrap();
    let capacity = prepared.output_capacity();
    let (received, allocations) =
        allocation_probe::observe(|| prepared.evaluate(&input.encode()).unwrap());
    assert_eq!(received, expected);
    assert_eq!((allocations.allocations, allocations.reallocations), (0, 0));
    assert_eq!(prepared.output_capacity(), capacity);
    Quantity::decode(received).unwrap()
}
#[test]
fn authored_products_and_quotients_derive_dimensions_with_exact_reference_coordinates() {
    let length = quantity("1m").dimension();
    let time = quantity("1s").dimension();
    for (body, dimension, expected) in [
        ("value / 2s", length.quotient(time).unwrap(), (15, -2)),
        ("value * 2s", length.product(time).unwrap(), (6, -1)),
        ("value * 2m", length.product(length).unwrap(), (6, -1)),
    ] {
        let value = evaluate(&compile("Distance", body), quantity("30cm"));
        assert_eq!(value.dimension(), dimension);
        assert_eq!((value.coefficient(), value.exponent()), expected);
        assert_eq!(value.role(), QuantityRole::Linear);
    }
    // Literal-only and runtime-generic operands use the same checked path.
    let literal = evaluate(&compile("Distance", "30cm / 2s"), quantity("0m"));
    let generic = evaluate(&compile("Quantity", "value / 2s"), quantity("30cm"));
    assert_eq!(literal, generic);
    let swapped = evaluate(&compile("Distance", "2s * value"), quantity("30cm"));
    assert_eq!(
        swapped,
        evaluate(&compile("Distance", "value * 2s"), quantity("30cm"))
    );
}
#[test]
fn nondecimal_zero_and_decimal_extent_refusals_match_without_rounding() {
    for (body, input) in [("value / 3s", "1m"), ("value / 0s", "1m")] {
        let program = compile("Distance", body);
        let reference = program.evaluate(&quantity(input).encode()).unwrap_err();
        let mut prepared = PreparedPortableExpressionEvaluator::new(&program).unwrap();
        assert_eq!(
            prepared.evaluate(&quantity(input).encode()).unwrap_err(),
            reference
        );
        if body.contains("3s") {
            assert_eq!(
                reference,
                PortableExpressionEvaluationRefusal::QuantityProjection(
                    ExactNumericRefusal::InexactProjection
                )
            );
        } else {
            assert_eq!(
                reference,
                PortableExpressionEvaluationRefusal::QuantityProduct(
                    RationalQuantityRefusal::Numeric(ExactNumericRefusal::DivisionByZero)
                )
            );
        }
    }
    let program = compile("Distance", "value * 10m");
    let extreme = Quantity::from_decimal(1, 128, Unit::resolve("m").unwrap()).unwrap();
    assert!(matches!(
        program.evaluate(&extreme.encode()),
        Err(PortableExpressionEvaluationRefusal::QuantityEncoding(
            QuantityRefusal::ExponentOutOfRange
        ))
    ));
}
#[test]
fn generic_runtime_points_and_delta_values_cannot_acquire_linear_product_roles() {
    let program = compile("Quantity", "value * 2s");
    for role in [QuantityRole::Point, QuantityRole::Delta] {
        let value = Quantity::from_decimal_role(21, 0, Unit::resolve("°C").unwrap(), role).unwrap();
        assert!(program.evaluate(&value.encode()).is_err());
        assert!(PreparedPortableExpressionEvaluator::new(&program)
            .unwrap()
            .evaluate(&value.encode())
            .is_err());
    }
}
