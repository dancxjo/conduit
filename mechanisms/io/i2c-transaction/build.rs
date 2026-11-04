//! Check the authored protocol schemas on the fabrication host, not on the MCU.
use conduit_plot::{check_syntax_document, parse_syntax_document, StartupCatalog};
use std::{env, fs, path::PathBuf};

fn main() {
    let source = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap())
        .join("../../../plots/device-protocols/i2c-types.conduit");
    println!("cargo:rerun-if-changed={}", source.display());
    let syntax = parse_syntax_document(&fs::read_to_string(source).expect("I2C Type Source"));
    assert!(syntax.diagnostics.is_empty(), "invalid I2C Type Source");
    let checked =
        check_syntax_document(&syntax, &StartupCatalog::new()).expect("checked I2C Type Source");
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    for (name, file) in [
        ("I2cTransaction", "request.type"),
        ("I2cResult", "result.type"),
    ] {
        let ty = checked
            .native_types
            .iter()
            .find(|ty| ty.name == name)
            .expect("named I2C Type");
        fs::write(
            output.join(file),
            ty.value_type.canonical_bytes().expect("canonical I2C Type"),
        )
        .expect("write checked I2C schema");
    }
}
