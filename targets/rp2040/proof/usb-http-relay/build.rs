fn main() {
    println!("cargo:rerun-if-changed=../../firmware/pico-w-signal/memory.x");
    if std::env::var("TARGET").unwrap().starts_with("thumbv6m-") {
        let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
        std::fs::write(
            out.join("memory.x"),
            include_bytes!("../../firmware/pico-w-signal/memory.x"),
        )
        .unwrap();
        println!("cargo:rustc-link-search={}", out.display());
        println!("cargo:rustc-link-arg=-Tlink.x");
    }
}
