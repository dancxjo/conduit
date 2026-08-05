use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let target = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    // Only emit the RP2040-specific linker flags for thumbv6m
    if env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default() == "arm"
        && env::var("TARGET").unwrap_or_default() == "thumbv6m-none-eabi"
    {
        let out = PathBuf::from(env::var("OUT_DIR").unwrap());
        let memory_x = include_str!("memory.x");
        fs::write(out.join("memory.x"), memory_x).unwrap();
        println!("cargo:rustc-link-search={}", out.display());
        println!("cargo:rustc-link-arg=-Tlink.x");
    }
    let _ = target;
    println!("cargo:rerun-if-changed=memory.x");
    println!("cargo:rerun-if-changed=build.rs");
}
