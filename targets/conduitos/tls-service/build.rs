fn main() {
    let directory = std::env::var("CARGO_MANIFEST_DIR").expect("Cargo manifest directory");
    println!("cargo:rerun-if-changed=linker.ld");
    println!("cargo:rustc-link-arg=-T{directory}/linker.ld");
    println!("cargo:rustc-link-arg=--build-id=none");
    println!("cargo:rustc-link-arg=-z");
    println!("cargo:rustc-link-arg=max-page-size=4096");
}
