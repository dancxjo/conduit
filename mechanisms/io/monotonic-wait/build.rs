//! Compile reviewed clock-at schemas; parser/checker never enter MCU runtime.
fn main() {
    let source = "../../../plots/device-protocols/clock-types.conduit";
    println!("cargo:rerun-if-changed={source}");
    if std::env::var_os("CARGO_FEATURE_CLOCK_AT").is_none() {
        return;
    }
    let source = std::fs::read_to_string(source).expect("reviewed clock Types");
    let syntax = conduit_plot::parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty());
    let checked =
        conduit_plot::check_syntax_document(&syntax, &conduit_plot::StartupCatalog::new())
            .expect("checked clock Types");
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    for (name, file) in [
        ("MonotonicClockRequest", "clock-request.type"),
        ("MonotonicClockResult", "clock-result.type"),
    ] {
        let ty = checked
            .native_types
            .iter()
            .find(|ty| ty.name == name)
            .expect("clock Type");
        assert!(ty.invariants.is_empty() && ty.value_contracts.is_empty());
        std::fs::write(out.join(file), ty.value_type.canonical_bytes().unwrap()).unwrap();
    }
}
