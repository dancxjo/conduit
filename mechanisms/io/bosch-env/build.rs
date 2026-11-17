//! Optional typed kernel boundary: compile schemas once on the building Host.
#[cfg(not(feature = "kernel-step"))]
fn main() {}

#[cfg(feature = "kernel-step")]
fn main() {
    use conduit_plot::{check_syntax_document, parse_syntax_document, StartupCatalog};
    use std::{env, fs, path::PathBuf};
    let source = "../../../plots/device-protocols/main.conduit";
    println!("cargo:rerun-if-changed={source}");
    let syntax = parse_syntax_document(&fs::read_to_string(source).unwrap());
    assert!(syntax.diagnostics.is_empty());
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let mut constants = String::new();
    for (name, file, constant) in [
        ("BmeObservationInput", "input.type", "INPUT_MAXIMUM_BYTES"),
        (
            "BmeObservationResult",
            "result.type",
            "RESULT_MAXIMUM_BYTES",
        ),
    ] {
        let ty = checked
            .native_types
            .iter()
            .find(|ty| ty.name == name)
            .unwrap();
        assert!(ty.invariants.is_empty() && ty.value_contracts.is_empty());
        fs::write(out.join(file), ty.value_type.canonical_bytes().unwrap()).unwrap();
        let maximum = conduit_plot::maximum_prepared_canonical_value_bytes(&ty.value_type).unwrap();
        constants.push_str(&format!("pub const {constant}: usize = {maximum};\n"));
    }
    let plot = checked
        .plots
        .iter()
        .find(|plot| plot.name == "bme280-observation")
        .unwrap();
    // A handwritten native implementation may not silently retarget itself to
    // newly changed Source. Updating either pin requires correspondence review
    // and differential qualification, not merely regenerating schema bytes.
    assert_eq!(
        checked.source_document_id.as_str(),
        "5d61f0ccbe6d2bc90ca070270b63aad6bef2b75e4dd2274253054744accd2653",
        "native BME Source changed: requalify the reviewed implementation"
    );
    assert_eq!(
        plot.checked_plot_id.as_str(),
        "9053de354a696fe977f884b1f55fa904b72957c7414d36d832902f8830c3f904",
        "native BME checked Plot changed: requalify the reviewed implementation"
    );
    constants.push_str(&format!(
        "pub const SOURCE_DOCUMENT_ID: &str = {:?};\npub const CHECKED_PLOT_ID: &str = {:?};\n",
        checked.source_document_id.as_str(),
        plot.checked_plot_id.as_str(),
    ));
    fs::write(out.join("contract.rs"), constants).unwrap();
}
