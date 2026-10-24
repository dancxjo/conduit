#[cfg(not(feature = "host-call"))]
fn main() {}
#[cfg(feature = "host-call")]
fn main() {
    use conduit_plot::{check_syntax_document, parse_syntax_document, StartupCatalog};
    let path = "../../../plots/device-protocols/tm1637.conduit";
    println!("cargo:rerun-if-changed={path}");
    let syntax = parse_syntax_document(&std::fs::read_to_string(path).unwrap());
    assert!(syntax.diagnostics.is_empty());
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let ty = checked
        .native_types
        .iter()
        .find(|ty| ty.name == "Tm1637Packets")
        .unwrap();
    assert!(ty.invariants.is_empty() && ty.value_contracts.is_empty());
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    std::fs::write(
        out.join("packets.type"),
        ty.value_type.canonical_bytes().unwrap(),
    )
    .unwrap();
}
