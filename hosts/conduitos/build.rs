fn main() {
    println!("cargo:rerun-if-changed=linker/x86_64.ld");
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("x86_64")
        && std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("none")
    {
        let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("Cargo sets manifest directory");
        println!("cargo:rustc-link-arg=-T{manifest}/linker/x86_64.ld");
    }
}
