fn main() {
    println!("cargo:rerun-if-changed=linker/x86_64.ld");
    println!("cargo:rerun-if-changed=linker/aarch64_a0.ld");
    println!("cargo:rerun-if-changed=linker/aarch64_a2.ld");
    println!("cargo:rerun-if-changed=linker/aarch64_a3.ld");
    println!("cargo:rerun-if-env-changed=CONDUITOS_BUILD_ID");
    println!("cargo:rerun-if-env-changed=CONDUITOS_IMAGE_ID");
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("Cargo sets manifest directory");
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("x86_64")
        && std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("none")
    {
        println!("cargo:rustc-link-arg=-T{manifest}/linker/x86_64.ld");
    }
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("aarch64")
        && std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("none")
    {
        println!("cargo:rustc-link-arg-bin=conduitos-aarch64-a0=-T{manifest}/linker/aarch64_a0.ld");
        println!("cargo:rustc-link-arg-bin=conduitos-aarch64-a2=-T{manifest}/linker/aarch64_a2.ld");
        println!("cargo:rustc-link-arg-bin=conduitos-aarch64-a3=-T{manifest}/linker/aarch64_a3.ld");
    }
}
