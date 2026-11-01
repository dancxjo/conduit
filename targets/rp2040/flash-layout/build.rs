//! Geometry is prepared once; firmware receives constants and linker text only.
#[path = "build/definition.rs"]
mod definition;
fn main() {
    let source = "../deployment/browser/flash-layout.mjs";
    println!("cargo:rerun-if-changed={source}");
    println!("cargo:rerun-if-changed=build/definition.rs");
    let layout = definition::Layout::decode(include_str!("../deployment/browser/flash-layout.mjs"))
        .expect("checked Pico flash layout");
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    std::fs::write(out.join("layout.rs"), layout.rust()).unwrap();
    std::fs::write(out.join("memory.x"), layout.linker()).unwrap();
}
