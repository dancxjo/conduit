use conduit_core::ConfigurationValue;
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, PreparedPortableExpressionEvaluator, ProfileCatalog, StartupCatalog,
};
use conduit_seven_segment_mask::{SevenSegmentLook, SevenSegmentTemperatureUnit};

#[test]
fn the_requested_fahrenheit_look_is_checked_source_data() {
    let syntax = parse_syntax_document(include_str!("../types.conduit"));
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, "fahrenheit-look", &ProfileCatalog::new())
            .unwrap()
            .expanded;
    let ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value else {
        panic!("Source Look")
    };
    let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
    let mut prepared = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    let look = SevenSegmentLook::decode(prepared.evaluate(&[]).unwrap()).unwrap();
    assert_eq!(
        look.temperature_unit(),
        &SevenSegmentTemperatureUnit::Fahrenheit
    );
    assert_eq!(look, SevenSegmentLook::fahrenheit());
}
