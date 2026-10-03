use conduit_core::ConfigurationValue;
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, PreparedPortableExpressionEvaluator, ProfileCatalog, StartupCatalog,
};
fn lifecycle_catalog() -> StartupCatalog {
    let checked = check_syntax_document(
        &parse_syntax_document(include_str!(
            "../../../plots/device-protocols/i2c-types.conduit"
        )),
        &StartupCatalog::new(),
    )
    .unwrap();
    let mut catalog = StartupCatalog::new();
    for (name, path) in [
        ("I2cTransaction", "machine/i2c/transact/request"),
        ("I2cResult", "machine/i2c/transact/result"),
    ] {
        catalog
            .insert_checked_native_type(
                path,
                checked
                    .native_types
                    .iter()
                    .find(|ty| ty.name == name)
                    .unwrap(),
            )
            .unwrap();
    }
    catalog
}
#[test]
fn lifecycle_policy_is_checked_source_and_prepares_without_device_code() {
    let syntax = parse_syntax_document(include_str!(
        "../../../plots/device-protocols/bme280-lifecycle.conduit"
    ));
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &lifecycle_catalog()).unwrap();
    for name in checked.plots.iter().map(|plot| plot.name.as_str()) {
        let expanded = expand_canonical_plot_for_authoring(&checked, name, &ProfileCatalog::new())
            .unwrap_or_else(|error| panic!("{name}: {error:?}"))
            .expanded;
        if name == "bme280-protocol-transition" {
            assert!(expanded.gears.len() > 1);
        } else {
            assert_eq!(expanded.gears.len(), 1);
        }
        let ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value else {
            panic!("program")
        };
        let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
        PreparedPortableExpressionEvaluator::new(&program).unwrap();
    }
}
#[path = "common/allocation_probe.rs"]
mod allocation_probe;
#[path = "bme280_lifecycle/fixture.rs"]
mod fixture;
#[path = "bme280_lifecycle/transcript.rs"]
mod transcript;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

#[test]
fn preparation_footprint_is_finite_and_reported() {
    let (fixture, observation) = allocation_probe::observe(fixture::Fixture::new);
    assert!(fixture.checked_plot_count() > 10);
    assert!(observation.peak_bytes < 16 * 1024 * 1024, "{observation:?}");
    eprintln!("BME280 policy preparation: {observation:?}");
}
