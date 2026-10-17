//! Native laws must not disappear when a scalar representation is specialized.
use conduit_core::ConfigurationValue;
use conduit_plot::*;

fn program(source: &str) -> PortableExpressionProgram {
    let checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, "choose", &ProfileCatalog::new()).unwrap();
    let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!("program")
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}

fn refuses_without_native_witness(program: &PortableExpressionProgram) {
    assert!(matches!(
        PreparedPortableExpressionEvaluator::new_scalar_tape(program),
        Err(PortableExpressionEvaluationRefusal::UnsupportedType(reason))
            if reason == "scalar-tape/native-law-witness-unavailable@1"
    ));
}

#[test]
fn native_range_and_where_profiles_refuse_before_any_play_input() {
    for declaration in [
        "type Small = U64 in 0..=10",
        "type Small = U64 where . <= 10",
    ] {
        let source =
            format!("{declaration}\nplot choose (\n >> value: Small\n result: Small >>\n) = (.)\n");
        refuses_without_native_witness(&program(&source));
    }
    // This where law can justify erasing overflow checks in checked arithmetic.
    // The representation alone cannot justify accepting arbitrary U32 bytes.
    refuses_without_native_witness(&program(
        "type Small = U32 where . < 4_294_967_295\nplot choose (\n >> value: Small\n result: U32 >>\n) = (. + 1)\n",
    ));
}

#[test]
fn native_type_on_an_intermediate_node_cannot_hide_behind_intrinsic_ports() {
    let native = program(
        "type Small = U64 in 0..=10\nplot choose (\n >> value: Small\n result: Small >>\n) = (.)\n",
    )
    .input_type;
    let mut intrinsic = program("plot choose (\n >> value: U64\n result: U64 >>\n) = (. + 1)\n");
    let PortableExpressionOperation::Binary { left, .. } = &mut intrinsic.root.operation else {
        panic!("binary")
    };
    left.value_type = native;
    refuses_without_native_witness(&intrinsic);
}
